use async_trait::async_trait;
use sona_application::local_asr::{
    HybridStreamingAsrFactory, IdleResourcePruner, LocalAsrRegistry,
};
use sona_core::ports::asr::{
    AsrPortError, AsrRuntimeObserver, AsrStreamingSession, AsrTranscriptionRequest,
    BatchTranscriberPort, OnlineBatchTranscriptionRequest, StreamingAsrFactoryPort,
    StreamingInferenceSpec,
};
use sona_core::transcription::runtime::LiveTranscribePlan;
use sona_core::transcription::transcript::TranscriptSegment;
use sona_sherpa_onnx::runtime::RecognizerPool;
use std::path::PathBuf;
use std::sync::Arc;

pub(crate) fn local_asr_registry(recognizer_pool: RecognizerPool) -> LocalAsrRegistry {
    let vad_engines = sona_vad::built_in_engines();
    let punct_engines = sona_punct::built_in_engines();
    LocalAsrRegistry::empty()
        .register(Arc::new(sona_sherpa_onnx::SherpaOnnxAdapter::new(
            recognizer_pool,
            vad_engines.clone(),
            punct_engines.clone(),
        )))
        .register(Arc::new(sona_llama_cpp::LlamaCppAdapter::new(
            vad_engines,
            punct_engines,
        )))
}

pub(crate) fn local_batch_transcriber() -> impl BatchTranscriberPort {
    let registry = local_asr_registry(RecognizerPool::default());
    sona_application::local_asr::LocalBatchTranscriberRouter::new(registry)
}

struct CliAsrIdlePruner(RecognizerPool);

#[async_trait]
impl IdleResourcePruner for CliAsrIdlePruner {
    async fn prune_idle_resources(&self) {
        self.0.prune_all_idle().await;
        sona_llama_cpp::prune_idle_llama_models();
    }
}

/// Builds a CLI streaming ASR factory by composing the shared application
/// hybrid factory with CLI provider adapters and idle resource pruners.
pub fn create_cli_streaming_asr_factory(
    registry: LocalAsrRegistry,
    recognizer_pool: RecognizerPool,
) -> HybridStreamingAsrFactory {
    HybridStreamingAsrFactory::from_local_registry(
        registry,
        Some(Arc::new(sona_online_asr::OnlineAsrAdapter)),
    )
    .with_idle_pruner(Arc::new(CliAsrIdlePruner(recognizer_pool)))
}

/// CLI composition root for streaming ASR.
#[derive(Clone)]
pub struct CliStreamingAsrFactory(HybridStreamingAsrFactory);

impl CliStreamingAsrFactory {
    pub fn new(registry: LocalAsrRegistry, recognizer_pool: RecognizerPool) -> Self {
        Self(create_cli_streaming_asr_factory(registry, recognizer_pool))
    }
}

#[async_trait]
impl StreamingAsrFactoryPort for CliStreamingAsrFactory {
    async fn prepare(&self, spec: &StreamingInferenceSpec) -> Result<(), AsrPortError> {
        self.0.prepare(spec).await
    }

    async fn create(
        &self,
        pipeline_id: &str,
        spec: &StreamingInferenceSpec,
        observer: Arc<dyn AsrRuntimeObserver>,
    ) -> Result<Arc<dyn AsrStreamingSession>, AsrPortError> {
        self.0.create(pipeline_id, spec, observer).await
    }
}
pub(crate) fn streaming_transcriber() -> Arc<dyn StreamingAsrFactoryPort> {
    let recognizer_pool = RecognizerPool::default();
    let registry = local_asr_registry(recognizer_pool.clone());
    Arc::new(CliStreamingAsrFactory::new(registry, recognizer_pool))
}

pub(crate) async fn local_streaming_session(
    plan: &LiveTranscribePlan,
    instance_id: &str,
    observer: Arc<dyn AsrRuntimeObserver>,
) -> Result<Arc<dyn AsrStreamingSession>, String> {
    let session = sona_sherpa_onnx::streaming::create_streaming_session(
        sona_sherpa_onnx::runtime::RecognizerPool::default(),
        plan.to_local_streaming_request(instance_id),
        observer,
    )
    .await
    .map_err(|error| error.to_string())?;
    Ok(session)
}

pub(crate) async fn online_batch_transcribe(
    file_path: PathBuf,
    request: AsrTranscriptionRequest,
) -> Result<Vec<TranscriptSegment>, sona_core::ports::asr::AsrPortError> {
    sona_online_asr::OnlineAsrAdapter
        .transcribe_batch(OnlineBatchTranscriptionRequest { file_path, request })
        .await
        .map(|output| output.segments)
}

pub(crate) fn online_streaming_session(
    request: AsrTranscriptionRequest,
    instance_id: &str,
    observer: Arc<dyn AsrRuntimeObserver>,
) -> Result<Arc<dyn AsrStreamingSession>, sona_core::ports::asr::AsrPortError> {
    sona_online_asr::OnlineAsrAdapter.create_streaming_session(
        instance_id.to_string(),
        request,
        observer,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use sona_core::ports::asr::{
        AsrEngineConfig, AsrMode, OnlineAsrProviderRequest, StreamingInferenceSpec,
    };

    #[tokio::test]
    async fn streaming_transcriber_prepares_online_spec() {
        let factory = streaming_transcriber();
        let request = AsrTranscriptionRequest {
            engine_config: AsrEngineConfig::Online {
                provider: OnlineAsrProviderRequest {
                    provider_id: "volcengine-doubao".to_string(),
                    profile_id: "volcengine-doubao".to_string(),
                    config: serde_json::json!({
                        "appId": "test-app",
                        "accessToken": "test-token",
                    }),
                },
            },
            mode: AsrMode::Streaming,
            enable_itn: false,
            language: "zh".to_string(),
            hotwords: None,
            speaker_processing: None,
            normalization_options: Default::default(),
            postprocess_options: Default::default(),
        };
        let spec = StreamingInferenceSpec::from_request(&request).unwrap();
        assert!(factory.prepare(&spec).await.is_ok());
    }

    #[tokio::test]
    async fn streaming_transcriber_rejects_unsupported_online_provider() {
        let factory = streaming_transcriber();
        let request = AsrTranscriptionRequest {
            engine_config: AsrEngineConfig::Online {
                provider: OnlineAsrProviderRequest {
                    provider_id: "nonexistent-provider".to_string(),
                    profile_id: "nonexistent-provider".to_string(),
                    config: serde_json::json!({}),
                },
            },
            mode: AsrMode::Streaming,
            enable_itn: false,
            language: "zh".to_string(),
            hotwords: None,
            speaker_processing: None,
            normalization_options: Default::default(),
            postprocess_options: Default::default(),
        };
        let spec = StreamingInferenceSpec::from_request(&request).unwrap();
        assert!(factory.prepare(&spec).await.is_err());
    }
}
