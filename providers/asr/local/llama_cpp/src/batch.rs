use std::collections::HashMap;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, LazyLock, Mutex, OnceLock};

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
    AsrPortError, AsrPortErrorKind, BatchSegmentationMode, BatchTranscriberPort,
    BatchTranscriptionObserver, LocalAsrEngine, NoopBatchTranscriptionObserver,
    local_asr_engine_mismatch, pcm_s16le_bytes_to_f32, resolve_ffmpeg_sidecar_path,
};
use sona_core::ports::punctuation::{
    PunctuationEngineSet, apply_optional_punctuation, load_configured_punctuation,
};
use sona_core::ports::vad::{VadDetectionOptions, VadEngineSet};
use sona_core::transcription::runtime::BatchTranscribePlan;
use sona_core::transcription::segmentation::{
    AudioSegment, BATCH_SEGMENTATION_SAMPLE_RATE, segment_batch_audio,
};
use sona_core::transcription::transcript::{
    TranscriptSegment, TranscriptUpdate, ensure_transcript_segment_timing,
    normalize_recognizer_text,
};

const MODEL_TYPE_QWEN3_ASR: &str = "qwen3-asr";
const N_BATCH: i32 = 512;
const MAX_GENERATED_TOKENS: usize = 4096;
/// Qwen3-ASR GGUFs ship a 65536-token context, so a generous hotword budget
/// stays negligible against audio and generation tokens.
const QWEN3_ASR_HOTWORDS_MAX_CHARS: usize = 2048;

static BACKEND: OnceLock<Result<LlamaBackend, String>> = OnceLock::new();
/// Cache key: canonical model path plus the resolved GPU layer count.
type ModelCacheKey = (PathBuf, u32);
static MODEL_CACHE: OnceLock<Mutex<HashMap<ModelCacheKey, Arc<LlamaModel>>>> = OnceLock::new();

#[derive(Clone, Default)]
pub struct LlamaBatchAsrAdapter {
    vad_engines: VadEngineSet,
    punct_engines: PunctuationEngineSet,
}

impl LlamaBatchAsrAdapter {
    pub fn new(vad_engines: VadEngineSet, punct_engines: PunctuationEngineSet) -> Self {
        Self {
            vad_engines,
            punct_engines,
        }
    }
}

#[async_trait]
impl BatchTranscriberPort for LlamaBatchAsrAdapter {
    async fn transcribe(
        &self,
        plan: BatchTranscribePlan,
    ) -> Result<Vec<TranscriptSegment>, AsrPortError> {
        let job =
            LlamaBatchTranscriptionJob::from_plan(plan, &self.vad_engines, &self.punct_engines)?;
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
        let job =
            LlamaBatchTranscriptionJob::from_plan(plan, &self.vad_engines, &self.punct_engines)?;
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

#[derive(Debug, Clone)]
struct LlamaBatchTranscriptionJob {
    input_path: PathBuf,
    model_path: PathBuf,
    mmproj_path: PathBuf,
    num_threads: i32,
    /// Normalized hotword terms ready for the model-specific rendering.
    hotwords: Vec<String>,
    /// Trained Qwen3-ASR prefill name (`Some("Chinese")`, ...); `None`
    /// lets the model auto-detect.
    language_prefill: Option<&'static str>,
    /// Resolved GPU offload intent for this run.
    gpu_offload: GpuOffload,
    vad_model: Option<PathBuf>,
    vad_buffer: f32,
    batch_segmentation_mode: BatchSegmentationMode,
    punctuation_model: Option<PathBuf>,
    vad_engines: VadEngineSet,
    punct_engines: PunctuationEngineSet,
}

impl LlamaBatchTranscriptionJob {
    fn from_plan(
        plan: BatchTranscribePlan,
        vad_engines: &VadEngineSet,
        punct_engines: &PunctuationEngineSet,
    ) -> Result<Self, AsrPortError> {
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
                    "llama.cpp batch ASR supports model type '{MODEL_TYPE_QWEN3_ASR}', got '{}'.",
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
            hotwords: options.hotwords,
            language_prefill: options.language_prefill,
            gpu_offload: options.gpu_offload,
            vad_model: plan.vad_model.map(PathBuf::from),
            vad_buffer: plan.vad_buffer,
            batch_segmentation_mode: plan.batch_segmentation_mode,
            punctuation_model: plan.punctuation_model.map(PathBuf::from),
            vad_engines: vad_engines.clone(),
            punct_engines: punct_engines.clone(),
        })
    }

    fn transcribe(
        self,
        observer: Arc<dyn BatchTranscriptionObserver>,
    ) -> Result<Vec<TranscriptSegment>, AsrPortError> {
        let backend = backend()?;
        let requested = match self.gpu_offload {
            GpuOffload::Auto => resolve_auto_gpu_offload(gpu_backend_available()),
            explicit => explicit,
        };
        let enable_gpu = requested == GpuOffload::Enabled;
        let (model, mtmd) = match init_inference(
            backend,
            &self.model_path,
            &self.mmproj_path,
            self.num_threads,
            enable_gpu,
        ) {
            Ok(pair) => pair,
            Err(error) if enable_gpu => {
                log::warn!("llama.cpp GPU offload failed; retrying on CPU: {error}");
                init_inference(
                    backend,
                    &self.model_path,
                    &self.mmproj_path,
                    self.num_threads,
                    false,
                )?
            }
            Err(error) => return Err(error),
        };
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

        let audio_segments = self.plan_audio_segments(&samples, sample_rate)?;
        let punctuation =
            load_configured_punctuation(&self.punct_engines, self.punctuation_model.as_deref())?;
        let prompt = qwen3_asr_prompt(&model, &self.hotwords, self.language_prefill)?;
        let add_special = true;
        let language_forced = self.language_prefill.is_some();
        let segment_total = audio_segments.len();

        let mut results = Vec::new();
        for (segment_index, segment) in audio_segments.into_iter().enumerate() {
            let (start_sample, end_sample) = segment_bounds(&segment, sample_rate, samples.len());
            if end_sample <= start_sample {
                continue;
            }
            let segment_samples = samples[start_sample..end_sample].to_vec();
            if let Some(mut segment_result) = transcribe_segment(
                backend,
                &model,
                &mtmd,
                &prompt,
                add_special,
                language_forced,
                self.num_threads,
                &segment_samples,
                &segment,
                segment_index,
                segment_total,
                observer.as_ref(),
            )? {
                segment_result.text =
                    apply_optional_punctuation(punctuation.as_deref(), &segment_result.text);
                if !segment_result.text.is_empty() {
                    observer.on_transcript_update(&TranscriptUpdate {
                        remove_ids: Vec::new(),
                        upsert_segments: vec![segment_result.clone()],
                    });
                    results.push(segment_result);
                }
            }
        }

        observer.on_progress(100.0);
        Ok(results)
    }

    /// Splits the decoded audio into inference segments.
    ///
    /// Detection always runs on the 16 kHz timeline VAD engines expect; when
    /// the model requests a different rate the segments are expressed as
    /// seconds and mapped back onto the model-rate samples by the caller.
    fn plan_audio_segments(
        &self,
        samples: &[f32],
        sample_rate: u32,
    ) -> Result<Vec<AudioSegment>, AsrPortError> {
        if self.batch_segmentation_mode == BatchSegmentationMode::Whole {
            return Ok(vec![AudioSegment {
                samples: Vec::new(),
                start_time: 0.0,
                duration: samples.len() as f32 / (sample_rate.max(1) as f32),
            }]);
        }

        let vad_engine = self.vad_engines.resolve(self.vad_model.as_deref());
        let Some(engine) = vad_engine else {
            return Ok(segment_batch_audio(
                samples,
                sample_rate,
                BatchSegmentationMode::Vad,
                None,
                &self.vad_options(),
            ));
        };

        if sample_rate == BATCH_SEGMENTATION_SAMPLE_RATE {
            return Ok(segment_batch_audio(
                samples,
                sample_rate,
                BatchSegmentationMode::Vad,
                Some(engine.as_ref()),
                &self.vad_options(),
            ));
        }

        let detection_samples =
            decode_audio_input(&self.input_path, BATCH_SEGMENTATION_SAMPLE_RATE)?;
        Ok(segment_batch_audio(
            &detection_samples,
            BATCH_SEGMENTATION_SAMPLE_RATE,
            BatchSegmentationMode::Vad,
            Some(engine.as_ref()),
            &self.vad_options(),
        ))
    }

    fn vad_options(&self) -> VadDetectionOptions {
        let mut options =
            VadDetectionOptions::batch_defaults(self.vad_model.as_deref().unwrap_or(Path::new("")));
        options.buffer_seconds = self.vad_buffer;
        options
    }
}

/// Maps a second-based [`AudioSegment`] back onto model-rate sample indices.
fn segment_bounds(
    segment: &AudioSegment,
    sample_rate: u32,
    total_samples: usize,
) -> (usize, usize) {
    let rate = sample_rate.max(1) as f32;
    let start = ((segment.start_time * rate).round() as usize).min(total_samples);
    let end = ((segment.end_time() * rate).round() as usize).clamp(start, total_samples);
    (start, end)
}

#[allow(clippy::too_many_arguments)]
fn transcribe_segment(
    backend: &'static LlamaBackend,
    model: &Arc<LlamaModel>,
    mtmd: &MtmdContext,
    prompt: &str,
    add_special: bool,
    language_forced: bool,
    num_threads: i32,
    samples: &[f32],
    segment: &AudioSegment,
    segment_index: usize,
    segment_total: usize,
    observer: &dyn BatchTranscriptionObserver,
) -> Result<Option<TranscriptSegment>, AsrPortError> {
    let audio = MtmdBitmap::from_audio_data(samples).map_err(|error| {
        AsrPortError::new(
            AsrPortErrorKind::InvalidRequest,
            format!("Failed to create llama.cpp audio input: {error}"),
        )
    })?;
    if !audio.is_audio() {
        return Err(AsrPortError::invalid_request(
            "Input is not a supported audio file.",
        ));
    }

    let context_size = NonZeroU32::new(model.n_ctx_train().max(32_768));
    let context_params = LlamaContextParams::default()
        .with_n_ctx(context_size)
        .with_n_batch(N_BATCH as u32)
        .with_n_threads(num_threads)
        .with_n_threads_batch(num_threads);
    let mut context = model
        .new_context(backend, context_params)
        .map_err(|error| {
            AsrPortError::new(
                AsrPortErrorKind::Runtime,
                format!("Failed to create llama.cpp inference context: {error}"),
            )
        })?;

    let chunks = mtmd
        .tokenize(
            MtmdInputText {
                text: prompt.to_string(),
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
        .eval_chunks(mtmd, &context, 0, 0, N_BATCH, true)
        .map_err(|error| {
            AsrPortError::new(
                AsrPortErrorKind::Runtime,
                format!("Failed to evaluate llama.cpp ASR audio: {error}"),
            )
        })?;
    observer.on_progress(audio_ingest_progress(segment_index, segment_total));

    let mut sampler = LlamaSampler::greedy();
    let mut decoder = encoding_rs::UTF_8.new_decoder();
    let mut generated = String::new();
    let segment_id = uuid::Uuid::new_v4().to_string();
    let mut generated_tokens = 0usize;
    let available = context.n_ctx().saturating_sub(n_past.max(0) as u32) as usize;
    let generation_limit = MAX_GENERATED_TOKENS.min(available);
    let generation_end = n_past.saturating_add(i32::try_from(generation_limit).unwrap_or(i32::MAX));
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
                observer,
                language_forced,
                &segment_id,
                segment,
                &generated,
                generated_tokens,
                segment_index,
                segment_total,
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
        return Ok(None);
    }
    let mut segment_result = TranscriptSegment {
        id: segment_id,
        text,
        start: f64::from(segment.start_time),
        end: f64::from(segment.end_time()),
        is_final: true,
        timing: None,
        tokens: None,
        timestamps: None,
        durations: None,
        translation: None,
        speaker: None,
        speaker_attribution: None,
    };
    ensure_transcript_segment_timing(&mut segment_result);
    Ok(Some(segment_result))
}

#[allow(clippy::too_many_arguments)]
fn emit_partial_transcript(
    observer: &dyn BatchTranscriptionObserver,
    language_forced: bool,
    segment_id: &str,
    segment: &AudioSegment,
    generated: &str,
    generated_tokens: usize,
    segment_index: usize,
    segment_total: usize,
) {
    let text = parse_qwen3_asr_partial_output(generated, language_forced);
    if text.is_empty() {
        return;
    }

    let mut partial = TranscriptSegment {
        id: segment_id.to_string(),
        text,
        start: f64::from(segment.start_time),
        end: f64::from(segment.end_time()),
        is_final: false,
        timing: None,
        tokens: None,
        timestamps: None,
        durations: None,
        translation: None,
        speaker: None,
        speaker_attribution: None,
    };
    ensure_transcript_segment_timing(&mut partial);
    observer.on_transcript_update(&TranscriptUpdate {
        remove_ids: Vec::new(),
        upsert_segments: vec![partial],
    });
    observer.on_progress(llama_generation_progress(
        generated_tokens,
        segment_index,
        segment_total,
    ));
}

/// Audio-ingest progress spans 10%..60% across segments; generation then owns
/// 60%..95% through [`llama_generation_progress`].
fn audio_ingest_progress(segment_index: usize, segment_total: usize) -> f32 {
    const INGEST_START: f32 = 10.0;
    const INGEST_SPAN: f32 = 50.0;
    let total = segment_total.max(1) as f32;
    INGEST_START + INGEST_SPAN * (segment_index.min(segment_total) as f32 + 1.0) / total
}

fn llama_generation_progress(
    generated_tokens: usize,
    segment_index: usize,
    segment_total: usize,
) -> f32 {
    const GENERATION_START: f32 = 60.0;
    const GENERATION_SPAN: f32 = 35.0;
    const TOKEN_SCALE: f32 = 160.0;

    let total = segment_total.max(1) as f32;
    let completed = segment_index.min(segment_total.max(1) - 1) as f32;
    let completion_curve = 1.0 - (-(generated_tokens as f32) / TOKEN_SCALE).exp();
    (GENERATION_START + GENERATION_SPAN * (completed + completion_curve) / total).min(95.0)
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

/// Engine-ready forms derived from a [`BatchTranscribePlan`] after option
/// validation.
#[derive(Debug)]
struct ValidatedOptions {
    hotwords: Vec<String>,
    /// Trained Qwen3-ASR prefill name; `None` lets the model auto-detect.
    language_prefill: Option<&'static str>,
    /// Resolved GPU offload intent for this run.
    gpu_offload: GpuOffload,
}

/// GPU offload request resolved from the engine-neutral `gpu_acceleration`
/// value. Supported backends are Metal (macOS) and Vulkan (Windows, Linux);
/// `cuda` is accepted as an alias for compatible builds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GpuOffload {
    Disabled,
    Enabled,
    /// Decide at model-load time from the devices the linked runtime exposes.
    Auto,
}

/// Whether the linked ggml runtime registered a non-CPU device — a bundled
/// Metal/Vulkan/CUDA backend backed by a usable driver. CPU-only builds always
/// report `false`.
pub(crate) fn gpu_backend_available() -> bool {
    static GPU_BACKEND_AVAILABLE: LazyLock<bool> =
        LazyLock::new(|| LlamaModelParams::default().devices().len() > 1);
    *GPU_BACKEND_AVAILABLE
}

/// Full-offload layer count; every supported model fits comfortably in the
/// VRAM implied by any present accelerator.
const GPU_OFFLOAD_ALL_LAYERS: u32 = u32::MAX;

fn resolve_gpu_offload(value: Option<&str>) -> Result<GpuOffload, AsrPortError> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        None | Some("cpu") => Ok(GpuOffload::Disabled),
        Some("auto") => Ok(GpuOffload::Auto),
        Some("cuda") | Some("metal") | Some("vulkan") => Ok(GpuOffload::Enabled),
        Some(other) => Err(AsrPortError::new(
            AsrPortErrorKind::Unsupported,
            format!(
                "llama.cpp batch transcription supports gpu_acceleration 'auto', 'cpu', 'cuda', 'metal', or 'vulkan'; got '{other}'."
            ),
        )),
    }
}

fn resolve_auto_gpu_offload(gpu_present: bool) -> GpuOffload {
    if gpu_present {
        GpuOffload::Enabled
    } else {
        GpuOffload::Disabled
    }
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
    if plan.speaker_processing.is_some() {
        unsupported.push("speaker processing");
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

    let gpu_requested = resolve_gpu_offload(plan.gpu_acceleration.as_deref())?;

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
        other => {
            return Err(AsrPortError::new(
                AsrPortErrorKind::Unsupported,
                format!("Unknown llama.cpp batch ASR model type '{other}'."),
            ));
        }
    };

    let hotwords = normalize_hotwords(
        plan.hotwords.as_deref().unwrap_or_default(),
        QWEN3_ASR_HOTWORDS_MAX_CHARS,
    );

    Ok(ValidatedOptions {
        hotwords,
        language_prefill,
        gpu_offload: gpu_requested,
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

/// Loads the model and multimodal projector. `n_gpu_layers` is explicit so
/// CPU runs stay on CPU even when a GPU backend is registered
/// (`LlamaModelParams` defaults to auto-offload).
fn init_inference(
    backend: &'static LlamaBackend,
    model_path: &Path,
    mmproj_path: &Path,
    num_threads: i32,
    enable_gpu: bool,
) -> Result<(Arc<LlamaModel>, MtmdContext), AsrPortError> {
    let n_gpu_layers = if enable_gpu {
        GPU_OFFLOAD_ALL_LAYERS
    } else {
        0
    };
    let model = load_model(backend, model_path, n_gpu_layers)?;
    let mtmd_params = MtmdContextParams {
        use_gpu: enable_gpu,
        n_threads: num_threads,
        ..MtmdContextParams::default()
    };
    let mmproj = path_to_str(mmproj_path, "mmproj")?;
    let mtmd = MtmdContext::init_from_file(mmproj, &model, &mtmd_params).map_err(|error| {
        AsrPortError::new(
            AsrPortErrorKind::Model,
            format!(
                "Failed to initialize llama.cpp ASR mmproj {}: {error}",
                mmproj_path.display()
            ),
        )
    })?;
    Ok((model, mtmd))
}

fn load_model(
    backend: &LlamaBackend,
    model_path: &Path,
    n_gpu_layers: u32,
) -> Result<Arc<LlamaModel>, AsrPortError> {
    let canonical_path = model_path.canonicalize().map_err(|error| {
        AsrPortError::new(
            AsrPortErrorKind::FileSystem,
            format!(
                "Failed to resolve llama.cpp model path {}: {error}",
                model_path.display()
            ),
        )
    })?;
    // The layer count participates in the key so switching between CPU and
    // GPU runs never reuses a model loaded for the other mode.
    let cache_key = (canonical_path, n_gpu_layers);
    let cache = MODEL_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut cache = cache.lock().map_err(|_| {
        AsrPortError::new(
            AsrPortErrorKind::Runtime,
            "llama.cpp model cache lock was poisoned.",
        )
    })?;
    if let Some(model) = cache.get(&cache_key) {
        return Ok(Arc::clone(model));
    }

    // An explicit count is required: `LlamaModelParams::default()` carries
    // n_gpu_layers=-1, which auto-offloads as soon as any GPU backend is
    // registered — silently ignoring an explicit 'cpu' request.
    let params = LlamaModelParams::default().with_n_gpu_layers(n_gpu_layers);
    let model = Arc::new(
        LlamaModel::load_from_file(backend, &cache_key.0, &params).map_err(|error| {
            AsrPortError::new(
                AsrPortErrorKind::Model,
                format!(
                    "Failed to load llama.cpp model {}: {error}",
                    cache_key.0.display()
                ),
            )
        })?,
    );
    cache.insert(cache_key, Arc::clone(&model));
    Ok(model)
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
        GpuOffload, LlamaBatchTranscriptionJob, MODEL_TYPE_QWEN3_ASR, audio_ingest_progress,
        llama_generation_progress, normalize_hotwords, parse_qwen3_asr_output,
        parse_qwen3_asr_partial_output, qwen3_asr_language, resolve_auto_gpu_offload,
        resolve_gpu_offload, segment_bounds, truncate_hotwords, validate_supported_options,
    };
    use sona_core::export::ExportFormat;
    use sona_core::ports::asr::{AsrPortErrorKind, LocalAsrEngine};
    use sona_core::ports::punctuation::PunctuationEngineSet;
    use sona_core::ports::vad::VadEngineSet;
    use sona_core::transcription::runtime::{BatchTranscribePlan, OutputTarget};
    use sona_core::transcription::segmentation::AudioSegment;
    use std::path::PathBuf;

    fn empty_vad_engines() -> VadEngineSet {
        VadEngineSet::empty()
    }

    fn empty_punct_engines() -> PunctuationEngineSet {
        PunctuationEngineSet::empty()
    }

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
    fn vad_configuration_is_accepted_for_segmented_batch_runs() {
        let mut plan = plan();
        let input = std::env::temp_dir().join(format!(
            "sona-llama-vad-accept-{}.wav",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&input, b"not-a-real-wav").unwrap();
        plan.input_path = input.clone();
        plan.vad_model = Some("/models/silero-vad".to_string());
        plan.batch_segmentation_mode = sona_core::ports::asr::BatchSegmentationMode::Vad;

        // VAD options pass validation; the job then fails on the missing
        // model/mmproj file configuration instead.
        let error = LlamaBatchTranscriptionJob::from_plan(
            plan,
            &empty_vad_engines(),
            &empty_punct_engines(),
        )
        .unwrap_err();

        assert_eq!(error.kind, AsrPortErrorKind::Model);
        assert!(error.message.contains("file configuration"));
        std::fs::remove_file(input).unwrap();
    }

    #[test]
    fn punctuation_configuration_is_accepted_for_batch_runs() {
        let mut plan = plan();
        let input = std::env::temp_dir().join(format!(
            "sona-llama-punct-accept-{}.wav",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&input, b"not-a-real-wav").unwrap();
        plan.input_path = input.clone();
        plan.punctuation_model = Some("/models/punct".to_string());

        // Punctuation options pass validation; the job then fails on the
        // missing model/mmproj file configuration instead.
        let error = LlamaBatchTranscriptionJob::from_plan(
            plan,
            &empty_vad_engines(),
            &empty_punct_engines(),
        )
        .unwrap_err();

        assert_eq!(error.kind, AsrPortErrorKind::Model);
        assert!(error.message.contains("file configuration"));
        std::fs::remove_file(input).unwrap();
    }

    #[test]
    fn unknown_model_types_name_the_supported_set() {
        let mut plan = plan();
        plan.model_type = "whisper-large".to_string();

        let error = LlamaBatchTranscriptionJob::from_plan(
            plan,
            &empty_vad_engines(),
            &empty_punct_engines(),
        )
        .unwrap_err();

        assert_eq!(error.kind, AsrPortErrorKind::Unsupported);
        assert!(error.message.contains(MODEL_TYPE_QWEN3_ASR));
    }

    #[test]
    fn output_parsing_handles_qwen3_asr_markers() {
        assert_eq!(parse_qwen3_asr_output("<asr_text>你好"), "你好");
        assert_eq!(parse_qwen3_asr_partial_output("no marker yet", false), "");
        assert_eq!(
            parse_qwen3_asr_partial_output("<asr_text>你好", false),
            "你好"
        );
    }
    #[test]
    fn token_generation_progress_is_monotonic_and_capped() {
        let first = llama_generation_progress(8, 0, 1);
        let later = llama_generation_progress(160, 0, 1);
        let much_later = llama_generation_progress(10_000, 0, 1);

        assert!(first > 60.0);
        assert!(later > first);
        assert_eq!(much_later, 95.0);
    }

    #[test]
    fn generation_and_ingest_progress_span_across_segments() {
        let curve = 1.0 - (-(8f32) / 160.0).exp();
        let single = llama_generation_progress(8, 0, 1);
        let first_of_two = llama_generation_progress(8, 0, 2);
        let second_of_two = llama_generation_progress(8, 1, 2);

        assert_eq!(single, 60.0 + 35.0 * curve);
        assert_eq!(first_of_two, 60.0 + 35.0 * curve / 2.0);
        assert_eq!(second_of_two, 60.0 + 35.0 * (1.0 + curve) / 2.0);

        let ingest_first = audio_ingest_progress(0, 2);
        let ingest_last = audio_ingest_progress(1, 2);
        assert_eq!(ingest_first, 35.0);
        assert_eq!(ingest_last, 60.0);
    }

    #[test]
    fn segment_bounds_map_seconds_onto_model_rate_samples() {
        let segment = AudioSegment {
            samples: Vec::new(),
            start_time: 1.0,
            duration: 2.0,
        };

        assert_eq!(segment_bounds(&segment, 16_000, 100_000), (16_000, 48_000));
        // A 44.1 kHz source maps through the same seconds.
        assert_eq!(segment_bounds(&segment, 44_100, 200_000), (44_100, 132_300));
        // Bounds clamp to the available sample range.
        assert_eq!(segment_bounds(&segment, 16_000, 20_000), (16_000, 20_000));
    }

    #[test]
    fn rejects_options_not_implemented_by_the_cpu_batch_adapter() {
        let mut plan = plan();
        plan.save_to_path = Some(PathBuf::from("copy.wav"));
        plan.enable_itn = true;

        let error = validate_supported_options(&plan).unwrap_err();

        assert_eq!(error.kind, AsrPortErrorKind::Unsupported);
        assert!(error.message.contains("save_wav"));
        assert!(error.message.contains("enable_itn"));
    }

    #[test]
    fn adapter_mismatch_uses_shared_error_contract() {
        let mut plan = plan();
        plan.engine = LocalAsrEngine::SherpaOnnx;

        let error = LlamaBatchTranscriptionJob::from_plan(
            plan,
            &empty_vad_engines(),
            &empty_punct_engines(),
        )
        .unwrap_err();
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

    #[test]
    fn gpu_offload_mapping_covers_supported_values() {
        assert_eq!(resolve_gpu_offload(None).unwrap(), GpuOffload::Disabled);
        assert_eq!(
            resolve_gpu_offload(Some("cpu")).unwrap(),
            GpuOffload::Disabled
        );
        assert_eq!(resolve_gpu_offload(Some("auto")).unwrap(), GpuOffload::Auto);
        assert_eq!(
            resolve_gpu_offload(Some("cuda")).unwrap(),
            GpuOffload::Enabled
        );
        assert_eq!(
            resolve_gpu_offload(Some("metal")).unwrap(),
            GpuOffload::Enabled
        );
        assert_eq!(
            resolve_gpu_offload(Some(" vulkan ")).unwrap(),
            GpuOffload::Enabled
        );
        assert_eq!(resolve_gpu_offload(Some("")).unwrap(), GpuOffload::Disabled);

        let error = resolve_gpu_offload(Some("directml")).unwrap_err();
        assert_eq!(error.kind, AsrPortErrorKind::Unsupported);
        assert!(error.message.contains("'directml'"));
    }

    #[test]
    fn auto_gpu_offload_follows_the_runtime_probe() {
        assert_eq!(resolve_auto_gpu_offload(true), GpuOffload::Enabled);
        assert_eq!(resolve_auto_gpu_offload(false), GpuOffload::Disabled);
    }

    #[test]
    fn validated_options_carry_the_gpu_intent() {
        let mut plan = plan();
        plan.gpu_acceleration = Some("vulkan".to_string());
        let options = validate_supported_options(&plan).unwrap();
        assert_eq!(options.gpu_offload, GpuOffload::Enabled);
    }
}
