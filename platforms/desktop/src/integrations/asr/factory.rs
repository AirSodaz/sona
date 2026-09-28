use async_trait::async_trait;
use sona_application::live_transcription::LiveTranscriptionCoordinator;
use sona_application::local_asr::{LocalAsrRegistry, LocalStreamingAsrFactory};
use sona_core::ports::asr::{
    AsrEngine, AsrPortError, AsrRuntimeObserver, AsrStreamingSession, NoopAsrRuntimeObserver,
    StreamingAsrFactoryPort, StreamingInferenceSpec,
};
use sona_sherpa_onnx::runtime::RecognizerPool;
use std::sync::Arc;

/// Desktop composition root for streaming ASR. The application coordinator owns
/// lifecycle and sharing; this adapter only selects and creates engine sessions
/// through the local engine registry.
pub struct DesktopStreamingAsrFactory {
    local: LocalStreamingAsrFactory,
    recognizer_pool: RecognizerPool,
}

impl DesktopStreamingAsrFactory {
    pub fn new(registry: LocalAsrRegistry, recognizer_pool: RecognizerPool) -> Self {
        Self {
            local: LocalStreamingAsrFactory::new(registry),
            recognizer_pool,
        }
    }

    pub fn coordinator(&self) -> LiveTranscriptionCoordinator {
        LiveTranscriptionCoordinator::new(Arc::new(self.clone()), Arc::new(NoopAsrRuntimeObserver))
    }
}

impl Clone for DesktopStreamingAsrFactory {
    fn clone(&self) -> Self {
        Self {
            local: self.local.clone(),
            recognizer_pool: self.recognizer_pool.clone(),
        }
    }
}

#[async_trait]
impl StreamingAsrFactoryPort for DesktopStreamingAsrFactory {
    async fn prepare(&self, spec: &StreamingInferenceSpec) -> Result<(), AsrPortError> {
        match spec.engine() {
            AsrEngine::Local => self.local.prepare(spec).await,
            AsrEngine::Online => {
                sona_online_asr::OnlineAsrAdapter.prepare(spec).await?;
                self.recognizer_pool.prune_all_idle().await;
                sona_llama_cpp::prune_idle_llama_models();
                Ok(())
            }
        }
    }

    async fn create(
        &self,
        pipeline_id: &str,
        spec: &StreamingInferenceSpec,
        observer: Arc<dyn AsrRuntimeObserver>,
    ) -> Result<Arc<dyn AsrStreamingSession>, AsrPortError> {
        match spec.engine() {
            AsrEngine::Local => self.local.create(pipeline_id, spec, observer).await,
            AsrEngine::Online => {
                self.recognizer_pool.prune_all_idle().await;
                sona_llama_cpp::prune_idle_llama_models();
                sona_online_asr::OnlineAsrAdapter
                    .create(pipeline_id, spec, observer)
                    .await
            }
        }
    }
}
