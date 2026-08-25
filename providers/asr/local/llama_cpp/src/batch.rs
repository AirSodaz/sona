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
const MODEL_TYPE_GRANITE_SPEECH: &str = "granite-speech";
const N_BATCH: i32 = 512;
const MAX_GENERATED_TOKENS: usize = 4096;
/// Qwen3-ASR GGUFs ship a 65536-token context, so a generous hotword budget
/// stays negligible against audio and generation tokens.
const QWEN3_ASR_HOTWORDS_MAX_CHARS: usize = 2048;
/// Granite Speech 4.1 caps text positions at 4096 while consuming ~10 audio
/// tokens per second, leaving only a few hundred prompt tokens for a
/// six-minute clip; keep keyword lists small.
const GRANITE_SPEECH_HOTWORDS_MAX_CHARS: usize = 256;

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
    model_type: String,
    /// Normalized hotword terms ready for the model-specific rendering.
    hotwords: Vec<String>,
    /// Trained Qwen3-ASR prefill name (`Some("Chinese")`, ...); `None`
    /// lets the model auto-detect.
    language_prefill: Option<&'static str>,
}

impl LlamaBatchTranscriptionJob {
    fn from_plan(plan: BatchTranscribePlan) -> Result<Self, AsrPortError> {
        if plan.engine != LocalAsrEngine::LlamaCpp {
            return Err(local_asr_engine_mismatch(
                LocalAsrEngine::LlamaCpp,
                plan.engine,
            ));
        }
        if !matches!(
            plan.model_type.as_str(),
            MODEL_TYPE_QWEN3_ASR | MODEL_TYPE_GRANITE_SPEECH
        ) {
            return Err(AsrPortError::new(
                AsrPortErrorKind::Unsupported,
                format!(
                    "llama.cpp batch ASR supports model types '{MODEL_TYPE_QWEN3_ASR}' and '{MODEL_TYPE_GRANITE_SPEECH}', got '{}'.",
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
        let options = validate_supported_options(&plan)?;

        let file_config = plan.file_config.as_ref().ok_or_else(|| {
            AsrPortError::new(
                AsrPortErrorKind::Model,
                "llama.cpp batch ASR models require model and mmproj file configuration.",
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
            model_type: plan.model_type,
            hotwords: options.hotwords,
            language_prefill: options.language_prefill,
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
                    "Failed to initialize llama.cpp ASR mmproj {}: {error}",
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

        let prompt = build_transcription_prompt(
            &self.model_type,
            &model,
            &self.hotwords,
            self.language_prefill,
        )?;
        let add_special = self.model_type != MODEL_TYPE_GRANITE_SPEECH;
        let chunks = mtmd
            .tokenize(
                MtmdInputText {
                    text: prompt,
                    add_special,
                    parse_special: true,
                },
                &[&audio],
            )
            .map_err(|error| {
                AsrPortError::new(
                    AsrPortErrorKind::Model,
                    format!("Failed to tokenize llama.cpp ASR audio prompt: {error}"),
                )
            })?;
        let n_past = chunks
            .eval_chunks(&mtmd, &context, 0, 0, N_BATCH, true)
            .map_err(|error| {
                AsrPortError::new(
                    AsrPortErrorKind::Runtime,
                    format!("Failed to evaluate llama.cpp ASR audio: {error}"),
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
                            format!(
                                "Failed to decode llama.cpp output token (id={}): {error}",
                                token.0
                            ),
                        )
                    })?,
            );
            generated_tokens = generated_tokens.saturating_add(1);

            if generated_tokens.is_multiple_of(8) {
                emit_partial_transcript(
                    observer.as_ref(),
                    &self.model_type,
                    self.language_prefill.is_some(),
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

        let text = parse_transcript_output(&self.model_type, &generated);
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
    model_type: &str,
    language_forced: bool,
    segment_id: &str,
    duration: f64,
    generated: &str,
    generated_tokens: usize,
) {
    let text = parse_partial_transcript_output(model_type, generated, language_forced);
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

/// Engine-ready forms derived from a [`BatchTranscribePlan`] after option
/// validation.
#[derive(Debug)]
struct ValidatedOptions {
    hotwords: Vec<String>,
    /// Trained Qwen3-ASR prefill name; `None` lets the model auto-detect.
    language_prefill: Option<&'static str>,
}

fn validate_supported_options(
    plan: &BatchTranscribePlan,
) -> Result<ValidatedOptions, AsrPortError> {
    let mut unsupported = Vec::new();
    if plan.save_to_path.is_some() {
        unsupported.push("save_wav");
    }
    if plan.enable_itn {
        unsupported.push("enable_itn");
    }
    if plan.punctuation_model.is_some() {
        unsupported.push("punctuation model");
    }
    if plan.vad_model.is_some() {
        unsupported.push("VAD model");
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

    if !unsupported.is_empty() {
        return Err(AsrPortError::new(
            AsrPortErrorKind::Unsupported,
            format!(
                "llama.cpp batch transcription does not yet support: {}.",
                unsupported.join(", ")
            ),
        ));
    }

    let language_prefill = match plan.model_type.as_str() {
        MODEL_TYPE_QWEN3_ASR => qwen3_asr_language(&plan.language)?,
        MODEL_TYPE_GRANITE_SPEECH => {
            if !plan.language.eq_ignore_ascii_case("auto") {
                return Err(AsrPortError::new(
                    AsrPortErrorKind::Unsupported,
                    format!(
                        "Granite Speech llama.cpp transcription detects language automatically and does not support '{}'.",
                        plan.language
                    ),
                ));
            }
            None
        }
        other => {
            return Err(AsrPortError::new(
                AsrPortErrorKind::Unsupported,
                format!("Unknown llama.cpp batch ASR model type '{other}'."),
            ));
        }
    };

    let hotwords = normalize_hotwords(
        plan.hotwords.as_deref().unwrap_or_default(),
        hotword_char_budget(plan.model_type.as_str()),
    );

    Ok(ValidatedOptions {
        hotwords,
        language_prefill,
    })
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

const GRANITE_SPEECH_TRANSCRIBE_PROMPT: &str =
    "transcribe the speech with proper punctuation and capitalization.";
/// Official keyword-biased ASR prompt from the Granite Speech model card.
/// Keyword runs use this documented pairing verbatim instead of appending
/// `Keywords:` to the punctuated variant.
const GRANITE_SPEECH_KEYWORD_PROMPT: &str = "transcribe the speech to text.";

fn build_transcription_prompt(
    model_type: &str,
    model: &LlamaModel,
    hotwords: &[String],
    language_prefill: Option<&str>,
) -> Result<String, AsrPortError> {
    match model_type {
        MODEL_TYPE_GRANITE_SPEECH => Ok(granite_speech_prompt(hotwords)),
        _ => qwen3_asr_prompt(model, hotwords, language_prefill),
    }
}

/// Granite Speech renders its embedded chat template literally
/// (`USER: ...\n ASSISTANT:`), so build it by hand: llama-cpp-2's heuristic
/// template engine rejects this GGUF, and the tokenizer sets
/// add_bos_token=false so the caller skips special-token prepending.
/// Hotwords ride the trained keyword-biasing suffix inside the same user
/// turn, joined with ", " exactly as documented.
fn granite_speech_prompt(hotwords: &[String]) -> String {
    let task = if hotwords.is_empty() {
        GRANITE_SPEECH_TRANSCRIBE_PROMPT
    } else {
        GRANITE_SPEECH_KEYWORD_PROMPT
    };
    let mut prompt = format!("USER: {}{}", mtmd_default_marker(), task);
    if !hotwords.is_empty() {
        prompt.push_str(" Keywords: ");
        prompt.push_str(&hotwords.join(", "));
    }
    prompt.push_str("\n ASSISTANT:");
    prompt
}

/// Qwen3-ASR consumes hotwords as background knowledge inside the ChatML
/// system message — the channel the model was trained on for context
/// biasing — and forces a language by prefilling
/// `language <Name><asr_text>` after the generation prompt.
fn qwen3_asr_prompt(
    model: &LlamaModel,
    hotwords: &[String],
    language_prefill: Option<&str>,
) -> Result<String, AsrPortError> {
    let message = |role: &str, content: String| {
        LlamaChatMessage::new(role.to_string(), content).map_err(|error| {
            AsrPortError::new(
                AsrPortErrorKind::Protocol,
                format!("Failed to construct Qwen3-ASR chat message: {error}"),
            )
        })
    };

    let mut messages = Vec::with_capacity(2);
    if !hotwords.is_empty() {
        messages.push(message("system", hotwords.join(" "))?);
    }
    messages.push(message("user", mtmd_default_marker().to_string())?);

    let template = model.chat_template(None).map_err(|error| {
        AsrPortError::new(
            AsrPortErrorKind::Model,
            format!("Qwen3-ASR GGUF does not provide a usable chat template: {error}"),
        )
    })?;
    let mut prompt = model
        .apply_chat_template(&template, &messages, true)
        .map_err(|error| {
            AsrPortError::new(
                AsrPortErrorKind::Protocol,
                format!("Failed to apply Qwen3-ASR chat template: {error}"),
            )
        })?;
    if let Some(language) = language_prefill {
        prompt.push_str("language ");
        prompt.push_str(language);
        prompt.push_str("<asr_text>");
    }
    Ok(prompt)
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
            format!("llama.cpp batch ASR file configuration is missing '{label}'."),
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
            format!(
                "llama.cpp batch ASR {label} file was not found: {}",
                path.display()
            ),
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

fn parse_transcript_output(model_type: &str, output: &str) -> String {
    if model_type == MODEL_TYPE_GRANITE_SPEECH {
        return normalize_recognizer_text(output.trim());
    }
    parse_qwen3_asr_output(output)
}

fn parse_partial_transcript_output(
    model_type: &str,
    output: &str,
    language_forced: bool,
) -> String {
    if model_type == MODEL_TYPE_GRANITE_SPEECH {
        return normalize_recognizer_text(output.trim());
    }
    parse_qwen3_asr_partial_output(output, language_forced)
}

fn parse_qwen3_asr_output(output: &str) -> String {
    let transcript = match output.split_once("<asr_text>") {
        Some((_, transcript)) => transcript,
        // Upstream llama.cpp (#26749): some builds leak the trained
        // `language Xxx` prefix without the <asr_text> tag.
        None => strip_leaked_language_prefix(output),
    };
    normalize_recognizer_text(transcript.trim())
}

fn parse_qwen3_asr_partial_output(output: &str, language_forced: bool) -> String {
    if let Some((_, transcript)) = output.split_once("<asr_text>") {
        return normalize_recognizer_text(transcript.trim());
    }
    // With a forced-language prefill the tag lives in the prompt rather than
    // the generated stream, so partials are plain text. Otherwise the
    // transcript starts only once the model emits the tag itself.
    if language_forced {
        return normalize_recognizer_text(output.trim());
    }
    String::new()
}

/// Strips a leading `language <CanonicalName>` prefix when the emitted name
/// matches a trained Qwen3-ASR language, keeping transcripts that begin with
/// an unrecognized word after "language".
fn strip_leaked_language_prefix(output: &str) -> &str {
    let Some(rest) = output.strip_prefix("language ") else {
        return output;
    };
    match rest.split_once(' ') {
        Some((name, tail)) if is_qwen3_asr_language_name(name) => tail,
        Some(_) => output,
        // A bare prefix decodes to silence, mirroring the trained
        // `language None<asr_text>` empty case.
        None => "",
    }
}

/// Splits the shared hotword string on ASCII commas and newlines, trims each
/// entry, drops sherpa-style ` :weight` suffixes (no llama.cpp model has a
/// weight concept), and truncates to the model's character budget.
fn normalize_hotwords(raw: &str, max_chars: usize) -> Vec<String> {
    let terms: Vec<String> = raw
        .split([',', '\n'])
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(strip_hotword_weight)
        .collect();
    truncate_hotwords(terms, max_chars)
}

/// Sherpa transducer syntax appends weights as `term :2.0`; a trailing
/// space-separated `:<number>` segment is stripped instead of leaking
/// literally into the prompt. Terms like `host:8080` survive untouched.
fn strip_hotword_weight(term: &str) -> String {
    match term.rsplit_once(' ') {
        Some((head, suffix)) if suffix.starts_with(':') && suffix[1..].parse::<f32>().is_ok() => {
            head.trim_end().to_string()
        }
        _ => term.to_string(),
    }
}

/// Greedily keeps whole terms while their separator-joined length fits the
/// budget; oversized lists lose their tail instead of degrading every term.
fn truncate_hotwords(mut terms: Vec<String>, max_chars: usize) -> Vec<String> {
    terms.retain(|term| !term.is_empty());
    let mut kept = Vec::new();
    let mut used = 0usize;
    for term in terms {
        let cost = term.chars().count() + usize::from(!kept.is_empty());
        if used + cost > max_chars {
            log::warn!(
                "Hotword list truncated to {max_chars} characters to protect the llama.cpp context budget."
            );
            break;
        }
        used += cost;
        kept.push(term);
    }
    kept
}

fn hotword_char_budget(model_type: &str) -> usize {
    if model_type == MODEL_TYPE_GRANITE_SPEECH {
        GRANITE_SPEECH_HOTWORDS_MAX_CHARS
    } else {
        QWEN3_ASR_HOTWORDS_MAX_CHARS
    }
}

/// Trained Qwen3-ASR language names paired with the ISO codes Sona's
/// selectors emit; the prefill expects the canonical names verbatim.
const QWEN3_ASR_LANGUAGES: &[(&str, &str)] = &[
    ("zh", "Chinese"),
    ("en", "English"),
    ("yue", "Cantonese"),
    ("ar", "Arabic"),
    ("de", "German"),
    ("fr", "French"),
    ("es", "Spanish"),
    ("pt", "Portuguese"),
    ("id", "Indonesian"),
    ("it", "Italian"),
    ("ko", "Korean"),
    ("ru", "Russian"),
    ("th", "Thai"),
    ("vi", "Vietnamese"),
    ("ja", "Japanese"),
    ("tr", "Turkish"),
    ("hi", "Hindi"),
    ("ms", "Malay"),
    ("nl", "Dutch"),
    ("sv", "Swedish"),
    ("da", "Danish"),
    ("fi", "Finnish"),
    ("pl", "Polish"),
    ("cs", "Czech"),
    ("fil", "Filipino"),
    ("fa", "Persian"),
    ("el", "Greek"),
    ("hu", "Hungarian"),
    ("mk", "Macedonian"),
    ("ro", "Romanian"),
];

/// Maps Sona's language value onto the trained prefill name. `None` means
/// auto-detect; unmapped values fail with typed feedback instead of being
/// silently ignored.
fn qwen3_asr_language(language: &str) -> Result<Option<&'static str>, AsrPortError> {
    let trimmed = language.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("auto") {
        return Ok(None);
    }
    let lowered = trimmed.to_ascii_lowercase();
    QWEN3_ASR_LANGUAGES
        .iter()
        .copied()
        .find(|(code, name)| *code == lowered || name.eq_ignore_ascii_case(trimmed))
        .map(|(_, name)| Some(name))
        .ok_or_else(|| {
            AsrPortError::new(
                AsrPortErrorKind::Unsupported,
                format!(
                    "Qwen3-ASR llama.cpp transcription supports language 'auto' or one of: {}; got '{trimmed}'.",
                    QWEN3_ASR_LANGUAGES
                        .iter()
                        .map(|(code, _)| *code)
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )
        })
}

fn is_qwen3_asr_language_name(candidate: &str) -> bool {
    QWEN3_ASR_LANGUAGES
        .iter()
        .any(|(_, name)| name.eq_ignore_ascii_case(candidate))
}

#[cfg(test)]
mod tests {
    use super::{
        LlamaBatchTranscriptionJob, MODEL_TYPE_GRANITE_SPEECH, MODEL_TYPE_QWEN3_ASR,
        granite_speech_prompt, llama_generation_progress, normalize_hotwords,
        parse_partial_transcript_output, parse_qwen3_asr_output, parse_qwen3_asr_partial_output,
        parse_transcript_output, pcm_s16le_bytes_to_f32, qwen3_asr_language,
        resolve_ffmpeg_sidecar_path_from_exe, truncate_hotwords, validate_supported_options,
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
        assert_eq!(
            parse_qwen3_asr_partial_output("language Chinese", false),
            ""
        );
        assert_eq!(
            parse_qwen3_asr_partial_output("language Chinese<asr_text>你好", false),
            "你好"
        );
    }

    #[test]
    fn forced_language_partials_are_plain_text() {
        assert_eq!(parse_qwen3_asr_partial_output("你好世界", true), "你好世界");
        assert_eq!(parse_qwen3_asr_partial_output("", true), "");
    }

    #[test]
    fn granite_speech_type_passes_the_model_type_guard() {
        let mut plan = plan();
        plan.model_type = MODEL_TYPE_GRANITE_SPEECH.to_string();

        // The guard passes; the job then fails on the missing input file,
        // proving model type was accepted by dispatch.
        let error = LlamaBatchTranscriptionJob::from_plan(plan).unwrap_err();

        assert_eq!(error.kind, AsrPortErrorKind::InvalidRequest);
        assert!(
            error
                .message
                .contains("Input file must be an existing file")
        );
    }

    #[test]
    fn unknown_model_types_name_the_supported_set() {
        let mut plan = plan();
        plan.model_type = "whisper-large".to_string();

        let error = LlamaBatchTranscriptionJob::from_plan(plan).unwrap_err();

        assert_eq!(error.kind, AsrPortErrorKind::Unsupported);
        assert!(error.message.contains(MODEL_TYPE_QWEN3_ASR));
        assert!(error.message.contains(MODEL_TYPE_GRANITE_SPEECH));
    }

    #[test]
    fn output_parsing_dispatches_by_model_type() {
        assert_eq!(
            parse_transcript_output(MODEL_TYPE_GRANITE_SPEECH, "  Hello, world!  "),
            "Hello, world!"
        );
        assert_eq!(
            parse_transcript_output(MODEL_TYPE_QWEN3_ASR, "<asr_text>你好"),
            "你好"
        );
        assert_eq!(
            parse_partial_transcript_output(MODEL_TYPE_GRANITE_SPEECH, " Partial text ", false),
            "Partial text"
        );
        assert_eq!(
            parse_partial_transcript_output(MODEL_TYPE_QWEN3_ASR, "no marker yet", false),
            ""
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
        plan.save_to_path = Some(PathBuf::from("copy.wav"));
        plan.enable_itn = true;
        plan.gpu_acceleration = Some("cuda".to_string());

        let error = validate_supported_options(&plan).unwrap_err();

        assert_eq!(error.kind, AsrPortErrorKind::Unsupported);
        assert!(error.message.contains("save_wav"));
        assert!(error.message.contains("enable_itn"));
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

    #[test]
    fn qwen3_accepts_mapped_languages_and_derives_prefills() {
        let mut plan = plan();
        plan.language = "zh".to_string();
        let options = validate_supported_options(&plan).unwrap();
        assert_eq!(options.language_prefill, Some("Chinese"));

        plan.language = "auto".to_string();
        let options = validate_supported_options(&plan).unwrap();
        assert_eq!(options.language_prefill, None);
    }

    #[test]
    fn qwen3_rejects_unmapped_languages() {
        let mut plan = plan();
        plan.language = "tlh".to_string();

        let error = validate_supported_options(&plan).unwrap_err();

        assert_eq!(error.kind, AsrPortErrorKind::Unsupported);
        assert!(error.message.contains("'tlh'"));
    }

    #[test]
    fn granite_speech_rejects_language_override() {
        let mut plan = plan();
        plan.model_type = MODEL_TYPE_GRANITE_SPEECH.to_string();
        plan.language = "en".to_string();

        let error = validate_supported_options(&plan).unwrap_err();

        assert_eq!(error.kind, AsrPortErrorKind::Unsupported);
        assert!(error.message.contains("detects language automatically"));
    }

    #[test]
    fn qwen3_language_mapping_covers_selector_codes() {
        assert_eq!(qwen3_asr_language("auto").unwrap(), None);
        assert_eq!(qwen3_asr_language("").unwrap(), None);
        assert_eq!(qwen3_asr_language(" yue ").unwrap(), Some("Cantonese"));
        assert_eq!(qwen3_asr_language("Japanese").unwrap(), Some("Japanese"));
    }

    #[test]
    fn hotwords_normalize_and_drop_weights() {
        assert_eq!(
            normalize_hotwords("Sona, offline ASR\n\nWhisper :2.0,,", 2048),
            vec![
                "Sona".to_string(),
                "offline ASR".to_string(),
                "Whisper".to_string()
            ]
        );
        assert!(normalize_hotwords(" , ,, ", 2048).is_empty());
        // Weights only strip when separated by a space, keeping
        // host:port-like terms intact.
        assert_eq!(
            normalize_hotwords("TCP:8080", 2048),
            vec!["TCP:8080".to_string()]
        );
    }

    #[test]
    fn hotwords_truncate_to_the_model_budget() {
        let terms = vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()];
        // "alpha beta" fits 10 chars; gamma would exceed.
        assert_eq!(
            truncate_hotwords(terms.clone(), 10),
            vec!["alpha".to_string(), "beta".to_string()]
        );
        // A budget smaller than the first term yields no usable keywords.
        assert!(truncate_hotwords(terms, 4).is_empty());
    }

    #[test]
    fn granite_speech_prompts_match_documented_templates() {
        assert_eq!(
            granite_speech_prompt(&[]),
            "USER: <__media__>transcribe the speech with proper punctuation and capitalization.\n ASSISTANT:"
        );
        assert_eq!(
            granite_speech_prompt(&["Acme".to_string(), "TCP".to_string()]),
            "USER: <__media__>transcribe the speech to text. Keywords: Acme, TCP\n ASSISTANT:"
        );
    }

    #[test]
    fn qwen3_final_output_strips_leaked_language_prefix_without_tag() {
        assert_eq!(
            parse_qwen3_asr_output("language English hello world"),
            "hello world"
        );
        assert_eq!(parse_qwen3_asr_output("language German"), "");
        assert_eq!(
            parse_qwen3_asr_output("language barriers exist everywhere"),
            "language barriers exist everywhere"
        );
    }
}
