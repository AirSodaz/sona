use super::factory::DesktopStreamingAsrFactory;
use super::local_asr_registry;
use super::metrics::{
    AsrInferenceMetric, AsrMetricsStore, AsrModelLoadMetric, AsrRuntimeMetricsSnapshot,
    new_metrics_store, set_batch_inference_metric, set_live_inference_metric,
    set_model_load_metric, snapshot_metrics,
};
use sona_application::live_transcription::LiveTranscriptionCoordinator;
use sona_application::local_asr::LocalAsrRegistry;
use sona_core::ports::asr::{
    AsrPortError, AsrRuntimeObserver, AsrStreamingSession, AsrTranscriptionRequest,
    StreamingAsrFactoryPort, StreamingInferenceSpec,
};
use sona_sherpa_onnx::runtime::RecognizerPool;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::Mutex;

pub struct AsrState {
    pub(crate) recognizer_pool: RecognizerPool,
    pub(crate) registry: LocalAsrRegistry,
    pub(crate) metrics: AsrMetricsStore,
    pub(crate) live_coordinator: LiveTranscriptionCoordinator,
    external_sources: Mutex<HashMap<String, ExternalSourceState>>,
    next_external_generation: AtomicU64,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalLiveSource {
    pub source_token: String,
    pub source_id: String,
    pub source_generation: u64,
    pub source_cursor: u64,
}

struct ExternalSourceState {
    source: sona_application::live_transcription::LiveSourceEpoch,
    sequence: u64,
    sample_cursor: u64,
}

impl Default for AsrState {
    fn default() -> Self {
        Self::new()
    }
}

impl AsrState {
    pub fn new() -> Self {
        let recognizer_pool = RecognizerPool::new();
        let registry = local_asr_registry(recognizer_pool.clone());
        let factory = DesktopStreamingAsrFactory::new(registry.clone());
        Self {
            recognizer_pool,
            registry,
            metrics: new_metrics_store(),
            live_coordinator: factory.coordinator(),
            external_sources: Mutex::new(HashMap::new()),
            next_external_generation: AtomicU64::new(1),
        }
    }

    pub fn recognizer_pool(&self) -> RecognizerPool {
        self.recognizer_pool.clone()
    }

    pub(crate) fn metrics_store(&self) -> AsrMetricsStore {
        self.metrics.clone()
    }

    pub(crate) fn live_coordinator(&self) -> &LiveTranscriptionCoordinator {
        &self.live_coordinator
    }

    pub(crate) async fn create_independent_streaming_session(
        &self,
        session_id: &str,
        request: &AsrTranscriptionRequest,
        observer: Arc<dyn AsrRuntimeObserver>,
    ) -> Result<Arc<dyn AsrStreamingSession>, AsrPortError> {
        let spec = StreamingInferenceSpec::from_request(request)?;
        DesktopStreamingAsrFactory::new(self.registry.clone())
            .create(session_id, &spec, observer)
            .await
    }

    pub(crate) async fn create_external_source(&self) -> ExternalLiveSource {
        let generation = self
            .next_external_generation
            .fetch_add(1, Ordering::Relaxed);
        let token = uuid::Uuid::new_v4().to_string();
        let source = sona_application::live_transcription::LiveSourceEpoch::new(
            format!("external-source-{generation}"),
            generation,
        );
        self.external_sources.lock().await.insert(
            token.clone(),
            ExternalSourceState {
                source: source.clone(),
                sequence: 0,
                sample_cursor: 0,
            },
        );
        ExternalLiveSource {
            source_token: token,
            source_id: source.source_id,
            source_generation: source.generation,
            source_cursor: 0,
        }
    }

    pub(crate) async fn external_source(
        &self,
        token: &str,
    ) -> Option<(sona_application::live_transcription::LiveSourceEpoch, u64)> {
        self.external_sources
            .lock()
            .await
            .get(token)
            .map(|source| (source.source.clone(), source.sample_cursor))
    }

    pub(crate) async fn next_external_frame(
        &self,
        token: &str,
        samples: Vec<f32>,
    ) -> Option<(
        sona_application::live_transcription::LiveSourceEpoch,
        sona_core::ports::asr::AsrAudioFrame,
    )> {
        let mut sources = self.external_sources.lock().await;
        let source = sources.get_mut(token)?;
        source.sequence = source.sequence.saturating_add(1);
        let frame = sona_core::ports::asr::AsrAudioFrame::new(
            source.sequence,
            source.sample_cursor,
            samples,
        );
        source.sample_cursor = frame.end_sample();
        Some((source.source.clone(), frame))
    }

    pub(crate) async fn remove_external_source(
        &self,
        token: &str,
    ) -> Option<sona_application::live_transcription::LiveSourceEpoch> {
        self.external_sources
            .lock()
            .await
            .remove(token)
            .map(|source| source.source)
    }

    pub async fn record_model_load_metric(&self, metric: AsrModelLoadMetric) {
        set_model_load_metric(&self.metrics, metric);
    }

    pub async fn record_live_inference_metric(&self, metric: AsrInferenceMetric) {
        set_live_inference_metric(&self.metrics, metric);
    }

    pub async fn record_batch_inference_metric(&self, metric: AsrInferenceMetric) {
        set_batch_inference_metric(&self.metrics, metric);
    }

    pub async fn metrics_snapshot(&self) -> AsrRuntimeMetricsSnapshot {
        snapshot_metrics(&self.metrics)
    }
}
