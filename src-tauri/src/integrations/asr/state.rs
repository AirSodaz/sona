use super::metrics::{
    AsrInferenceMetric, AsrMetricsStore, AsrModelLoadMetric, AsrRuntimeMetricsSnapshot,
    new_metrics_store, set_batch_inference_metric, set_live_inference_metric,
    set_model_load_metric, snapshot_metrics,
};
use super::traits::AsrStreamingSession;
use super::types::AsrEngine;
use sona_local_asr::runtime::RecognizerPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct AsrState {
    pub active_sessions: Mutex<HashMap<String, Arc<dyn AsrStreamingSession>>>,
    pub instance_engines: Mutex<HashMap<String, AsrEngine>>,
    pub recognizer_pool: RecognizerPool,
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

    pub async fn has_online_session(&self, instance_id: &str) -> bool {
        self.active_sessions.lock().await.contains_key(instance_id)
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
        async fn start(
            &self,
            _emitter: std::sync::Arc<dyn crate::platform::event::EventEmitter>,
            _state: &AsrState,
            _instance_id: &str,
        ) -> Result<(), crate::integrations::asr::SherpaError> {
            Ok(())
        }
        async fn stop(
            &self,
            _state: &AsrState,
            _instance_id: &str,
        ) -> Result<(), crate::integrations::asr::SherpaError> {
            Ok(())
        }
        async fn flush(
            &self,
            _emitter: std::sync::Arc<dyn crate::platform::event::EventEmitter>,
            _state: &AsrState,
            _instance_id: &str,
        ) -> Result<(), crate::integrations::asr::SherpaError> {
            Ok(())
        }
        async fn feed_audio_chunk(
            &self,
            _emitter: std::sync::Arc<dyn crate::platform::event::EventEmitter>,
            _state: &AsrState,
            _instance_id: &str,
            _samples: Vec<u8>,
        ) -> Result<(), crate::integrations::asr::SherpaError> {
            Ok(())
        }
        async fn feed_audio_samples(
            &self,
            _emitter: std::sync::Arc<dyn crate::platform::event::EventEmitter>,
            _state: &AsrState,
            _instance_id: &str,
            _samples: &[f32],
        ) -> Result<(), crate::integrations::asr::SherpaError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_remove_session() {
        let state = AsrState::new();
        let instance_id = "test_instance";

        {
            let mut active = state.active_sessions.lock().await;
            active.insert(instance_id.to_string(), Arc::new(DummySession));
        }
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

    #[tokio::test]
    async fn test_resolve_punctuation_behavior() {
        let pool = RecognizerPool::new();

        // 1. None should return None
        let res_none =
            crate::integrations::asr::sherpa_onnx::resolve_punctuation(&pool, None).await;
        assert!(res_none.is_none());

        // 2. Empty path should return None
        let res_empty =
            crate::integrations::asr::sherpa_onnx::resolve_punctuation(&pool, Some("".to_string()))
                .await;
        assert!(res_empty.is_none());

        // 3. Non-existent path should return None
        let res_nonexistent = crate::integrations::asr::sherpa_onnx::resolve_punctuation(
            &pool,
            Some("nonexistent_path_123".to_string()),
        )
        .await;
        assert!(res_nonexistent.is_none());
    }
}
