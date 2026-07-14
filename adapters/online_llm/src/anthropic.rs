use async_trait::async_trait;
use reqwest::Client;
use rig_core::client::CompletionClient;
use rig_core::providers::anthropic;
use serde_json::{Value, json};
use sona_core::llm::provider_protocol::{
    StandardLlmResponse, extract_anthropic_text_response, join_url,
};
use sona_core::llm::runtime::{LlmCompletionRequest, LlmPromptCachePolicy};
use sona_core::ports::llm::LlmPortError;

use crate::completion::{
    LlmAdapter, build_rig_completion_request, completion_input, extract_text_response, port_result,
    reasoning_budget_tokens, structured_schema, token_usage_from_rig_usage,
};
use crate::transport::{LlmApiUrl, classify_llm_port_error, post_json_request};

pub fn build_anthropic_payload_for_request(
    request: &LlmCompletionRequest,
    stream: bool,
) -> Result<Value, String> {
    let max_tokens = request.options.max_output_tokens.unwrap_or(8192);
    let mut payload = json!({
        "model": request.config.model,
        "messages": [{"role": "user", "content": completion_input(request)}],
        "max_tokens": max_tokens,
        "temperature": request.effective_temperature().unwrap_or(0.7),
        "stream": stream,
    });
    if let Some(system_prompt) = request.system_prompt.as_deref() {
        payload["system"] = if request.options.prompt_cache == LlmPromptCachePolicy::Automatic {
            json!([{
                "type": "text",
                "text": system_prompt,
                "cache_control": {"type": "ephemeral"}
            }])
        } else {
            json!(system_prompt)
        };
    }
    if request.effective_reasoning_enabled() {
        let max_budget = max_tokens.saturating_sub(1).min(u64::from(u32::MAX)) as u32;
        if max_budget < 1024 {
            return Err(
                "Anthropic reasoning requires max_output_tokens to be greater than 1024"
                    .to_string(),
            );
        }
        payload["thinking"] = json!({
            "type": "enabled",
            "budget_tokens": reasoning_budget_tokens(request.effective_reasoning_level())
                .min(max_budget),
        });
        payload["temperature"] = json!(1.0);
    }
    if request.options.prompt_cache == LlmPromptCachePolicy::Automatic
        && request.system_prompt.is_none()
    {
        payload["messages"][0]["content"] = json!([{
            "type": "text",
            "text": completion_input(request),
            "cache_control": {"type": "ephemeral"}
        }]);
    }
    if let Some(schema) = structured_schema(request)? {
        payload["output_config"] = json!({
            "format": {
                "type": "json_schema",
                "schema": schema,
            }
        });
    }
    Ok(payload)
}

pub struct AnthropicAdapter;

#[async_trait]
impl LlmAdapter for AnthropicAdapter {
    async fn generate(
        &self,
        _client: &Client,
        request: &LlmCompletionRequest,
    ) -> Result<StandardLlmResponse, LlmPortError> {
        let config = &request.config;
        if request.effective_reasoning_enabled() {
            let url = port_result(LlmApiUrl::parse(&join_url(
                &config.base_url,
                "/v1/messages",
            )))?;
            let payload = port_result(build_anthropic_payload_for_request(request, false))?;

            let response = post_json_request(
                &url,
                vec![
                    ("x-api-key", config.api_key.clone()),
                    ("anthropic-version", "2023-06-01".to_string()),
                ],
                payload,
                config.timeout_seconds,
            )
            .await?;

            let (text, usage) = port_result(extract_anthropic_text_response(&response))?;

            return Ok(StandardLlmResponse { text, usage });
        }

        let reqwest_client = port_result(LlmApiUrl::parse(&config.base_url))?
            .client(config.timeout_seconds)
            .map_err(classify_llm_port_error)?;
        let client = anthropic::Client::builder()
            .api_key(&config.api_key)
            .base_url(&config.base_url)
            .http_client(reqwest_client)
            .build()
            .map_err(|error| classify_llm_port_error(error.to_string()))?;

        let mut model = client.completion_model(&config.model);
        if request.options.prompt_cache == LlmPromptCachePolicy::Automatic {
            model = model.with_automatic_caching();
        }
        let response = port_result(build_rig_completion_request(model, request))?
            .send()
            .await
            .map_err(|error| classify_llm_port_error(error.to_string()))?;

        Ok(StandardLlmResponse {
            text: port_result(extract_text_response(&response.choice))?,
            usage: token_usage_from_rig_usage(Some(response.usage)),
        })
    }
}
