use super::factory::create_desktop_streaming_asr_factory;
use super::local_asr_registry;
use super::metrics::{
    AsrInferenceMetric, AsrMetricsStore, AsrModelLoadMetric, AsrRuntimeMetricsSnapshot,
    new_metrics_store, set_batch_inference_metric, set_live_inference_metric,
    set_model_load_metric, snapshot_metrics,
};
use sona_application::live_transcription::LiveTranscriptionCoordinator;
use sona_application::local_asr::LocalAsrRegistry;
use sona_sherpa_onnx::runtime::RecognizerPool;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{Mutex, watch};

/// Registry that maps batch `instance_id` to a cancellation-signal sender.
///
/// Each in-flight `process_batch_file` call registers itself here. The
/// corresponding `cancel_batch_task` Tauri command looks up the sender and
/// sends a cancellation signal; the processor's `tokio::select!` branch then
/// wins and returns an error immediately.
pub(crate) struct BatchCancelGuard {
    senders: Arc<std::sync::Mutex<HashMap<String, Arc<watch::Sender<bool>>>>>,
    instance_id: Option<String>,
    sender: Arc<watch::Sender<bool>>,
}

impl BatchCancelGuard {
    #[cfg(test)]
    pub fn instance_id(&self) -> Option<&str> {
        self.instance_id.as_deref()
    }
}

impl Drop for BatchCancelGuard {
    fn drop(&mut self) {
        if let Some(id) = self.instance_id.take()
            && let Ok(mut senders) = self.senders.lock()
            && senders
                .get(&id)
                .is_some_and(|current| Arc::ptr_eq(current, &self.sender))
        {
            senders.remove(&id);
        }
    }
}

#[derive(Default, Clone)]
pub(crate) struct BatchCancelRegistry {
    senders: Arc<std::sync::Mutex<HashMap<String, Arc<watch::Sender<bool>>>>>,
}

impl BatchCancelRegistry {
    #[cfg(test)]
    pub fn new() -> Self {
        Self {
            senders: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    /// Register a new cancellation channel for `instance_id`.
    ///
    /// Returns the receiver that the processor should watch along with a
    /// RAII `BatchCancelGuard` that removes the registration automatically on drop.
    pub async fn register(&self, instance_id: &str) -> (watch::Receiver<bool>, BatchCancelGuard) {
        let (tx, rx) = watch::channel(false);
        let tx = Arc::new(tx);
        if let Ok(mut senders) = self.senders.lock() {
            senders.insert(instance_id.to_string(), tx.clone());
        }
        let guard = BatchCancelGuard {
            senders: Arc::clone(&self.senders),
            instance_id: Some(instance_id.to_string()),
            sender: tx.clone(),
        };
        (rx, guard)
    }

    /// Send the cancellation signal for `instance_id`.
    ///
    /// Returns `true` if a live registration was found, `false` otherwise.
    pub async fn cancel(&self, instance_id: &str) -> bool {
        let senders = self.senders.lock().ok();
        if let Some(senders) = senders
            && let Some(tx) = senders.get(instance_id)
        {
            let _ = tx.send(true);
            true
        } else {
            false
        }
    }

    /// Remove the registration for `instance_id` once the task has finished
    /// (either normally or by cancellation).
    #[cfg(test)]
    pub async fn remove(&self, instance_id: &str) {
        if let Ok(mut senders) = self.senders.lock() {
            senders.remove(instance_id);
        }
    }

    pub async fn has_active_tasks(&self) -> bool {
        self.senders.lock().map(|s| !s.is_empty()).unwrap_or(false)
    }
}

#[derive(Clone)]
pub struct AsrState {
    pub(crate) recognizer_pool: RecognizerPool,
    pub(crate) registry: LocalAsrRegistry,
    pub(crate) metrics: AsrMetricsStore,
    pub(crate) live_coordinator: Arc<LiveTranscriptionCoordinator>,
    pub(crate) batch_cancel: Arc<BatchCancelRegistry>,
    external_sources: Arc<Mutex<HashMap<String, ExternalSourceState>>>,
    next_external_generation: Arc<AtomicU64>,
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
        let factory =
            create_desktop_streaming_asr_factory(registry.clone(), recognizer_pool.clone());
        Self {
            recognizer_pool,
            registry,
            metrics: new_metrics_store(),
            live_coordinator: Arc::new(factory.coordinator()),
            batch_cancel: Arc::new(BatchCancelRegistry::default()),
            external_sources: Arc::new(Mutex::new(HashMap::new())),
            next_external_generation: Arc::new(AtomicU64::new(1)),
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

    pub async fn is_busy(&self) -> bool {
        if self.live_coordinator.is_active().await {
            return true;
        }
        if self.batch_cancel.has_active_tasks().await {
            return true;
        }
        false
    }

    pub async fn clear_model_caches(&self) {
        self.recognizer_pool.clear().await;
        sona_llama_cpp::clear_all_llama_models();
        sona_llama_cpp::clear_all_llm_models();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn batch_cancel_registry_registers_and_cancels() {
        let registry = BatchCancelRegistry::new();
        assert!(!registry.has_active_tasks().await);

        let (rx, guard) = registry.register("task-1").await;
        assert!(registry.has_active_tasks().await);
        assert_eq!(guard.instance_id(), Some("task-1"));
        assert!(!*rx.borrow());

        let cancelled = registry.cancel("task-1").await;
        assert!(cancelled);
        assert!(*rx.borrow());

        drop(guard);
        assert!(!registry.has_active_tasks().await);
    }

    #[tokio::test]
    async fn batch_cancel_guard_removes_on_drop_without_explicit_remove() {
        let registry = BatchCancelRegistry::new();
        assert!(!registry.has_active_tasks().await);

        {
            let (_rx, guard) = registry.register("task-drop").await;
            assert!(registry.has_active_tasks().await);
            assert_eq!(guard.instance_id(), Some("task-drop"));
        }
        // Guard dropped here, registration should be cleaned up
        assert!(!registry.has_active_tasks().await);
    }

    #[tokio::test]
    async fn batch_cancel_manual_remove_before_guard_drop_is_safe() {
        let registry = BatchCancelRegistry::new();
        let (_rx, guard) = registry.register("task-manual").await;
        assert!(registry.has_active_tasks().await);

        registry.remove("task-manual").await;
        assert!(!registry.has_active_tasks().await);

        // Dropping guard after manual remove does not panic or resurrect entry
        drop(guard);
        assert!(!registry.has_active_tasks().await);
    }

    #[tokio::test]
    async fn dropping_superseded_batch_guard_preserves_new_registration() {
        let registry = BatchCancelRegistry::new();
        let (_old_rx, old_guard) = registry.register("shared-id").await;
        let (_new_rx, new_guard) = registry.register("shared-id").await;

        drop(old_guard);
        assert!(registry.has_active_tasks().await);
        assert!(registry.cancel("shared-id").await);
        drop(new_guard);
        assert!(!registry.has_active_tasks().await);
    }
}
