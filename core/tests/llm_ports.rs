use async_trait::async_trait;
use sona_core::domain::{BuiltinLlmProvider, LlmProvider};
use sona_core::llm::provider_protocol::{LlmModelSummary, StandardLlmResponse};
use sona_core::llm::requests::{LlmConfig, LlmGenerateRequest, LlmModelsRequest};
use sona_core::llm::tasks::LlmProviderStrategy;
use sona_core::ports::llm::{LlmModelLister, LlmTextGenerator};

fn sample_config() -> LlmConfig {
    LlmConfig {
        provider: LlmProvider::Builtin(BuiltinLlmProvider::OpenAi),
        strategy: LlmProviderStrategy::OpenAiCompatible,
        base_url: "https://api.example.com/v1".to_string(),
        api_key: "test-key".to_string(),
        model: "test-model".to_string(),
        api_path: None,
        api_version: None,
        temperature: None,
        reasoning_enabled: None,
        reasoning_level: None,
        timeout_seconds: None,
    }
}

struct EchoLlmAdapter;

#[async_trait]
impl LlmTextGenerator for EchoLlmAdapter {
    async fn generate_text(
        &self,
        request: LlmGenerateRequest,
    ) -> Result<StandardLlmResponse, String> {
        Ok(StandardLlmResponse {
            text: request.input,
            usage: None,
        })
    }
}

#[async_trait]
impl LlmModelLister for EchoLlmAdapter {
    async fn list_models(&self, request: LlmModelsRequest) -> Result<Vec<LlmModelSummary>, String> {
        Ok(vec![LlmModelSummary {
            model: format!("{}:default", request.provider.as_str()),
            context_window: None,
            max_output_tokens: None,
            input_price: None,
            output_price: None,
            supports_reasoning: None,
            supports_tools: None,
            supports_multimodal: None,
            ..LlmModelSummary::default()
        }])
    }
}

#[tokio::test]
async fn text_generator_port_uses_core_llm_request_and_response_types() {
    let adapter = EchoLlmAdapter;
    let response = adapter
        .generate_text(LlmGenerateRequest {
            config: sample_config(),
            input: "hello".to_string(),
            source: None,
        })
        .await
        .expect("text generation through port should succeed");

    assert_eq!(response.text, "hello");
    assert_eq!(response.usage, None);
}

#[tokio::test]
async fn model_lister_port_uses_core_model_request_and_summary_types() {
    let adapter = EchoLlmAdapter;
    let models = adapter
        .list_models(LlmModelsRequest {
            provider: LlmProvider::Builtin(BuiltinLlmProvider::OpenAi),
            strategy: Some(LlmProviderStrategy::OpenAiCompatible),
            base_url: "https://api.example.com/v1".to_string(),
            api_key: "test-key".to_string(),
        })
        .await
        .expect("model listing through port should succeed");

    assert_eq!(models.len(), 1);
    assert_eq!(models[0].model, "open_ai:default");
}
