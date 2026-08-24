use async_trait::async_trait;
use sona_application::live_transcription::LiveTranscriptionCoordinator;
use sona_core::ports::asr::{
    AsrPortError, AsrRuntimeObserver, AsrStreamingSession, NoopAsrRuntimeObserver,
    StreamingAsrFactoryPort, StreamingInferenceSpec,
};
use sona_sherpa_onnx::runtime::RecognizerPool;
use std::sync::Arc;

/// Desktop composition root for streaming ASR. The application coordinator owns
/// lifecycle and sharing; this adapter only selects and creates engine sessions.
pub struct DesktopStreamingAsrFactory {
    recognizer_pool: RecognizerPool,
}

impl DesktopStreamingAsrFactory {
    pub fn new(recognizer_pool: RecognizerPool) -> Self {
        Self { recognizer_pool }
    }

    pub fn coordinator(&self) -> LiveTranscriptionCoordinator {
        LiveTranscriptionCoordinator::new(Arc::new(self.clone()), Arc::new(NoopAsrRuntimeObserver))
    }
}

impl Clone for DesktopStreamingAsrFactory {
    fn clone(&self) -> Self {
        Self {
            recognizer_pool: self.recognizer_pool.clone(),
        }
    }
}

#[async_trait]
impl StreamingAsrFactoryPort for DesktopStreamingAsrFactory {
    async fn prepare(&self, spec: &StreamingInferenceSpec) -> Result<(), AsrPortError> {
        let request = spec.engine_request();
        match spec.engine() {
            sona_core::ports::asr::AsrEngine::LocalSherpa => {
                let request =
                    sona_core::ports::asr::LocalSherpaStreamingRequest::from_local_sherpa_request(
                        "prepare".to_string(),
                        request,
                    )?;
                sona_sherpa_onnx::streaming::prepare_streaming_resources(
                    self.recognizer_pool.clone(),
                    &request,
                )
                .await?;
                Ok(())
            }
            sona_core::ports::asr::AsrEngine::Online => {
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
        let request = spec.engine_request();
        match spec.engine() {
            sona_core::ports::asr::AsrEngine::LocalSherpa => {
                sona_sherpa_onnx::streaming::create_streaming_session(
                    self.recognizer_pool.clone(),
                    sona_core::ports::asr::LocalSherpaStreamingRequest::from_local_sherpa_request(
                        pipeline_id.to_string(),
                        request,
                    )?,
                    observer,
                )
                .await
                .map(|session| session as Arc<dyn AsrStreamingSession>)
            }
            sona_core::ports::asr::AsrEngine::Online => {
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
