use async_trait::async_trait;
use reqwest::Client;
use rig_core::client::{CompletionClient, Nothing};
use rig_core::providers::ollama;
use sona_core::llm::provider_protocol::StandardLlmResponse;
use sona_core::llm::runtime::LlmCompletionRequest;
use sona_core::ports::llm::LlmPortError;

use crate::completion::{
    LlmAdapter, build_rig_completion_request, extract_text_response, port_result,
    token_usage_from_rig_usage,
};
use crate::transport::{LlmApiUrl, classify_llm_port_error};

pub struct OllamaAdapter;

#[async_trait]
impl LlmAdapter for OllamaAdapter {
    async fn generate(
        &self,
        _client: &Client,
        request: &LlmCompletionRequest,
    ) -> Result<StandardLlmResponse, LlmPortError> {
        let config = &request.config;
        let reqwest_client = port_result(LlmApiUrl::parse(&config.base_url))?
            .client(config.timeout_seconds)
            .map_err(classify_llm_port_error)?;
        let client = ollama::Client::builder()
            .api_key(Nothing)
            .base_url(config.base_url.trim_end_matches("/v1"))
            .http_client(reqwest_client)
            .build()
            .map_err(|error| classify_llm_port_error(error.to_string()))?;

        let response = port_result(build_rig_completion_request(
            client.completion_model(&config.model),
            request,
        ))?
        .send()
        .await
        .map_err(|error| classify_llm_port_error(error.to_string()))?;

        Ok(StandardLlmResponse {
            text: port_result(extract_text_response(&response.choice))?,
            usage: token_usage_from_rig_usage(Some(response.usage)),
        })
    }
}
