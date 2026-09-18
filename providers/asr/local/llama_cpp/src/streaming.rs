use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::mtmd::{MtmdBitmap, MtmdContext, MtmdInputText};
use llama_cpp_2::sampling::LlamaSampler;
use sona_core::ports::asr::{
    AsrEngineConfig, AsrMode, AsrPortError, AsrPortErrorKind, AsrRuntimeObserver,
    AsrStreamingSession, AsrTranscriptionRequest, LocalAsrEngine, StreamingAsrFactoryPort,
    StreamingInferenceSpec, TranscriptNormalizationOptions, TranscriptPostprocessOptions,
    local_asr_engine_mismatch, validate_local_asr_mode,
};
use sona_core::ports::punctuation::{PunctuationEngineSet, load_configured_punctuation};
use sona_core::ports::vad::{VadDetectionOptions, VadEngineSet};
use sona_core::transcription::pseudo_streaming::{
    DecodeStage, PseudoStreamDecodeResult, PseudoStreamDecoder, PseudoStreamingSession,
    PseudoStreamingSessionConfig,
};

use crate::batch::{
    GpuOffload, MAX_GENERATED_TOKENS, MODEL_TYPE_QWEN3_ASR, N_BATCH, QWEN3_ASR_HOTWORDS_MAX_CHARS,
    backend, gpu_backend_available, init_inference, normalize_hotwords, parse_qwen3_asr_output,
    parse_qwen3_asr_partial_output, qwen3_asr_language, qwen3_asr_prompt, resolve_auto_gpu_offload,
    resolve_gpu_offload, resolve_required_model_file,
};

const MAX_PARTIAL_GENERATED_TOKENS: usize = 256;

/// Engine-ready configuration parsed from a streaming request.
#[derive(Debug)]
pub(crate) struct ValidatedStreamingRequest {
    pub(crate) model_file: PathBuf,
    pub(crate) mmproj_file: PathBuf,
    pub(crate) num_threads: i32,
    pub(crate) language_prefill: Option<&'static str>,
    pub(crate) hotwords: Vec<String>,
    pub(crate) gpu_offload: GpuOffload,
    pub(crate) vad_model: Option<String>,
    pub(crate) vad_buffer: f32,
    pub(crate) punctuation_model: Option<String>,
    pub(crate) normalization_options: TranscriptNormalizationOptions,
    pub(crate) postprocess_options: TranscriptPostprocessOptions,
    pub(crate) initial_refresh_rate_ms: Option<u32>,
}

pub(crate) fn validate_streaming_request(
    request: &AsrTranscriptionRequest,
) -> Result<ValidatedStreamingRequest, AsrPortError> {
    validate_local_asr_mode(request, AsrMode::Streaming)?;

    let (
        local_engine,
        model_id,
        model_path,
        num_threads,
        punctuation_model,
        vad_model,
        vad_buffer,
        model_type,
        file_config,
        gpu_acceleration,
        initial_refresh_rate_ms,
    ) = match &request.engine_config {
        AsrEngineConfig::Local {
            local_engine,
            model_id,
            model_path,
            num_threads,
            punctuation_model,
            vad_model,
            vad_buffer,
            model_type,
            file_config,
            gpu_acceleration,
            initial_refresh_rate_ms,
            ..
        } => (
            *local_engine,
            model_id.clone(),
            model_path.clone(),
            *num_threads,
            punctuation_model.clone(),
            vad_model.clone(),
            *vad_buffer,
            model_type.clone(),
            file_config.clone(),
            gpu_acceleration.clone(),
            *initial_refresh_rate_ms,
        ),
        _ => {
            return Err(AsrPortError::invalid_request(
                "Expected Local engine config for llama.cpp streaming transcription",
            ));
        }
    };

    if local_engine != LocalAsrEngine::LlamaCpp {
        return Err(local_asr_engine_mismatch(
            LocalAsrEngine::LlamaCpp,
            local_engine,
        ));
    }

    if model_type != MODEL_TYPE_QWEN3_ASR {
        return Err(AsrPortError::new(
            AsrPortErrorKind::Unsupported,
            format!("Unknown llama.cpp streaming ASR model type '{model_type}'."),
        ));
    }

    let language_prefill = qwen3_asr_language(&request.language)?;
    let hotwords = normalize_hotwords(
        request.hotwords.as_deref().unwrap_or_default(),
        QWEN3_ASR_HOTWORDS_MAX_CHARS,
    );

    let gpu_requested = resolve_gpu_offload(gpu_acceleration.as_deref())?;
    let gpu_offload = match gpu_requested {
        GpuOffload::Auto => resolve_auto_gpu_offload(gpu_backend_available()),
        other => other,
    };

    let model_dir = Path::new(&model_path);
    let config = file_config.as_ref().as_ref().cloned().unwrap_or_default();
    let model_file = resolve_required_model_file(model_dir, &config, false)?;
    let mmproj_file = resolve_required_model_file(model_dir, &config, true)?;

    let initial_refresh_rate_ms = initial_refresh_rate_ms.or_else(|| {
        model_id
            .as_deref()
            .and_then(sona_core::models::preset_models::find_preset_model)
            .and_then(|m| m.resolved_rules().initial_refresh_rate_ms)
    });

    Ok(ValidatedStreamingRequest {
        model_file,
        mmproj_file,
        num_threads,
        language_prefill,
        hotwords,
        gpu_offload,
        vad_model,
        vad_buffer,
        punctuation_model,
        normalization_options: request.normalization_options,
        postprocess_options: request.postprocess_options.clone(),
        initial_refresh_rate_ms,
    })
}

/// Decoder implementation adapting llama.cpp Qwen3-ASR to [`PseudoStreamDecoder`].
pub struct Qwen3PseudoStreamDecoder {
    backend: &'static LlamaBackend,
    model: Arc<LlamaModel>,
    mtmd: Arc<Mutex<MtmdContext>>,
    prompt: String,
    language_forced: bool,
    num_threads: i32,
}

impl Qwen3PseudoStreamDecoder {
    pub fn new(
        backend: &'static LlamaBackend,
        model: Arc<LlamaModel>,
        mtmd: Arc<Mutex<MtmdContext>>,
        prompt: String,
        language_forced: bool,
        num_threads: i32,
    ) -> Self {
        Self {
            backend,
            model,
            mtmd,
            prompt,
            language_forced,
            num_threads,
        }
    }
}

impl PseudoStreamDecoder for Qwen3PseudoStreamDecoder {
    fn decode(
        &self,
        audio_samples: &[f32],
        stage: DecodeStage,
    ) -> Result<Option<PseudoStreamDecodeResult>, AsrPortError> {
        if audio_samples.len() < 800 {
            return Ok(None);
        }

        let audio = match MtmdBitmap::from_audio_data(audio_samples) {
            Ok(audio) => audio,
            Err(error) => {
                if stage == DecodeStage::Partial {
                    log::debug!("Skipping partial decode due to audio conversion error: {error}");
                    return Ok(None);
                }
                return Err(AsrPortError::new(
                    AsrPortErrorKind::InvalidRequest,
                    format!("Failed to create llama.cpp audio input: {error}"),
                ));
            }
        };

        if !audio.is_audio() {
            if stage == DecodeStage::Partial {
                return Ok(None);
            }
            return Err(AsrPortError::invalid_request(
                "Input is not a supported audio file.",
            ));
        }

        let context_size = NonZeroU32::new(self.model.n_ctx_train().max(32_768));
        let context_params = LlamaContextParams::default()
            .with_n_ctx(context_size)
            .with_n_batch(N_BATCH as u32)
            .with_n_threads(self.num_threads)
            .with_n_threads_batch(self.num_threads);
        let mut context = self
            .model
            .new_context(self.backend, context_params)
            .map_err(|error| {
                AsrPortError::new(
                    AsrPortErrorKind::Runtime,
                    format!("Failed to create llama.cpp inference context: {error}"),
                )
            })?;

        let n_past = {
            let mtmd = self.mtmd.lock().map_err(|_| {
                AsrPortError::new(
                    AsrPortErrorKind::Runtime,
                    "llama.cpp multimodal context lock was poisoned.",
                )
            })?;
            let chunks = mtmd
                .tokenize(
                    MtmdInputText {
                        text: self.prompt.clone(),
                        add_special: true,
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
            chunks
                .eval_chunks(&mtmd, &context, 0, 0, N_BATCH, true)
                .map_err(|error| {
                    AsrPortError::new(
                        AsrPortErrorKind::Runtime,
                        format!("Failed to evaluate llama.cpp ASR audio: {error}"),
                    )
                })?
        };

        let mut sampler = LlamaSampler::greedy();
        let mut decoder = encoding_rs::UTF_8.new_decoder();
        let mut generated = String::new();
        let available = context.n_ctx().saturating_sub(n_past.max(0) as u32) as usize;
        let max_tokens = match stage {
            DecodeStage::Partial => MAX_PARTIAL_GENERATED_TOKENS,
            DecodeStage::Final => MAX_GENERATED_TOKENS,
        };
        let generation_limit = max_tokens.min(available);
        let generation_end =
            n_past.saturating_add(i32::try_from(generation_limit).unwrap_or(i32::MAX));

        for token_position in n_past..generation_end {
            let token = sampler.sample(&context, -1);
            if self.model.is_eog_token(token) {
                break;
            }
            sampler.accept(token);
            let piece = self
                .model
                .token_to_piece(token, &mut decoder, false, None)
                .map_err(|error| {
                    AsrPortError::new(
                        AsrPortErrorKind::Protocol,
                        format!(
                            "Failed to decode llama.cpp output token (id={}): {error}",
                            token.0
                        ),
                    )
                })?;
            generated.push_str(&piece);

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

        let text = match stage {
            DecodeStage::Partial => {
                parse_qwen3_asr_partial_output(&generated, self.language_forced)
            }
            DecodeStage::Final => parse_qwen3_asr_output(&generated),
        };

        if text.is_empty() {
            Ok(None)
        } else {
            Ok(Some(PseudoStreamDecodeResult {
                text,
                tokens: None,
                timestamps: None,
            }))
        }
    }
}

/// Factory producing streaming ASR sessions backed by llama.cpp and [`PseudoStreamingSession`].
#[derive(Clone)]
pub struct LlamaCppStreamingFactory {
    vad_engines: VadEngineSet,
    punct_engines: PunctuationEngineSet,
}

impl LlamaCppStreamingFactory {
    pub fn new(vad_engines: VadEngineSet, punct_engines: PunctuationEngineSet) -> Self {
        Self {
            vad_engines,
            punct_engines,
        }
    }
}

#[async_trait]
impl StreamingAsrFactoryPort for LlamaCppStreamingFactory {
    async fn prepare(&self, spec: &StreamingInferenceSpec) -> Result<(), AsrPortError> {
        let request = spec.engine_request();
        let validated = validate_streaming_request(&request)?;
        let enable_gpu = validated.gpu_offload == GpuOffload::Enabled;

        tokio::task::spawn_blocking(move || {
            let backend = backend()?;
            let _ = init_inference(
                backend,
                &validated.model_file,
                &validated.mmproj_file,
                validated.num_threads,
                enable_gpu,
            )?;
            Ok(())
        })
        .await
        .map_err(|error| {
            AsrPortError::runtime(format!(
                "Failed to prepare llama.cpp streaming resources: {error}"
            ))
        })?
    }

    async fn create(
        &self,
        pipeline_id: &str,
        spec: &StreamingInferenceSpec,
        observer: Arc<dyn AsrRuntimeObserver>,
    ) -> Result<Arc<dyn AsrStreamingSession>, AsrPortError> {
        let request = spec.engine_request();
        let validated = validate_streaming_request(&request)?;
        let enable_gpu = validated.gpu_offload == GpuOffload::Enabled;
        let vad_engines = self.vad_engines.clone();
        let punct_engines = self.punct_engines.clone();
        let pipeline_id = pipeline_id.to_string();

        tokio::task::spawn_blocking(move || {
            let backend = backend()?;
            let (model, mtmd) = init_inference(
                backend,
                &validated.model_file,
                &validated.mmproj_file,
                validated.num_threads,
                enable_gpu,
            )?;

            let prompt = qwen3_asr_prompt(&model, &validated.hotwords, validated.language_prefill)?;
            let language_forced = validated.language_prefill.is_some();

            let vad_path = validated
                .vad_model
                .as_deref()
                .filter(|p| !p.trim().is_empty())
                .map(PathBuf::from)
                .ok_or_else(|| {
                    AsrPortError::new(
                        AsrPortErrorKind::InvalidRequest,
                        "VAD model path is required for llama.cpp streaming transcription",
                    )
                })?;

            let vad_options = VadDetectionOptions {
                model_path: vad_path,
                threshold: 0.35,
                min_silence_duration: 0.5,
                min_speech_duration: 0.25,
                buffer_seconds: if validated.vad_buffer > 0.0 {
                    validated.vad_buffer
                } else {
                    5.0
                },
            };
            let vad = vad_engines.create_stream_detector(&vad_options)?;

            let punctuation = load_configured_punctuation(
                &punct_engines,
                validated.punctuation_model.as_deref().map(Path::new),
            )?;

            let decoder = Arc::new(Qwen3PseudoStreamDecoder::new(
                backend,
                model,
                Arc::new(Mutex::new(mtmd)),
                prompt,
                language_forced,
                validated.num_threads,
            ));

            let session = PseudoStreamingSession::new(PseudoStreamingSessionConfig {
                instance_id: pipeline_id,
                decoder,
                vad,
                punctuation,
                observer,
                normalization_options: validated.normalization_options,
                postprocess_options: validated.postprocess_options,
                initial_refresh_rate_ms: validated.initial_refresh_rate_ms,
            })?;

            Ok(Arc::new(session) as Arc<dyn AsrStreamingSession>)
        })
        .await
        .map_err(|error| {
            AsrPortError::runtime(format!(
                "Failed to create llama.cpp streaming session: {error}"
            ))
        })?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sona_core::models::config::ModelFileConfig;
    use sona_core::ports::asr::BatchSegmentationMode;

    fn create_test_model_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sona-llama-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("model.gguf"), b"dummy").unwrap();
        std::fs::write(dir.join("mmproj.gguf"), b"dummy").unwrap();
        dir
    }

    fn sample_streaming_request(model_dir: &Path) -> AsrTranscriptionRequest {
        AsrTranscriptionRequest {
            mode: AsrMode::Streaming,
            language: "zh".to_string(),
            enable_itn: false,
            normalization_options: TranscriptNormalizationOptions::default(),
            postprocess_options: TranscriptPostprocessOptions::default(),
            hotwords: Some("Sona, AI".to_string()),
            speaker_processing: None,
            engine_config: AsrEngineConfig::Local {
                local_engine: LocalAsrEngine::LlamaCpp,
                model_id: Some("qwen3-asr-0.6b-q8-gguf".to_string()),
                model_path: model_dir.to_string_lossy().to_string(),
                num_threads: 4,
                punctuation_model: None,
                alignment_model: None,
                vad_model: Some("silero_vad.onnx".to_string()),
                vad_buffer: 0.5,
                batch_segmentation_mode: BatchSegmentationMode::Vad,
                model_type: MODEL_TYPE_QWEN3_ASR.to_string(),
                file_config: Box::new(Some(ModelFileConfig {
                    model: Some("model.gguf".to_string()),
                    mmproj: Some("mmproj.gguf".to_string()),
                    ..ModelFileConfig::default()
                })),
                gpu_acceleration: Some("auto".to_string()),
                initial_refresh_rate_ms: None,
                ffmpeg_path: None,
            },
        }
    }

    #[test]
    fn validate_streaming_request_extracts_expected_fields() {
        let dir = create_test_model_dir();
        let request = sample_streaming_request(&dir);
        let validated = validate_streaming_request(&request).expect("validation should succeed");

        assert_eq!(validated.num_threads, 4);
        assert_eq!(validated.language_prefill, Some("Chinese"));
        assert_eq!(validated.hotwords, vec!["Sona", "AI"]);
        assert_eq!(validated.vad_model.as_deref(), Some("silero_vad.onnx"));
        assert_eq!(validated.vad_buffer, 0.5);
        assert_eq!(validated.model_file, dir.join("model.gguf"));
        assert_eq!(validated.mmproj_file, dir.join("mmproj.gguf"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn validate_streaming_request_rejects_batch_mode() {
        let dir = create_test_model_dir();
        let mut request = sample_streaming_request(&dir);
        request.mode = AsrMode::Batch;

        let error = validate_streaming_request(&request).expect_err("should reject batch mode");
        assert_eq!(error.kind, AsrPortErrorKind::InvalidRequest);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn validate_streaming_request_rejects_wrong_engine() {
        let dir = create_test_model_dir();
        let mut request = sample_streaming_request(&dir);
        if let AsrEngineConfig::Local {
            ref mut local_engine,
            ..
        } = request.engine_config
        {
            *local_engine = LocalAsrEngine::SherpaOnnx;
        }

        let error = validate_streaming_request(&request).expect_err("should reject wrong engine");
        assert_eq!(error.kind, AsrPortErrorKind::Unsupported);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn validate_streaming_request_rejects_wrong_model_type() {
        let dir = create_test_model_dir();
        let mut request = sample_streaming_request(&dir);
        if let AsrEngineConfig::Local {
            ref mut model_type, ..
        } = request.engine_config
        {
            *model_type = "whisper".to_string();
        }

        let error =
            validate_streaming_request(&request).expect_err("should reject wrong model type");
        assert_eq!(error.kind, AsrPortErrorKind::Unsupported);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn validate_streaming_request_rejects_unsupported_language() {
        let dir = create_test_model_dir();
        let mut request = sample_streaming_request(&dir);
        request.language = "klingon".to_string();

        let error =
            validate_streaming_request(&request).expect_err("should reject unsupported language");
        assert_eq!(error.kind, AsrPortErrorKind::Unsupported);
        let _ = std::fs::remove_dir_all(dir);
    }
}
