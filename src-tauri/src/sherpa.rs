use log::trace;
use tauri::{AppHandle, State};

mod adapter;
mod batch;
mod error;
mod groq;
mod metrics;
mod mistral;
mod model_config;
pub mod online;
mod online_traits;
mod postprocess;
mod runtime;
mod state;
mod transcript;
mod types;
mod volcengine;

const BATCH_PROGRESS_EVENT: &str = "batch-progress";

fn recognizer_output_event(instance_id: &str) -> String {
    format!("recognizer-output-{instance_id}")
}

pub use adapter::{AsrEngineAdapter, LocalSherpaAdapter};
pub use batch::transcribe_batch_with_progress;
pub use error::SherpaError;
pub use metrics::{AsrInferenceMetric, AsrModelLoadMetric, AsrRuntimeMetricsSnapshot};
pub use model_config::ModelFileConfig;
pub(crate) use model_config::{Recognizer, RecognizerInner, build_model_config, load_vad};
pub use online_traits::{OnlineAsrProviderAdapter, OnlineBatchProcessor, OnlineStreamingSession};
pub use postprocess::TranscriptPostprocessor;
pub use state::SherpaState;
pub(crate) use state::{ModelConfigKey, OfflineState};
pub(crate) use transcript::{
    ensure_transcript_segment_timing, finalize_transcript_text, normalize_recognizer_text,
    synthesize_durations,
};
pub use types::{
    AsrEngine, AsrMode, AsrTranscriptionRequest, BatchSegmentationMode, BatchTranscriptionRequest,
    OnlineAsrProviderRequest, TranscriptNormalizationOptions, TranscriptPostprocessOptions,
    TranscriptSegment, TranscriptTextReplacementRule, TranscriptTextReplacementRuleSet,
    TranscriptTiming, TranscriptTimingLevel, TranscriptTimingSource, TranscriptTimingUnit,
    TranscriptUpdate, VolcengineDoubaoAsrConfig,
};

async fn route_engine(state: &SherpaState, instance_id: &str) -> Result<AsrEngine, SherpaError> {
    state.instance_engine(instance_id).await.ok_or_else(|| {
        SherpaError::Generic(format!(
            "ASR engine not configured for instance: {}",
            instance_id
        ))
    })
}

#[derive(Default)]
struct LegacyLocalSherpaTransportRequest {
    model_path: Option<String>,
    num_threads: Option<i32>,
    enable_itn: Option<bool>,
    language: Option<String>,
    punctuation_model: Option<String>,
    vad_model: Option<String>,
    vad_buffer: Option<f32>,
    model_type: Option<String>,
    file_config: Option<ModelFileConfig>,
    hotwords: Option<String>,
    normalization_options: Option<TranscriptNormalizationOptions>,
    postprocess_options: Option<TranscriptPostprocessOptions>,
    gpu_acceleration: Option<String>,
}

impl LegacyLocalSherpaTransportRequest {
    fn into_asr_request(
        self,
        mode: AsrMode,
        command_name: &str,
    ) -> Result<AsrTranscriptionRequest, String> {
        let mut missing = Vec::new();
        if self.model_path.is_none() {
            missing.push("modelPath");
        }
        if self.num_threads.is_none() {
            missing.push("numThreads");
        }
        if self.enable_itn.is_none() {
            missing.push("enableItn");
        }
        if self.language.is_none() {
            missing.push("language");
        }
        if self.vad_buffer.is_none() {
            missing.push("vadBuffer");
        }
        if self.model_type.is_none() {
            missing.push("modelType");
        }

        if !missing.is_empty() {
            return Err(format!(
                "Missing asrRequest for {command_name}; legacy flat ASR fields are incomplete: {}",
                missing.join(", ")
            ));
        }

        Ok(AsrTranscriptionRequest::local_sherpa(
            mode,
            self.model_path.expect("checked modelPath"),
            self.num_threads.expect("checked numThreads"),
            self.enable_itn.expect("checked enableItn"),
            self.language.expect("checked language"),
            self.punctuation_model,
            self.vad_model,
            self.vad_buffer.expect("checked vadBuffer"),
            self.model_type.expect("checked modelType"),
            self.file_config,
            self.hotwords,
            self.normalization_options.unwrap_or_default(),
            self.postprocess_options.unwrap_or_default(),
            self.gpu_acceleration.clone(),
        ))
    }
}

fn resolve_transport_asr_request(
    asr_request: Option<AsrTranscriptionRequest>,
    legacy_request: LegacyLocalSherpaTransportRequest,
    mode: AsrMode,
    command_name: &str,
) -> Result<AsrTranscriptionRequest, String> {
    match asr_request {
        Some(request) => Ok(request),
        None => legacy_request.into_asr_request(mode, command_name),
    }
}

/// Feed f32 audio samples from the hardware capture worker to the correct
/// ASR backend. Routes by the engine selected during `init_recognizer` so an
/// expected cloud recognizer cannot silently fall through to local Sherpa when
/// the cloud session is missing or failed.
pub async fn feed_audio_samples<R: tauri::Runtime>(
    app: &AppHandle<R>,
    state: &SherpaState,
    instance_id: &str,
    samples: &[f32],
) -> Result<(), SherpaError> {
    match route_engine(state, instance_id).await? {
        AsrEngine::Online => online::feed_audio_samples_impl(state, instance_id, samples).await,
        AsrEngine::LocalSherpa => runtime::feed_audio_samples(app, state, instance_id, samples)
            .await
            .map_err(SherpaError::from),
    }
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn init_recognizer(
    state: State<'_, SherpaState>,
    instance_id: String,
    model_path: Option<String>,
    num_threads: Option<i32>,
    enable_itn: Option<bool>,
    language: Option<String>,
    punctuation_model: Option<String>,
    vad_model: Option<String>,
    vad_buffer: Option<f32>,
    model_type: Option<String>,
    file_config: Option<ModelFileConfig>,
    hotwords: Option<String>,
    normalization_options: Option<TranscriptNormalizationOptions>,
    postprocess_options: Option<TranscriptPostprocessOptions>,
    gpu_acceleration: Option<String>,
    asr_request: Option<AsrTranscriptionRequest>,
) -> Result<(), SherpaError> {
    let request = resolve_transport_asr_request(
        asr_request,
        LegacyLocalSherpaTransportRequest {
            model_path,
            num_threads,
            enable_itn,
            language,
            punctuation_model,
            vad_model,
            vad_buffer,
            model_type,
            file_config,
            hotwords,
            normalization_options,
            postprocess_options,
            gpu_acceleration,
        },
        AsrMode::Streaming,
        "init_recognizer",
    )
    .map_err(SherpaError::from)?;
    match request.engine {
        AsrEngine::LocalSherpa => {
            LocalSherpaAdapter::ensure_mode(&request, AsrMode::Streaming)
                .map_err(SherpaError::from)?;
            let init_result = runtime::init_recognizer_impl(
                state.clone(),
                instance_id.clone(),
                request.model_path,
                request.num_threads,
                request.enable_itn,
                request.language,
                request.punctuation_model,
                request.vad_model,
                request.vad_buffer,
                request.model_type,
                request.file_config,
                request.hotwords,
                Some(request.normalization_options),
                Some(request.postprocess_options),
                request.gpu_acceleration.clone(),
            )
            .await
            .map_err(SherpaError::from);
            if init_result.is_ok() {
                state
                    .set_instance_engine(&instance_id, AsrEngine::LocalSherpa)
                    .await;
            }
            init_result
        }
        AsrEngine::Online => {
            let init_result =
                online::init_streaming_recognizer_impl(state.clone(), instance_id.clone(), request)
                    .await;
            if init_result.is_ok() {
                state
                    .set_instance_engine(&instance_id, AsrEngine::Online)
                    .await;
            }
            init_result
        }
    }
}

#[tauri::command]
pub async fn start_recognizer(
    app: AppHandle,
    state: State<'_, SherpaState>,
    instance_id: String,
) -> Result<(), SherpaError> {
    match route_engine(&state, &instance_id).await? {
        AsrEngine::Online => online::start_streaming_recognizer_impl(app, state, instance_id).await,
        AsrEngine::LocalSherpa => runtime::start_recognizer_impl(state, instance_id)
            .await
            .map_err(SherpaError::from),
    }
}

#[tauri::command]
pub async fn stop_recognizer(
    state: State<'_, SherpaState>,
    instance_id: String,
) -> Result<(), SherpaError> {
    match route_engine(&state, &instance_id).await? {
        AsrEngine::Online => online::stop_streaming_recognizer_impl(state, instance_id).await,
        AsrEngine::LocalSherpa => runtime::stop_recognizer_impl(state, instance_id)
            .await
            .map_err(SherpaError::from),
    }
}

#[tauri::command]
pub async fn flush_recognizer(
    app: AppHandle,
    state: State<'_, SherpaState>,
    instance_id: String,
) -> Result<(), SherpaError> {
    match route_engine(&state, &instance_id).await? {
        AsrEngine::Online => online::flush_streaming_recognizer_impl(app, state, instance_id).await,
        AsrEngine::LocalSherpa => runtime::flush_recognizer_impl(app, state, instance_id)
            .await
            .map_err(SherpaError::from),
    }
}

#[tauri::command]
pub async fn feed_audio_chunk(
    app: AppHandle,
    state: State<'_, SherpaState>,
    instance_id: String,
    samples: Vec<u8>,
) -> Result<(), SherpaError> {
    trace!(
        "feed_audio_chunk called with id: {}, samples bytes: {}",
        instance_id,
        samples.len()
    );
    match route_engine(&state, &instance_id).await? {
        AsrEngine::Online => online::feed_audio_chunk_impl(app, state, instance_id, samples).await,
        AsrEngine::LocalSherpa => runtime::feed_audio_chunk_impl(app, state, instance_id, samples)
            .await
            .map_err(SherpaError::from),
    }
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn process_batch_file(
    app: AppHandle,
    state: State<'_, SherpaState>,
    file_path: String,
    save_to_path: Option<String>,
    model_path: Option<String>,
    num_threads: Option<i32>,
    enable_itn: Option<bool>,
    language: Option<String>,
    punctuation_model: Option<String>,
    vad_model: Option<String>,
    vad_buffer: Option<f32>,
    model_type: Option<String>,
    file_config: Option<ModelFileConfig>,
    hotwords: Option<String>,
    speaker_processing: Option<crate::speaker::SpeakerProcessingConfig>,
    normalization_options: Option<TranscriptNormalizationOptions>,
    postprocess_options: Option<TranscriptPostprocessOptions>,
    gpu_acceleration: Option<String>,
    asr_request: Option<AsrTranscriptionRequest>,
) -> Result<Vec<TranscriptSegment>, SherpaError> {
    let request = resolve_transport_asr_request(
        asr_request,
        LegacyLocalSherpaTransportRequest {
            model_path,
            num_threads,
            enable_itn,
            language,
            punctuation_model,
            vad_model,
            vad_buffer,
            model_type,
            file_config,
            hotwords,
            normalization_options,
            postprocess_options,
            gpu_acceleration,
        },
        AsrMode::Offline,
        "process_batch_file",
    )
    .map_err(SherpaError::from)?;
    match request.engine {
        AsrEngine::LocalSherpa => {
            let adapter = LocalSherpaAdapter;
            let batch_request = adapter
                .batch_request(file_path, save_to_path, request, speaker_processing)
                .map_err(SherpaError::from)?;
            batch::process_batch_request_impl(app, state.inner(), batch_request)
                .await
                .map_err(SherpaError::from)
        }
        AsrEngine::Online => {
            online::process_batch_file_impl(app, state.inner(), file_path, request).await
        }
    }
}

#[tauri::command]
pub async fn get_asr_runtime_metrics(
    state: State<'_, SherpaState>,
) -> Result<AsrRuntimeMetricsSnapshot, String> {
    Ok(state.metrics_snapshot().await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sherpa::types::{TranscriptNormalizationOptions, TranscriptPostprocessOptions};

    #[tokio::test]
    async fn route_engine_uses_selected_online_engine_even_without_active_session() {
        let state = SherpaState::new();
        state.set_instance_engine("record", AsrEngine::Online).await;

        assert_eq!(
            route_engine(&state, "record").await.unwrap(),
            AsrEngine::Online
        );
        assert!(!state.has_online_session("record").await);
    }

    #[tokio::test]
    async fn route_engine_returns_error_for_unconfigured_instances() {
        let state = SherpaState::new();

        assert!(route_engine(&state, "record").await.is_err());
    }

    fn local_request(mode: AsrMode, model_path: &str) -> AsrTranscriptionRequest {
        AsrTranscriptionRequest::local_sherpa(
            mode,
            model_path.to_string(),
            4,
            true,
            "auto".to_string(),
            None,
            None,
            5.0,
            "sensevoice".to_string(),
            None,
            None,
            TranscriptNormalizationOptions::default(),
            TranscriptPostprocessOptions::default(),
            None,
        )
    }

    #[test]
    fn transport_asr_request_prefers_canonical_asr_request() {
        let request = resolve_transport_asr_request(
            Some(local_request(AsrMode::Streaming, "C:/models/canonical")),
            LegacyLocalSherpaTransportRequest::default(),
            AsrMode::Streaming,
            "init_recognizer",
        )
        .expect("canonical asrRequest should be sufficient");

        assert_eq!(request.model_path, "C:/models/canonical");
        assert_eq!(request.mode, AsrMode::Streaming);
    }

    #[test]
    fn transport_asr_request_adapts_complete_legacy_flat_fields() {
        let request = resolve_transport_asr_request(
            None,
            LegacyLocalSherpaTransportRequest {
                model_path: Some("C:/models/legacy".to_string()),
                num_threads: Some(2),
                enable_itn: Some(false),
                language: Some("zh".to_string()),
                punctuation_model: Some("C:/models/punct".to_string()),
                vad_model: None,
                vad_buffer: Some(3.0),
                model_type: Some("sensevoice".to_string()),
                file_config: None,
                hotwords: Some("Sona".to_string()),
                normalization_options: Some(TranscriptNormalizationOptions {
                    enable_timeline: true,
                }),
                postprocess_options: None,
                gpu_acceleration: None,
            },
            AsrMode::Offline,
            "process_batch_file",
        )
        .expect("complete legacy fields should be adapted");

        assert_eq!(request.engine, AsrEngine::LocalSherpa);
        assert_eq!(request.mode, AsrMode::Offline);
        assert_eq!(request.model_path, "C:/models/legacy");
        assert_eq!(request.num_threads, 2);
        assert!(!request.enable_itn);
        assert_eq!(request.language, "zh");
        assert_eq!(
            request.punctuation_model.as_deref(),
            Some("C:/models/punct")
        );
        assert_eq!(request.hotwords.as_deref(), Some("Sona"));
        assert!(request.normalization_options.enable_timeline);
    }

    #[test]
    fn transport_asr_request_rejects_incomplete_legacy_flat_fields() {
        let error = resolve_transport_asr_request(
            None,
            LegacyLocalSherpaTransportRequest {
                model_path: Some("C:/models/legacy".to_string()),
                ..LegacyLocalSherpaTransportRequest::default()
            },
            AsrMode::Streaming,
            "init_recognizer",
        )
        .expect_err("missing canonical and incomplete legacy request should fail");

        assert!(error.contains("Missing asrRequest for init_recognizer"));
        assert!(error.contains("numThreads"));
        assert!(error.contains("modelType"));
    }
}
