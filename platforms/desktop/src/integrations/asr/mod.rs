use sona_core::ports::asr::LOCAL_SHERPA_PROVIDER_ID;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use tauri::{AppHandle, Manager};

mod adapter;
mod batch;
mod metrics;
mod observer;
mod online;
mod state;
mod traits;
mod transcript;
mod types;

const BATCH_PROGRESS_EVENT: &str = "batch-progress";

fn recognizer_output_event(instance_id: &str) -> String {
    format!("recognizer-output-{instance_id}")
}

pub use adapter::LocalSherpaAdapter;
pub use batch::transcribe_batch_with_progress;
pub(crate) use observer::TauriAsrRuntimeObserver;
pub use sona_core::models::config::ModelFileConfig;
pub use sona_core::ports::asr::AsrStreamingSession;
pub use sona_core::ports::asr::SherpaError;
pub use sona_core::transcription::asr_metrics::{
    AsrInferenceMetric, AsrModelLoadMetric, AsrRuntimeMetricsSnapshot,
};
pub use sona_core::transcription::postprocess::TranscriptPostprocessor;
pub(crate) use sona_local_asr::audio::{accept_vad_samples, vad_detected};
pub use sona_local_asr::punctuation::{Punctuation, load_punctuation};
pub use sona_local_asr::recognizer::Recognizer;
pub(crate) use sona_local_asr::recognizer::{
    build_model_config, create_recognizer_with_gpu_plan, decode_offline_samples,
};
pub(crate) use sona_local_asr::runtime::ModelConfigKey;
pub use sona_local_asr::runtime::RecognizerPool;
pub use state::AsrState;
pub use traits::{AsrBatchProcessor, AsrProviderAdapter};
pub(crate) use transcript::{
    finalize_transcript_text, normalize_recognizer_text, synthesize_durations,
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
        for capability in sona_online_asr::ONLINE_ASR_PROVIDER_CAPABILITIES {
            let adapter = online::DesktopOnlineAsrAdapter::new(capability.provider_id);
            map.insert(adapter.provider_id(), Arc::new(adapter));
        }
        map
    })
}

pub(crate) fn get_provider_id(request: &AsrTranscriptionRequest) -> Result<&str, SherpaError> {
    Ok(request.provider_id())
}

pub(crate) fn ensure_adapter(
    request: &AsrTranscriptionRequest,
) -> Result<Arc<dyn AsrProviderAdapter>, SherpaError> {
    let provider_id = match request.engine() {
        AsrEngine::LocalSherpa => LOCAL_SHERPA_PROVIDER_ID,
        AsrEngine::Online => sona_online_asr::resolve_online_asr_provider_id(request)?,
    };
    asr_adapters()
        .get(provider_id)
        .cloned()
        .ok_or_else(|| SherpaError::UnsupportedOnlineProvider {
            provider_id: provider_id.to_string(),
        })
}

pub(crate) fn recognizer_pool_for_app(app: Option<&AppHandle>) -> RecognizerPool {
    app.map(|app| app.state::<AsrState>().recognizer_pool())
        .unwrap_or_else(RecognizerPool::new)
}

/// Feed f32 audio samples from the hardware capture worker to the correct
/// ASR backend. Routes by the engine selected during `init_recognizer` so an
/// expected online recognizer cannot silently fall through to local Sherpa when
/// the online session is missing or failed.
pub async fn feed_audio_samples(
    state: &AsrState,
    instance_id: &str,
    samples: &[f32],
) -> Result<(), SherpaError> {
    let session = state
        .session(instance_id)
        .await
        .ok_or_else(|| SherpaError::Generic(format!("ASR instance {} not found", instance_id)))?;
    session.feed_audio_samples(samples).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use sona_core::ports::asr::{
        AsrEngineConfig, AsrMode, GROQ_WHISPER_PROVIDER_ID, LOCAL_SHERPA_PROVIDER_ID,
        OnlineAsrProviderRequest, VOLCENGINE_DOUBAO_PROVIDER_ID,
    };

    fn online_request(provider_id: &str, mode: AsrMode) -> AsrTranscriptionRequest {
        AsrTranscriptionRequest {
            mode,
            language: "auto".into(),
            enable_itn: false,
            normalization_options: Default::default(),
            postprocess_options: Default::default(),
            hotwords: None,
            speaker_processing: None,
            engine_config: AsrEngineConfig::Online {
                provider: OnlineAsrProviderRequest {
                    provider_id: provider_id.into(),
                    profile_id: "test".into(),
                    config: serde_json::Value::Null,
                },
            },
        }
    }

    fn local_request() -> AsrTranscriptionRequest {
        AsrTranscriptionRequest::local_sherpa(
            AsrMode::Streaming,
            "model".into(),
            1,
            false,
            "auto".into(),
            None,
            None,
            5.0,
            "zipformer".into(),
            None,
            None,
            Default::default(),
            Default::default(),
            None,
            None,
        )
    }

    #[test]
    fn desktop_registry_selects_volcengine_streaming() {
        let adapter = ensure_adapter(&online_request(
            VOLCENGINE_DOUBAO_PROVIDER_ID,
            AsrMode::Streaming,
        ))
        .unwrap();
        assert_eq!(adapter.provider_id(), VOLCENGINE_DOUBAO_PROVIDER_ID);
    }

    #[test]
    fn desktop_registry_selects_groq_batch() {
        let adapter =
            ensure_adapter(&online_request(GROQ_WHISPER_PROVIDER_ID, AsrMode::Batch)).unwrap();
        assert_eq!(adapter.provider_id(), GROQ_WHISPER_PROVIDER_ID);
    }

    #[test]
    fn desktop_registry_selects_local_streaming() {
        let adapter = ensure_adapter(&local_request()).unwrap();
        assert_eq!(adapter.provider_id(), LOCAL_SHERPA_PROVIDER_ID);
    }

    #[test]
    fn desktop_registry_rejects_batch_only_streaming() {
        let error = ensure_adapter(&online_request(
            GROQ_WHISPER_PROVIDER_ID,
            AsrMode::Streaming,
        ))
        .err()
        .expect("batch-only provider must not receive a streaming session");
        assert!(matches!(error, SherpaError::StreamingNotSupported { .. }));
    }
}
