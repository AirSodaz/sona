use super::metrics::{
    AsrInferenceMetric, AsrMetricsStore, AsrModelLoadMetric, AsrRuntimeMetricsSnapshot,
    new_metrics_store, set_batch_inference_metric, set_live_inference_metric,
    set_model_load_metric, snapshot_metrics,
};
use super::model_config::Recognizer;
use super::traits::AsrStreamingSession;
use super::types::AsrEngine;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelConfigKey {
    pub model_path: String,
    pub model_type: String,
    pub num_threads: i32,
    pub enable_itn: bool,
    pub language: String,
    pub hotwords: Option<String>,
    pub gpu_provider: Option<String>,
}

impl ModelConfigKey {
    pub fn new(
        model_path: String,
        model_type: String,
        num_threads: i32,
        enable_itn: bool,
        language: String,
        hotwords: Option<String>,
        gpu_provider: Option<String>,
    ) -> Self {
        Self {
            model_path,
            model_type,
            num_threads,
            enable_itn,
            language,
            hotwords,
            gpu_provider,
        }
    }

    pub fn with_gpu_provider(&self, gpu_provider: Option<String>) -> Self {
        Self {
            gpu_provider,
            ..self.clone()
        }
    }
}

pub struct AsrState {
    pub active_sessions: Mutex<HashMap<String, Arc<dyn AsrStreamingSession>>>,
    pub instance_engines: Mutex<HashMap<String, AsrEngine>>,
    pub recognizer_pool: Mutex<HashMap<ModelConfigKey, Arc<Recognizer>>>,
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
            recognizer_pool: Mutex::new(HashMap::new()),
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

    fn key(provider: Option<&str>) -> ModelConfigKey {
        ModelConfigKey {
            model_path: "C:/models/demo".to_string(),
            model_type: "sensevoice".to_string(),
            num_threads: 4,
            enable_itn: true,
            language: "auto".to_string(),
            hotwords: None,
            gpu_provider: provider.map(str::to_string),
        }
    }

    #[test]
    fn model_config_key_separates_gpu_provider() {
        assert_ne!(key(Some("cpu")), key(Some("cuda")));
        assert_ne!(key(Some("cpu")), key(None));
        assert_eq!(key(Some("cpu")), key(Some("cpu")));
    }

    struct DummySession;

    #[async_trait::async_trait]
    impl AsrStreamingSession for DummySession {
        async fn start(
            &self,
            _emitter: std::sync::Arc<dyn crate::core::event::EventEmitter>,
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
            _emitter: std::sync::Arc<dyn crate::core::event::EventEmitter>,
            _state: &AsrState,
            _instance_id: &str,
        ) -> Result<(), crate::integrations::asr::SherpaError> {
            Ok(())
        }
        async fn feed_audio_chunk(
            &self,
            _emitter: std::sync::Arc<dyn crate::core::event::EventEmitter>,
            _state: &AsrState,
            _instance_id: &str,
            _samples: Vec<u8>,
        ) -> Result<(), crate::integrations::asr::SherpaError> {
            Ok(())
        }
        async fn feed_audio_samples(
            &self,
            _emitter: std::sync::Arc<dyn crate::core::event::EventEmitter>,
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
}
