mod anthropic;
mod completion;
mod gemini;
mod model_discovery;
mod models_dev;
mod ollama;
mod openai_compatible;
mod providers;
mod responses;
mod streaming;
mod transport;

pub use anthropic::{AnthropicAdapter, build_anthropic_payload_for_request};
pub use completion::{
    build_standard_user_input, complete_with_provider, extract_text_response,
    token_usage_from_rig_usage,
};
pub use gemini::{
    GeminiAdapter, GeminiGenerateContentRequestParts,
    build_gemini_generate_content_request_parts_for_reqwest, build_gemini_payload_for_request,
    extract_gemini_usage,
};
pub use model_discovery::{
    build_gemini_models_url, build_openai_models_urls, get_gemini_models, get_openai_models,
    list_models_with_provider,
};
pub use models_dev::{
    ModelsDevCatalog, models_dev_provider_id, parse_models_dev_models, should_enrich_model_metadata,
};
pub use ollama::OllamaAdapter;
pub use openai_compatible::{
    AzureAdapter, CopilotAdapter, OpenAiAdapter, PerplexityAdapter,
    build_openai_chat_payload_for_request, generate_with_openai_chat_api,
    generate_with_openai_custom_path,
};
pub use providers::{
    GenericHttpAdapter, GoogleTranslateAdapter, GoogleTranslateData,
    GoogleTranslateFreeAttemptError, GoogleTranslateRequest, GoogleTranslateResponse,
    GoogleTranslateTranslation, execute_google_translate_free_request,
    execute_google_translate_request, fetch_google_translate_free_translation,
    parse_google_translate_free_retry_after, run_google_translate_free_requests_in_order,
};
pub use responses::{build_openai_responses_payload, generate_with_openai_responses_api};
pub use streaming::{
    extract_anthropic_stream_usage, extract_openai_responses_stream_usage,
    try_stream_completion_with_provider, try_stream_text_with_provider,
};
pub use transport::{LlmApiUrl, parse_llm_api_host, post_json_request, validate_llm_api_host};

use async_trait::async_trait;
use sona_core::llm::provider_protocol::{LlmModelSummary, StandardLlmResponse};
use sona_core::llm::requests::{LlmConfig, LlmGenerateRequest, LlmModelsRequest};
use sona_core::llm::runtime::{LlmCompletionRequest, LlmStreamDelta};
use sona_core::llm::streaming_protocol::StreamTextAccumulator;
use sona_core::ports::llm::{
    LlmCompletionPort, LlmModelDiscoveryPort, LlmModelLister, LlmModelMetadataPort, LlmPortError,
    LlmStreamingPort, LlmTextGenerator,
};

use crate::models_dev::default_models_dev_catalog;
use crate::transport::classify_llm_port_error;

#[derive(Clone, Copy, Debug, Default)]
pub struct OnlineLlmAdapter;

#[async_trait]
impl LlmTextGenerator for OnlineLlmAdapter {
    async fn generate_text(
        &self,
        request: LlmGenerateRequest,
    ) -> Result<StandardLlmResponse, String> {
        generate_text_with_provider(request).await
    }
}

#[async_trait]
impl LlmCompletionPort for OnlineLlmAdapter {
    async fn complete(
        &self,
        request: LlmCompletionRequest,
    ) -> Result<StandardLlmResponse, LlmPortError> {
        complete_with_provider(request).await
    }
}

#[async_trait]
impl LlmStreamingPort for OnlineLlmAdapter {
    async fn stream_completion(
        &self,
        request: LlmCompletionRequest,
        emit_delta: &mut (dyn FnMut(LlmStreamDelta) -> Result<(), LlmPortError> + Send),
    ) -> Result<StandardLlmResponse, LlmPortError> {
        let mut bridge = |text: &str, delta: &str| {
            emit_delta(LlmStreamDelta {
                text: text.to_string(),
                delta: delta.to_string(),
            })
            .map_err(|error| error.to_string())
        };
        let mut accumulator = StreamTextAccumulator::new(&mut bridge);
        let stream_result = try_stream_completion_with_provider(&request, &mut accumulator).await;
        let emitted_any = accumulator.emitted_any();
        drop(accumulator);
        match stream_result {
            Ok(Some(response)) => Ok(response),
            Ok(None) => complete_with_provider(request).await,
            Err(_) if !emitted_any => complete_with_provider(request).await,
            Err(error) => Err(classify_llm_port_error(error)),
        }
    }
}

#[async_trait]
impl LlmModelLister for OnlineLlmAdapter {
    async fn list_models(&self, request: LlmModelsRequest) -> Result<Vec<LlmModelSummary>, String> {
        list_models_with_provider(request).await
    }
}

pub async fn generate_text_with_provider(
    request: LlmGenerateRequest,
) -> Result<StandardLlmResponse, String> {
    complete_with_provider(request.into())
        .await
        .map_err(|error| error.to_string())
}

#[async_trait]
impl LlmModelDiscoveryPort for OnlineLlmAdapter {
    async fn list_models(
        &self,
        request: LlmModelsRequest,
    ) -> Result<Vec<LlmModelSummary>, LlmPortError> {
        list_models_with_provider(request)
            .await
            .map_err(classify_llm_port_error)
    }
}

#[async_trait]
impl LlmModelMetadataPort for OnlineLlmAdapter {
    async fn describe_model(
        &self,
        config: &LlmConfig,
    ) -> Result<Option<LlmModelSummary>, LlmPortError> {
        if !should_enrich_model_metadata(&config.provider, &config.base_url) {
            return Ok(None);
        }
        let Some(provider_id) = models_dev_provider_id(config.strategy) else {
            return Ok(None);
        };
        Ok(default_models_dev_catalog()
            .describe(provider_id, &config.model)
            .await)
    }
}
