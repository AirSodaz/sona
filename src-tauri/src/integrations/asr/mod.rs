use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use tauri::AppHandle;

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

pub use crate::core::asr_metrics::{
    AsrInferenceMetric, AsrModelLoadMetric, AsrRuntimeMetricsSnapshot,
};
pub use crate::core::model_config::ModelFileConfig;
pub use adapter::LocalSherpaAdapter;
pub use batch::transcribe_batch_with_progress;
pub use error::SherpaError;
pub use model_config::Recognizer;
pub(crate) use model_config::{
    RecognizerInner, accept_vad_samples, build_model_config, create_recognizer_with_gpu_plan,
    decode_offline_samples, load_vad, reset_vad, vad_detected,
};
pub use postprocess::TranscriptPostprocessor;
pub(crate) use state::ModelConfigKey;
pub use state::{AsrState, RecognizerPool};
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

pub(crate) fn get_provider_id(request: &AsrTranscriptionRequest) -> Result<&str, SherpaError> {
    Ok(request.provider_id())
}

pub(crate) fn ensure_adapter(
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
    app: &AppHandle,
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
    let emitter =
        std::sync::Arc::new(app.clone()) as std::sync::Arc<dyn crate::core::event::EventEmitter>;
    session
        .feed_audio_samples(emitter, state, instance_id, samples)
        .await
}
