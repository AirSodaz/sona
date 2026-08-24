use std::collections::HashMap;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};

use async_trait::async_trait;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{LlamaChatMessage, LlamaModel};
use llama_cpp_2::mtmd::{
    MtmdBitmap, MtmdContext, MtmdContextParams, MtmdInputText, mtmd_default_marker,
};
use llama_cpp_2::sampling::LlamaSampler;
use sona_core::models::config::ModelFileConfig;
use sona_core::ports::asr::{
    AsrPortError, AsrPortErrorKind, BatchTranscriberPort, BatchTranscriptionObserver,
    LocalAsrEngine, NoopBatchTranscriptionObserver, local_asr_engine_mismatch,
};
use sona_core::transcription::runtime::BatchTranscribePlan;
use sona_core::transcription::transcript::{
    TranscriptSegment, TranscriptUpdate, ensure_transcript_segment_timing,
    normalize_recognizer_text,
};

const MODEL_TYPE_QWEN3_ASR: &str = "qwen3-asr";
const N_BATCH: i32 = 512;
const MAX_GENERATED_TOKENS: usize = 4096;

static BACKEND: OnceLock<Result<LlamaBackend, String>> = OnceLock::new();
static MODEL_CACHE: OnceLock<Mutex<HashMap<PathBuf, Arc<LlamaModel>>>> = OnceLock::new();

#[derive(Debug, Clone, Copy, Default)]
pub struct LlamaBatchAsrAdapter;

#[async_trait]
impl BatchTranscriberPort for LlamaBatchAsrAdapter {
    async fn transcribe(
        &self,
        plan: BatchTranscribePlan,
    ) -> Result<Vec<TranscriptSegment>, AsrPortError> {
        let job = LlamaBatchTranscriptionJob::from_plan(plan)?;
        tokio::task::spawn_blocking(move || {
            job.transcribe(Arc::new(NoopBatchTranscriptionObserver))
        })
        .await
        .map_err(|error| {
            AsrPortError::new(
                AsrPortErrorKind::Runtime,
                format!("llama.cpp transcription task failed: {error}"),
            )
        })?
    }

    async fn transcribe_with_observer(
        &self,
        plan: BatchTranscribePlan,
        observer: Arc<dyn BatchTranscriptionObserver>,
    ) -> Result<Vec<TranscriptSegment>, AsrPortError> {
        let job = LlamaBatchTranscriptionJob::from_plan(plan)?;
        tokio::task::spawn_blocking(move || job.transcribe(observer))
            .await
            .map_err(|error| {
                AsrPortError::new(
                    AsrPortErrorKind::Runtime,
                    format!("llama.cpp transcription task failed: {error}"),
                )
            })?
    }
}

#[derive(Debug)]
struct LlamaBatchTranscriptionJob {
    input_path: PathBuf,
    model_path: PathBuf,
    mmproj_path: PathBuf,
    num_threads: i32,
}

impl LlamaBatchTranscriptionJob {
    fn from_plan(plan: BatchTranscribePlan) -> Result<Self, AsrPortError> {
        if plan.engine != LocalAsrEngine::LlamaCpp {
            return Err(local_asr_engine_mismatch(
                LocalAsrEngine::LlamaCpp,
                plan.engine,
            ));
        }
        if plan.model_type != MODEL_TYPE_QWEN3_ASR {
            return Err(AsrPortError::new(
                AsrPortErrorKind::Unsupported,
                format!(
                    "llama.cpp batch ASR currently supports only model type '{MODEL_TYPE_QWEN3_ASR}', got '{}'.",
                    plan.model_type
                ),
            ));
        }
        if !plan.input_path.is_file() {
            return Err(AsrPortError::new(
                AsrPortErrorKind::InvalidRequest,
                format!(
                    "Input file must be an existing file: {}",
                    plan.input_path.display()
                ),
            ));
        }
        if plan.num_threads <= 0 {
            return Err(AsrPortError::invalid_request(
                "llama.cpp thread count must be greater than zero.",
            ));
        }
        validate_supported_options(&plan)?;

        let file_config = plan.file_config.as_ref().ok_or_else(|| {
            AsrPortError::new(
                AsrPortErrorKind::Model,
                "Qwen3-ASR llama.cpp models require model and mmproj file configuration.",
            )
        })?;
        let model_root = Path::new(&plan.model_path);
        let model_path = resolve_required_model_file(model_root, file_config, false)?;
        let mmproj_path = resolve_required_model_file(model_root, file_config, true)?;

        Ok(Self {
            input_path: plan.input_path,
            model_path,
            mmproj_path,
            num_threads: plan.num_threads,
        })
    }

    fn transcribe(
        self,
        observer: Arc<dyn BatchTranscriptionObserver>,
    ) -> Result<Vec<TranscriptSegment>, AsrPortError> {
        let backend = backend()?;
        let model = load_model(backend, &self.model_path)?;
        let mtmd_params = MtmdContextParams {
            use_gpu: false,
            n_threads: self.num_threads,
            ..MtmdContextParams::default()
        };
        let mmproj = path_to_str(&self.mmproj_path, "mmproj")?;
        let mtmd = MtmdContext::init_from_file(mmproj, &model, &mtmd_params).map_err(|error| {
            AsrPortError::new(
                AsrPortErrorKind::Model,
                format!(
                    "Failed to initialize Qwen3-ASR mmproj {}: {error}",
                    self.mmproj_path.display()
                ),
            )
        })?;
        if !mtmd.support_audio() {
            return Err(AsrPortError::new(
                AsrPortErrorKind::Model,
                format!(
                    "Multimodal projector does not support audio: {}",
                    self.mmproj_path.display()
                ),
            ));
        }

        let sample_rate = mtmd.get_audio_sample_rate().unwrap_or(16_000).max(1);
        let samples = decode_audio_input(&self.input_path, sample_rate)?;
        observer.on_progress(10.0);
        let audio = MtmdBitmap::from_audio_data(&samples).map_err(|error| {
            AsrPortError::new(
                AsrPortErrorKind::InvalidRequest,
                format!(
                    "Failed to create llama.cpp audio input {}: {error}",
                    self.input_path.display()
                ),
            )
        })?;
        if !audio.is_audio() {
            return Err(AsrPortError::invalid_request(format!(
                "Input is not a supported audio file: {}",
                self.input_path.display()
            )));
        }

        let context_size = NonZeroU32::new(model.n_ctx_train().max(32_768));
        let context_params = LlamaContextParams::default()
            .with_n_ctx(context_size)
            .with_n_batch(N_BATCH as u32)
            .with_n_threads(self.num_threads)
            .with_n_threads_batch(self.num_threads);
        let mut context = model
            .new_context(backend, context_params)
            .map_err(|error| {
                AsrPortError::new(
                    AsrPortErrorKind::Runtime,
                    format!("Failed to create llama.cpp inference context: {error}"),
                )
            })?;

        let prompt = qwen3_asr_prompt(&model)?;
        let chunks = mtmd
            .tokenize(
                MtmdInputText {
                    text: prompt,
                    add_special: true,
                    parse_special: true,
                },
                &[&audio],
            )
            .map_err(|error| {
                AsrPortError::new(
                    AsrPortErrorKind::Model,
                    format!("Failed to tokenize Qwen3-ASR audio prompt: {error}"),
                )
            })?;
        let n_past = chunks
            .eval_chunks(&mtmd, &context, 0, 0, N_BATCH, true)
            .map_err(|error| {
                AsrPortError::new(
                    AsrPortErrorKind::Runtime,
                    format!("Failed to evaluate Qwen3-ASR audio: {error}"),
                )
            })?;
        observer.on_progress(60.0);

        let mut sampler = LlamaSampler::greedy();
        let mut decoder = encoding_rs::UTF_8.new_decoder();
        let mut generated = String::new();
        let segment_id = uuid::Uuid::new_v4().to_string();
        let duration = samples.len() as f64 / f64::from(sample_rate);
        let mut generated_tokens = 0usize;
        let available = context.n_ctx().saturating_sub(n_past.max(0) as u32) as usize;
        let generation_limit = MAX_GENERATED_TOKENS.min(available);
        let generation_end =
            n_past.saturating_add(i32::try_from(generation_limit).unwrap_or(i32::MAX));
        for token_position in n_past..generation_end {
            let token = sampler.sample(&context, -1);
            if model.is_eog_token(token) {
                break;
            }
            sampler.accept(token);
            generated.push_str(
                &model
                    .token_to_piece(token, &mut decoder, false, None)
                    .map_err(|error| {
                        AsrPortError::new(
                            AsrPortErrorKind::Protocol,
                            format!("Failed to decode llama.cpp output token: {error}"),
                        )
                    })?,
            );
            generated_tokens = generated_tokens.saturating_add(1);

            if generated_tokens.is_multiple_of(8) {
                emit_partial_transcript(
                    observer.as_ref(),
                    &segment_id,
                    duration,
                    &generated,
                    generated_tokens,
                );
            }

            let mut batch = LlamaBatch::new(1, 1);
            batch
                .add(token, token_position, &[0], true)
                .map_err(|error| {
                    AsrPortError::new(
                        AsrPortErrorKind::Runtime,
                        format!("Failed to prepare llama.cpp decode batch: {error}"),
                    )
                })?;
            context.decode(&mut batch).map_err(|error| {
                AsrPortError::new(
                    AsrPortErrorKind::Runtime,
                    format!("llama.cpp token generation failed: {error}"),
                )
            })?;
        }

        let text = parse_qwen3_asr_output(&generated);
        if text.is_empty() {
            observer.on_progress(100.0);
            return Ok(Vec::new());
        }
        let mut segment = TranscriptSegment {
            id: segment_id,
            text,
            start: 0.0,
            end: duration,
            is_final: true,
            timing: None,
            tokens: None,
            timestamps: None,
            durations: None,
            translation: None,
            speaker: None,
            speaker_attribution: None,
        };
        ensure_transcript_segment_timing(&mut segment);
        observer.on_transcript_update(&TranscriptUpdate {
            remove_ids: Vec::new(),
            upsert_segments: vec![segment.clone()],
        });
        observer.on_progress(100.0);
        Ok(vec![segment])
    }
}

fn emit_partial_transcript(
    observer: &dyn BatchTranscriptionObserver,
    segment_id: &str,
    duration: f64,
    generated: &str,
    generated_tokens: usize,
) {
    let text = parse_qwen3_asr_partial_output(generated);
    if text.is_empty() {
        return;
    }

    let mut segment = TranscriptSegment {
        id: segment_id.to_string(),
        text,
        start: 0.0,
        end: duration,
        is_final: false,
        timing: None,
        tokens: None,
        timestamps: None,
        durations: None,
        translation: None,
        speaker: None,
        speaker_attribution: None,
    };
    ensure_transcript_segment_timing(&mut segment);
    observer.on_transcript_update(&TranscriptUpdate {
        remove_ids: Vec::new(),
        upsert_segments: vec![segment],
    });
    observer.on_progress(llama_generation_progress(generated_tokens));
}

fn llama_generation_progress(generated_tokens: usize) -> f32 {
    const GENERATION_START: f32 = 60.0;
    const GENERATION_SPAN: f32 = 35.0;
    const TOKEN_SCALE: f32 = 160.0;

    let completion_curve = 1.0 - (-(generated_tokens as f32) / TOKEN_SCALE).exp();
    (GENERATION_START + GENERATION_SPAN * completion_curve).min(95.0)
}

fn decode_audio_input(path: &Path, sample_rate: u32) -> Result<Vec<f32>, AsrPortError> {
    let ffmpeg_path = resolve_ffmpeg_sidecar_path()?;
    let mut command = Command::new(&ffmpeg_path);

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }

    let output = command
        .arg("-loglevel")
        .arg("error")
        .arg("-i")
        .arg(path)
        .arg("-f")
        .arg("s16le")
        .arg("-acodec")
        .arg("pcm_s16le")
        .arg("-ar")
        .arg(sample_rate.to_string())
        .arg("-ac")
        .arg("1")
        .arg("-")
        .output()
        .map_err(|error| {
            AsrPortError::new(
                AsrPortErrorKind::FileSystem,
                format!(
                    "Failed to run FFmpeg audio decoder {}: {error}",
                    ffmpeg_path.display()
                ),
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(AsrPortError::new(
            AsrPortErrorKind::InvalidRequest,
            format!("Failed to decode audio input {}: {stderr}", path.display()),
        ));
    }

    let samples = pcm_s16le_bytes_to_f32(&output.stdout);
    if samples.is_empty() {
        return Err(AsrPortError::invalid_request(format!(
            "Decoded audio input contains no samples: {}",
            path.display()
        )));
    }
    Ok(samples)
}

fn resolve_ffmpeg_sidecar_path() -> Result<PathBuf, AsrPortError> {
    let executable = std::env::current_exe().map_err(|error| {
        AsrPortError::new(
            AsrPortErrorKind::FileSystem,
            format!("Failed to locate the current executable: {error}"),
        )
    })?;
    resolve_ffmpeg_sidecar_path_from_exe(&executable)
}

fn resolve_ffmpeg_sidecar_path_from_exe(executable: &Path) -> Result<PathBuf, AsrPortError> {
    let directory = executable.parent().ok_or_else(|| {
        AsrPortError::new(
            AsrPortErrorKind::FileSystem,
            "Failed to locate the executable directory.",
        )
    })?;

    #[cfg(windows)]
    let filename = "ffmpeg.exe";
    #[cfg(not(windows))]
    let filename = "ffmpeg";

    Ok(directory.join(filename))
}

fn pcm_s16le_bytes_to_f32(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / 32768.0)
        .collect()
}

fn validate_supported_options(plan: &BatchTranscribePlan) -> Result<(), AsrPortError> {
    let mut unsupported = Vec::new();
    if plan.save_to_path.is_some() {
        unsupported.push("save_wav");
    }
    if plan.enable_itn {
        unsupported.push("enable_itn");
    }
    if plan.language != "auto" {
        unsupported.push("language override");
    }
    if plan.punctuation_model.is_some() {
        unsupported.push("punctuation model");
    }
    if plan.vad_model.is_some() {
        unsupported.push("VAD model");
    }
    if plan.hotwords.is_some() {
        unsupported.push("hotwords");
    }
    if plan.speaker_processing.is_some() {
        unsupported.push("speaker processing");
    }
    if !matches!(
        plan.gpu_acceleration.as_deref(),
        None | Some("auto" | "cpu")
    ) {
        unsupported.push("GPU acceleration");
    }

    if unsupported.is_empty() {
        Ok(())
    } else {
        Err(AsrPortError::new(
            AsrPortErrorKind::Unsupported,
            format!(
                "Qwen3-ASR llama.cpp batch transcription does not yet support: {}.",
                unsupported.join(", ")
            ),
        ))
    }
}

fn backend() -> Result<&'static LlamaBackend, AsrPortError> {
    BACKEND
        .get_or_init(|| LlamaBackend::init().map_err(|error| error.to_string()))
        .as_ref()
        .map_err(|error| {
            AsrPortError::new(
                AsrPortErrorKind::Unavailable,
                format!("Failed to initialize llama.cpp backend: {error}"),
            )
        })
}

fn load_model(backend: &LlamaBackend, model_path: &Path) -> Result<Arc<LlamaModel>, AsrPortError> {
    let canonical_path = model_path.canonicalize().map_err(|error| {
        AsrPortError::new(
            AsrPortErrorKind::FileSystem,
            format!(
                "Failed to resolve llama.cpp model path {}: {error}",
                model_path.display()
            ),
        )
    })?;
    let cache = MODEL_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut cache = cache.lock().map_err(|_| {
        AsrPortError::new(
            AsrPortErrorKind::Runtime,
            "llama.cpp model cache lock was poisoned.",
        )
    })?;
    if let Some(model) = cache.get(&canonical_path) {
        return Ok(Arc::clone(model));
    }

    let model = Arc::new(
        LlamaModel::load_from_file(backend, &canonical_path, &LlamaModelParams::default())
            .map_err(|error| {
                AsrPortError::new(
                    AsrPortErrorKind::Model,
                    format!(
                        "Failed to load llama.cpp model {}: {error}",
                        canonical_path.display()
                    ),
                )
            })?,
    );
    cache.insert(canonical_path, Arc::clone(&model));
    Ok(model)
}

fn qwen3_asr_prompt(model: &LlamaModel) -> Result<String, AsrPortError> {
    let message = LlamaChatMessage::new("user".to_string(), mtmd_default_marker().to_string())
        .map_err(|error| {
            AsrPortError::new(
                AsrPortErrorKind::Protocol,
                format!("Failed to construct Qwen3-ASR prompt: {error}"),
            )
        })?;
    let template = model.chat_template(None).map_err(|error| {
        AsrPortError::new(
            AsrPortErrorKind::Model,
            format!("Qwen3-ASR GGUF does not provide a usable chat template: {error}"),
        )
    })?;
    model
        .apply_chat_template(&template, &[message], true)
        .map_err(|error| {
            AsrPortError::new(
                AsrPortErrorKind::Protocol,
                format!("Failed to apply Qwen3-ASR chat template: {error}"),
            )
        })
}

fn resolve_required_model_file(
    model_root: &Path,
    config: &ModelFileConfig,
    mmproj: bool,
) -> Result<PathBuf, AsrPortError> {
    let (label, configured) = if mmproj {
        ("mmproj", config.mmproj.as_deref())
    } else {
        ("model", config.model.as_deref())
    };
    let configured = configured.ok_or_else(|| {
        AsrPortError::new(
            AsrPortErrorKind::Model,
            format!("Qwen3-ASR llama.cpp file configuration is missing '{label}'."),
        )
    })?;
    let configured_path = Path::new(configured);
    let path = if configured_path.is_absolute() {
        configured_path.to_path_buf()
    } else {
        model_root.join(configured_path)
    };
    if !path.is_file() {
        return Err(AsrPortError::new(
            AsrPortErrorKind::Model,
            format!("Qwen3-ASR {label} file was not found: {}", path.display()),
        ));
    }
    Ok(path)
}

fn path_to_str<'a>(path: &'a Path, label: &str) -> Result<&'a str, AsrPortError> {
    path.to_str().ok_or_else(|| {
        AsrPortError::new(
            AsrPortErrorKind::FileSystem,
            format!("{label} path is not valid UTF-8: {}", path.display()),
        )
    })
}

fn parse_qwen3_asr_output(output: &str) -> String {
    let transcript = output
        .split_once("<asr_text>")
        .map_or(output, |(_, transcript)| transcript);
    normalize_recognizer_text(transcript.trim())
}

fn parse_qwen3_asr_partial_output(output: &str) -> String {
    output
        .split_once("<asr_text>")
        .map(|(_, transcript)| normalize_recognizer_text(transcript.trim()))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{
        LlamaBatchTranscriptionJob, MODEL_TYPE_QWEN3_ASR, llama_generation_progress,
        parse_qwen3_asr_output, parse_qwen3_asr_partial_output, pcm_s16le_bytes_to_f32,
        resolve_ffmpeg_sidecar_path_from_exe, validate_supported_options,
    };
    use sona_core::export::ExportFormat;
    use sona_core::ports::asr::{AsrPortErrorKind, LocalAsrEngine};
    use sona_core::transcription::runtime::{BatchTranscribePlan, OutputTarget};
    use std::path::PathBuf;

    fn plan() -> BatchTranscribePlan {
        BatchTranscribePlan {
            input_path: PathBuf::from("audio.wav"),
            save_to_path: None,
            engine: LocalAsrEngine::LlamaCpp,
            model_path: "models/qwen3-asr".to_string(),
            num_threads: 4,
            enable_itn: false,
            language: "auto".to_string(),
            punctuation_model: None,
            vad_model: None,
            vad_buffer: 5.0,
            batch_segmentation_mode: sona_core::ports::asr::BatchSegmentationMode::Whole,
            model_type: MODEL_TYPE_QWEN3_ASR.to_string(),
            file_config: None,
            hotwords: None,
            speaker_processing: None,
            gpu_acceleration: Some("auto".to_string()),
            export_format: ExportFormat::Json,
            output_target: OutputTarget::Stdout,
            quiet: true,
        }
    }

    #[test]
    fn extracts_qwen3_asr_text_payload() {
        assert_eq!(
            parse_qwen3_asr_output("language Chinese<asr_text> 你好，世界 "),
            "你好，世界"
        );
    }

    #[test]
    fn accepts_plain_transcript_output() {
        assert_eq!(parse_qwen3_asr_output(" hello world "), "hello world");
    }

    #[test]
    fn partial_output_waits_for_asr_text_marker() {
        assert_eq!(parse_qwen3_asr_partial_output("language Chinese"), "");
        assert_eq!(
            parse_qwen3_asr_partial_output("language Chinese<asr_text>你好"),
            "你好"
        );
    }

    #[test]
    fn token_generation_progress_is_monotonic_and_capped() {
        let first = llama_generation_progress(8);
        let later = llama_generation_progress(160);
        let much_later = llama_generation_progress(10_000);

        assert!(first > 60.0);
        assert!(later > first);
        assert_eq!(much_later, 95.0);
    }

    #[test]
    fn converts_little_endian_pcm_to_normalized_samples() {
        assert_eq!(
            pcm_s16le_bytes_to_f32(&[0x00, 0x80, 0x00, 0x00, 0xff, 0x7f]),
            vec![-1.0, 0.0, 32767.0 / 32768.0]
        );
    }

    #[test]
    fn resolves_ffmpeg_next_to_host_executable() {
        let executable = PathBuf::from("C:/sona/sona.exe");
        let ffmpeg = resolve_ffmpeg_sidecar_path_from_exe(&executable).unwrap();

        #[cfg(windows)]
        assert_eq!(ffmpeg, PathBuf::from("C:/sona/ffmpeg.exe"));
        #[cfg(not(windows))]
        assert_eq!(ffmpeg, PathBuf::from("C:/sona/ffmpeg"));
    }

    #[test]
    fn rejects_options_not_implemented_by_the_cpu_batch_adapter() {
        let mut plan = plan();
        plan.language = "zh".to_string();
        plan.hotwords = Some("Sona".to_string());
        plan.gpu_acceleration = Some("cuda".to_string());

        let error = validate_supported_options(&plan).unwrap_err();

        assert_eq!(error.kind, AsrPortErrorKind::Unsupported);
        assert!(error.message.contains("language override"));
        assert!(error.message.contains("hotwords"));
        assert!(error.message.contains("GPU acceleration"));
    }

    #[test]
    fn adapter_mismatch_uses_shared_error_contract() {
        let mut plan = plan();
        plan.engine = LocalAsrEngine::SherpaOnnx;

        let error = LlamaBatchTranscriptionJob::from_plan(plan).unwrap_err();
        assert_eq!(error.kind, AsrPortErrorKind::Unsupported);
        assert_eq!(
            error.message,
            "Local ASR adapter 'llama-cpp' cannot execute engine 'sherpa-onnx'."
        );
    }
}
