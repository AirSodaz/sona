use super::metrics::{
    AsrInferenceMetric, AsrMetricsStore, AsrModelLoadMetric, AsrRuntimeMetricsSnapshot,
    new_metrics_store, set_batch_inference_metric, set_live_inference_metric,
    set_model_load_metric, snapshot_metrics,
};
use super::types::AsrEngine;
use sona_core::ports::asr::AsrStreamingSession;
use sona_local_asr::runtime::RecognizerPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct AsrState {
    active_sessions: Mutex<HashMap<String, Arc<dyn AsrStreamingSession>>>,
    instance_engines: Mutex<HashMap<String, AsrEngine>>,
    pub(crate) recognizer_pool: RecognizerPool,
    pub(crate) metrics: AsrMetricsStore,
}

impl Default for AsrState {
    fn default() -> Self {
        Self::new()
    }
}

impl AsrState {
    pub fn new() -> Self {
        Self {
            active_sessions: Mutex::new(HashMap::new()),
            instance_engines: Mutex::new(HashMap::new()),
            recognizer_pool: RecognizerPool::new(),
            metrics: new_metrics_store(),
        }
    }

    pub fn recognizer_pool(&self) -> RecognizerPool {
        self.recognizer_pool.clone()
    }

    pub(crate) fn metrics_store(&self) -> AsrMetricsStore {
        self.metrics.clone()
    }

    pub async fn has_online_session(&self, instance_id: &str) -> bool {
        self.active_sessions.lock().await.contains_key(instance_id)
    }

    pub async fn insert_session(&self, instance_id: &str, session: Arc<dyn AsrStreamingSession>) {
        self.active_sessions
            .lock()
            .await
            .insert(instance_id.to_string(), session);
    }

    pub async fn session(&self, instance_id: &str) -> Option<Arc<dyn AsrStreamingSession>> {
        self.active_sessions.lock().await.get(instance_id).cloned()
    }

    pub async fn remove_session(&self, instance_id: &str) -> Option<Arc<dyn AsrStreamingSession>> {
        let mut sessions = self.active_sessions.lock().await;
        let session = sessions.remove(instance_id);
        self.instance_engines.lock().await.remove(instance_id);
        session
    }

    pub async fn set_instance_engine(&self, instance_id: &str, engine: AsrEngine) {
        self.instance_engines
            .lock()
            .await
            .insert(instance_id.to_string(), engine);
    }

    pub async fn instance_engine(&self, instance_id: &str) -> Option<AsrEngine> {
        self.instance_engines.lock().await.get(instance_id).copied()
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

#[cfg(test)]
mod tests {
    use super::*;

    struct DummySession;

    #[async_trait::async_trait]
    impl AsrStreamingSession for DummySession {
        async fn start(&self) -> Result<(), crate::integrations::asr::SherpaError> {
            Ok(())
        }
        async fn stop(&self) -> Result<(), crate::integrations::asr::SherpaError> {
            Ok(())
        }
        async fn flush(&self) -> Result<(), crate::integrations::asr::SherpaError> {
            Ok(())
        }
        async fn feed_audio_chunk(
            &self,
            _samples: Vec<u8>,
        ) -> Result<(), crate::integrations::asr::SherpaError> {
            Ok(())
        }
        async fn feed_audio_samples(
            &self,
            _samples: &[f32],
        ) -> Result<(), crate::integrations::asr::SherpaError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn session_lookup_returns_inserted_session() {
        let state = AsrState::new();
        let instance_id = "test_instance";
        let session: Arc<dyn AsrStreamingSession> = Arc::new(DummySession);

        state.insert_session(instance_id, session.clone()).await;

        let stored = state.session(instance_id).await.expect("session exists");
        assert!(Arc::ptr_eq(&session, &stored));
    }

    #[tokio::test]
    async fn test_remove_session() {
        let state = AsrState::new();
        let instance_id = "test_instance";

        state
            .insert_session(instance_id, Arc::new(DummySession))
            .await;
        state
            .set_instance_engine(instance_id, AsrEngine::LocalSherpa)
            .await;

        assert!(state.has_online_session(instance_id).await);
        assert_eq!(
            state.instance_engine(instance_id).await,
            Some(AsrEngine::LocalSherpa)
        );

        let removed = state.remove_session(instance_id).await;
        assert!(removed.is_some());

        assert!(!state.has_online_session(instance_id).await);
        assert_eq!(state.instance_engine(instance_id).await, None);
    }
}
