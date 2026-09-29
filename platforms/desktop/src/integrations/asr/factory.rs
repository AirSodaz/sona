use async_trait::async_trait;
pub use sona_application::local_asr::HybridStreamingAsrFactory;
use sona_application::local_asr::LocalAsrRegistry;
use sona_sherpa_onnx::runtime::RecognizerPool;
use std::sync::Arc;

/// Builds a desktop streaming ASR factory by composing the shared application
/// hybrid factory with desktop provider adapters and automatic idle resource pruners.
pub fn create_desktop_streaming_asr_factory(
    registry: LocalAsrRegistry,
    _recognizer_pool: RecognizerPool,
) -> HybridStreamingAsrFactory {
    HybridStreamingAsrFactory::from_local_registry(
        registry,
        Some(Arc::new(sona_online_asr::OnlineAsrAdapter)),
    )
}

/// Desktop composition root for streaming ASR.
#[derive(Clone)]
pub struct DesktopStreamingAsrFactory(HybridStreamingAsrFactory);

impl DesktopStreamingAsrFactory {
    pub fn new(registry: LocalAsrRegistry, recognizer_pool: RecognizerPool) -> Self {
        Self(create_desktop_streaming_asr_factory(
            registry,
            recognizer_pool,
        ))
    }

    pub fn coordinator(
        &self,
    ) -> sona_application::live_transcription::LiveTranscriptionCoordinator {
        self.0.coordinator()
    }
}

#[async_trait]
impl sona_core::ports::asr::StreamingAsrFactoryPort for DesktopStreamingAsrFactory {
    async fn prepare(
        &self,
        spec: &sona_core::ports::asr::StreamingInferenceSpec,
    ) -> Result<(), sona_core::ports::asr::AsrPortError> {
        self.0.prepare(spec).await
    }

    async fn create(
        &self,
        pipeline_id: &str,
        spec: &sona_core::ports::asr::StreamingInferenceSpec,
        observer: Arc<dyn sona_core::ports::asr::AsrRuntimeObserver>,
    ) -> Result<
        Arc<dyn sona_core::ports::asr::AsrStreamingSession>,
        sona_core::ports::asr::AsrPortError,
    > {
        self.0.create(pipeline_id, spec, observer).await
    }
}
