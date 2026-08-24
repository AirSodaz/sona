use std::sync::Arc;

use async_trait::async_trait;
use sona_core::ports::asr::{
    AsrPortError, AsrRuntimeObserver, AsrStreamingSession, BatchTranscriberPort,
    EngineCapabilities, LocalAsrAdapter, LocalAsrEngine, LocalSherpaStreamingRequest,
    StreamingAsrFactoryPort, StreamingInferenceSpec,
};

use crate::batch::LocalBatchAsrAdapter;
use crate::runtime::RecognizerPool;
use crate::streaming::{create_streaming_session, prepare_streaming_resources};

/// Provider facade for the sherpa-onnx local ASR engine.
///
/// Wraps the engine's batch transcriber and streaming session factory behind
/// the engine-neutral [`LocalAsrAdapter`] contract so hosts can compose this
/// engine without importing sherpa-specific types.
#[derive(Clone)]
pub struct SherpaOnnxAdapter {
    recognizer_pool: RecognizerPool,
}

impl SherpaOnnxAdapter {
    pub fn new(recognizer_pool: RecognizerPool) -> Self {
        Self { recognizer_pool }
    }
}

impl Default for SherpaOnnxAdapter {
    fn default() -> Self {
        Self::new(RecognizerPool::default())
    }
}

impl LocalAsrAdapter for SherpaOnnxAdapter {
    fn engine(&self) -> LocalAsrEngine {
        LocalAsrEngine::SherpaOnnx
    }

    fn capabilities(&self) -> EngineCapabilities {
        EngineCapabilities::BATCH
            | EngineCapabilities::STREAMING
            | EngineCapabilities::SPEAKER
            | EngineCapabilities::PUNCTUATION
            | EngineCapabilities::HOTWORDS
            | EngineCapabilities::GPU
    }

    fn batch_transcriber(&self) -> Arc<dyn BatchTranscriberPort> {
        Arc::new(LocalBatchAsrAdapter)
    }

    fn streaming_factory(&self) -> Option<Arc<dyn StreamingAsrFactoryPort>> {
        Some(Arc::new(SherpaOnnxStreamingFactory {
            recognizer_pool: self.recognizer_pool.clone(),
        }))
    }
}

#[derive(Clone)]
struct SherpaOnnxStreamingFactory {
    recognizer_pool: RecognizerPool,
}

#[async_trait]
impl StreamingAsrFactoryPort for SherpaOnnxStreamingFactory {
    async fn prepare(&self, spec: &StreamingInferenceSpec) -> Result<(), AsrPortError> {
        let request = spec.engine_request();
        let request = LocalSherpaStreamingRequest::from_local_sherpa_request(
            "prepare".to_string(),
            request,
        )?;
        prepare_streaming_resources(self.recognizer_pool.clone(), &request).await
    }

    async fn create(
        &self,
        pipeline_id: &str,
        spec: &StreamingInferenceSpec,
        observer: Arc<dyn AsrRuntimeObserver>,
    ) -> Result<Arc<dyn AsrStreamingSession>, AsrPortError> {
        let request = spec.engine_request();
        let request = LocalSherpaStreamingRequest::from_local_sherpa_request(
            pipeline_id.to_string(),
            request,
        )?;
        create_streaming_session(self.recognizer_pool.clone(), request, observer)
            .await
            .map(|session| session as Arc<dyn AsrStreamingSession>)
    }
}
