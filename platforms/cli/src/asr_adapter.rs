use sona_application::local_asr::{HybridStreamingAsrFactory, LocalAsrRegistry};
use sona_core::ports::asr::{
    AsrTranscriptionRequest, BatchTranscriberPort, OnlineBatchTranscriptionRequest,
    StreamingAsrFactoryPort,
};
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

/// Builds a CLI streaming ASR factory by composing the shared application
/// hybrid factory with CLI provider adapters and automatic idle resource pruners.
pub fn create_cli_streaming_asr_factory(registry: LocalAsrRegistry) -> HybridStreamingAsrFactory {
    HybridStreamingAsrFactory::from_local_registry(
        registry,
        Some(Arc::new(sona_online_asr::OnlineAsrAdapter)),
    )
}
/// CLI composition root for streaming ASR.
#[allow(dead_code)]
pub type CliStreamingAsrFactory = HybridStreamingAsrFactory;

pub(crate) fn streaming_transcriber() -> Arc<dyn StreamingAsrFactoryPort> {
    let recognizer_pool = RecognizerPool::default();
    let registry = local_asr_registry(recognizer_pool);
    Arc::new(create_cli_streaming_asr_factory(registry))
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
