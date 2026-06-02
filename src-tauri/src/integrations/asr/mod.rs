use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use tauri::{AppHandle, State};

mod adapter;
mod batch;
mod error;
mod groq;
mod metrics;
mod mistral;
mod model_config;
mod postprocess;
pub mod sherpa_onnx;
pub mod state;
pub mod traits;
mod transcript;
mod types;
mod volcengine;

const BATCH_PROGRESS_EVENT: &str = "batch-progress";

fn recognizer_output_event(instance_id: &str) -> String {
    format!("recognizer-output-{instance_id}")
}

pub use adapter::LocalSherpaAdapter;
pub use batch::transcribe_batch_with_progress;
pub use error::SherpaError;
pub use metrics::{AsrInferenceMetric, AsrModelLoadMetric, AsrRuntimeMetricsSnapshot};
pub use model_config::ModelFileConfig;
pub(crate) use model_config::{Recognizer, RecognizerInner, build_model_config, load_vad};
pub use postprocess::TranscriptPostprocessor;
pub use state::AsrState;
pub(crate) use state::ModelConfigKey;
pub use traits::{AsrBatchProcessor, AsrProviderAdapter, AsrStreamingSession};
pub(crate) use transcript::{
    ensure_transcript_segment_timing, finalize_transcript_text, normalize_recognizer_text,
    synthesize_durations,
};
pub use types::{
    AsrEngine, AsrEngineConfig, AsrMode, AsrTranscriptionRequest, BatchSegmentationMode,
    BatchTranscriptionRequest, OnlineAsrProviderRequest, TranscriptNormalizationOptions,
    TranscriptPostprocessOptions, TranscriptSegment, TranscriptTextReplacementRule,
    TranscriptTextReplacementRuleSet, TranscriptTiming, TranscriptTimingLevel,
    TranscriptTimingSource, TranscriptTimingUnit, TranscriptUpdate, VolcengineDoubaoAsrConfig,
};

fn asr_adapters() -> &'static HashMap<&'static str, Arc<dyn AsrProviderAdapter>> {
    static ADAPTERS: OnceLock<HashMap<&'static str, Arc<dyn AsrProviderAdapter>>> = OnceLock::new();
    ADAPTERS.get_or_init(|| {
        let mut map: HashMap<&'static str, Arc<dyn AsrProviderAdapter>> = HashMap::new();
        let local = LocalSherpaAdapter;
        map.insert(local.provider_id(), Arc::new(local));
        let volcengine = volcengine::VolcengineAdapter;
        map.insert(volcengine.provider_id(), Arc::new(volcengine));
        let groq = groq::GroqWhisperAdapter;
        map.insert(groq.provider_id(), Arc::new(groq));
        let mistral = mistral::MistralVoxtralAdapter;
        map.insert(mistral.provider_id(), Arc::new(mistral));
        map
    })
}

fn get_provider_id(request: &AsrTranscriptionRequest) -> Result<&str, SherpaError> {
    match &request.engine_config {
        AsrEngineConfig::LocalSherpa { .. } => Ok("local_sherpa"),
        AsrEngineConfig::Online { provider } => Ok(provider.provider_id.as_str()),
    }
}

fn ensure_adapter(
    request: &AsrTranscriptionRequest,
) -> Result<Arc<dyn AsrProviderAdapter>, SherpaError> {
    let provider_id = get_provider_id(request)?;
    asr_adapters()
        .get(provider_id)
        .cloned()
        .ok_or_else(|| SherpaError::UnsupportedOnlineProvider {
            provider_id: provider_id.to_string(),
        })
}

/// Feed f32 audio samples from the hardware capture worker to the correct
/// ASR backend. Routes by the engine selected during `init_recognizer` so an
/// expected cloud recognizer cannot silently fall through to local Sherpa when
/// the cloud session is missing or failed.
pub async fn feed_audio_samples(
    _app: &AppHandle,
    state: &AsrState,
    instance_id: &str,
    samples: &[f32],
) -> Result<(), SherpaError> {
    let session = {
        let sessions = state.active_sessions.lock().await;
        sessions.get(instance_id).cloned().ok_or_else(|| {
            SherpaError::Generic(format!("ASR instance {} not found", instance_id))
        })?
    };
    session
        .feed_audio_samples(_app.clone(), state, instance_id, samples)
        .await
}

#[tauri::command]
pub async fn init_recognizer(
    state: State<'_, AsrState>,
    instance_id: String,
    asr_request: AsrTranscriptionRequest,
) -> Result<(), SherpaError> {
    let adapter = ensure_adapter(&asr_request)?;
    let session = adapter
        .create_streaming_session(&state, &instance_id, &asr_request)
        .await?;
    if let Some(session) = session {
        let mut active = state.active_sessions.lock().await;
        active.insert(instance_id.clone(), session);
        state
            .set_instance_engine(&instance_id, asr_request.engine())
            .await;
        Ok(())
    } else {
        Err(SherpaError::StreamingNotSupported {
            provider_id: get_provider_id(&asr_request)?.to_string(),
        })
    }
}

#[tauri::command]
pub async fn start_recognizer(
    app: AppHandle,
    state: State<'_, AsrState>,
    instance_id: String,
) -> Result<(), SherpaError> {
    let session = {
        let sessions = state.active_sessions.lock().await;
        sessions.get(&instance_id).cloned().ok_or_else(|| {
            SherpaError::Generic(format!("ASR instance {} not found", instance_id))
        })?
    };
    session.start(app, &state, &instance_id).await
}

#[tauri::command]
pub async fn stop_recognizer(
    state: State<'_, AsrState>,
    instance_id: String,
) -> Result<(), SherpaError> {
    let session = {
        let sessions = state.active_sessions.lock().await;
        sessions.get(&instance_id).cloned().ok_or_else(|| {
            SherpaError::Generic(format!("ASR instance {} not found", instance_id))
        })?
    };
    session.stop(&state, &instance_id).await
}

#[tauri::command]
pub async fn flush_recognizer(
    app: AppHandle,
    state: State<'_, AsrState>,
    instance_id: String,
) -> Result<(), SherpaError> {
    let session = {
        let sessions = state.active_sessions.lock().await;
        sessions.get(&instance_id).cloned().ok_or_else(|| {
            SherpaError::Generic(format!("ASR instance {} not found", instance_id))
        })?
    };
    session.flush(app, &state, &instance_id).await
}

#[tauri::command]
pub async fn feed_audio_chunk(
    app: AppHandle,
    state: State<'_, AsrState>,
    instance_id: String,
    samples: Vec<u8>,
) -> Result<(), SherpaError> {
    let session = {
        let sessions = state.active_sessions.lock().await;
        sessions.get(&instance_id).cloned().ok_or_else(|| {
            SherpaError::Generic(format!("ASR instance {} not found", instance_id))
        })?
    };
    session
        .feed_audio_chunk(app, &state, &instance_id, samples)
        .await
}

#[tauri::command]
pub async fn process_batch_file(
    app: AppHandle,
    state: State<'_, AsrState>,
    file_path: String,
    save_to_path: Option<String>,
    speaker_processing: Option<crate::integrations::speaker::SpeakerProcessingConfig>,
    asr_request: AsrTranscriptionRequest,
) -> Result<Vec<TranscriptSegment>, SherpaError> {
    let adapter = ensure_adapter(&asr_request)?;
    let processor = adapter
        .create_batch_processor(&asr_request)?
        .ok_or_else(|| {
            SherpaError::Generic(format!(
                "Batch mode not supported for provider {}",
                get_provider_id(&asr_request).unwrap_or("unknown")
            ))
        })?;
    processor
        .process_file(
            app,
            &state,
            file_path,
            save_to_path,
            asr_request,
            speaker_processing,
        )
        .await
}

#[tauri::command]
pub async fn get_asr_runtime_metrics(
    state: State<'_, AsrState>,
) -> Result<AsrRuntimeMetricsSnapshot, String> {
    Ok(state.metrics_snapshot().await)
}
