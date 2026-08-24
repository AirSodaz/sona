use async_trait::async_trait;
use sona_application::live_transcription::LiveTranscriptionCoordinator;
use sona_application::local_asr::LocalAsrRegistry;
use sona_core::ports::asr::{
    AsrEngine, AsrPortError, AsrPortErrorKind, AsrRuntimeObserver, AsrStreamingSession,
    NoopAsrRuntimeObserver, StreamingAsrFactoryPort, StreamingInferenceSpec,
};
use std::sync::Arc;

/// Desktop composition root for streaming ASR. The application coordinator owns
/// lifecycle and sharing; this adapter only selects and creates engine sessions
/// through the local engine registry.
pub struct DesktopStreamingAsrFactory {
    registry: LocalAsrRegistry,
}

impl DesktopStreamingAsrFactory {
    pub fn new(registry: LocalAsrRegistry) -> Self {
        Self { registry }
    }

    pub fn coordinator(&self) -> LiveTranscriptionCoordinator {
        LiveTranscriptionCoordinator::new(Arc::new(self.clone()), Arc::new(NoopAsrRuntimeObserver))
    }

    fn local_streaming_factory(
        &self,
        spec: &StreamingInferenceSpec,
    ) -> Result<Arc<dyn StreamingAsrFactoryPort>, AsrPortError> {
        let request = spec.engine_request();
        let engine = request.engine_config.local_engine().ok_or_else(|| {
            AsrPortError::invalid_request("Local streaming requires a local engine selection")
        })?;
        let adapter = self.registry.get(engine).ok_or_else(|| {
            AsrPortError::new(
                AsrPortErrorKind::Unsupported,
                format!(
                    "The {} local ASR engine is not available on this host.",
                    engine.as_str()
                ),
            )
        })?;
        adapter.streaming_factory().ok_or_else(|| {
            AsrPortError::new(
                AsrPortErrorKind::Unsupported,
                format!(
                    "The {} local ASR engine does not support streaming transcription.",
                    engine.as_str()
                ),
            )
        })
    }
}

impl Clone for DesktopStreamingAsrFactory {
    fn clone(&self) -> Self {
        Self {
            registry: self.registry.clone(),
        }
    }
}

#[async_trait]
impl StreamingAsrFactoryPort for DesktopStreamingAsrFactory {
    async fn prepare(&self, spec: &StreamingInferenceSpec) -> Result<(), AsrPortError> {
        match spec.engine() {
            AsrEngine::Local => {
                let factory = self.local_streaming_factory(spec)?;
                factory.prepare(spec).await
            }
            AsrEngine::Online => {
                let request = spec.engine_request();
                sona_online_asr::resolve_online_asr_provider_id(&request)?;
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
            AsrEngine::Local => {
                let factory = self.local_streaming_factory(spec)?;
                factory.create(pipeline_id, spec, observer).await
            }
            AsrEngine::Online => {
                let request = spec.engine_request();
                sona_online_asr::resolve_online_asr_provider_id(&request)?;
                sona_online_asr::OnlineAsrAdapter.create_streaming_session(
                    pipeline_id.to_string(),
                    request,
                    observer,
                )
            }
        }
    }
}
