use async_trait::async_trait;
use reqwest::Client;
use rig_core::client::CompletionClient;
use rig_core::providers::{azure, copilot, openai, perplexity};
use serde_json::{Value, json};
use sona_core::llm::provider_protocol::{
    StandardLlmResponse, extract_text_from_json_response, extract_usage_from_json_response,
    strategy_uses_openai_chat_payload,
};
use sona_core::llm::runtime::{LlmCompletionRequest, LlmResponseFormat};
use sona_core::llm::streaming_protocol::{OpenAiChatPayloadConfig, build_openai_chat_payload};
use sona_core::llm::tasks::LlmProviderStrategy;
use sona_core::ports::llm::LlmPortError;

use crate::completion::{
    LlmAdapter, build_rig_completion_request, completion_input, extract_text_response, port_result,
    token_usage_from_rig_usage,
};
use crate::transport::{LlmApiUrl, classify_llm_port_error, post_json_request};

pub struct OpenAiAdapter;

#[async_trait]
impl LlmAdapter for OpenAiAdapter {
    async fn generate(
        &self,
        _client: &Client,
        request: &LlmCompletionRequest,
    ) -> Result<StandardLlmResponse, LlmPortError> {
        let config = &request.config;
        let reqwest_client = port_result(LlmApiUrl::parse(&config.base_url))?
            .client(config.timeout_seconds)
            .map_err(classify_llm_port_error)?;
        let client = openai::Client::builder()
            .api_key(&config.api_key)
            .base_url(&config.base_url)
            .http_client(reqwest_client)
            .build()
            .map_err(|error| classify_llm_port_error(error.to_string()))?;

        if request.effective_reasoning_enabled()
            && strategy_uses_openai_chat_payload(config.strategy)
        {
            let base_url = port_result(LlmApiUrl::parse(&config.base_url))?;
            let url = port_result(
                base_url.join(config.api_path.as_deref().unwrap_or("/v1/chat/completions")),
            )?;
            return generate_with_openai_chat_api(&url, request, vec![]).await;
        }

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

pub struct AzureAdapter;

#[async_trait]
impl LlmAdapter for AzureAdapter {
    async fn generate(
        &self,
        _client: &Client,
        request: &LlmCompletionRequest,
    ) -> Result<StandardLlmResponse, LlmPortError> {
        let config = &request.config;
        let api_version = config.api_version.as_deref().unwrap_or("2024-10-21");
        let reqwest_client = port_result(LlmApiUrl::parse(&config.base_url))?
            .client(config.timeout_seconds)
            .map_err(classify_llm_port_error)?;
        let client = azure::Client::builder()
            .api_key(azure::AzureOpenAIAuth::ApiKey(config.api_key.clone()))
            .azure_endpoint(config.base_url.clone())
            .api_version(api_version)
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

pub struct CopilotAdapter;

#[async_trait]
impl LlmAdapter for CopilotAdapter {
    async fn generate(
        &self,
        _client: &Client,
        request: &LlmCompletionRequest,
    ) -> Result<StandardLlmResponse, LlmPortError> {
        let config = &request.config;
        let reqwest_client = port_result(LlmApiUrl::parse(&config.base_url))?
            .client(config.timeout_seconds)
            .map_err(classify_llm_port_error)?;
        let client = copilot::Client::builder()
            .api_key(&config.api_key)
            .base_url(&config.base_url)
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

pub struct PerplexityAdapter;

#[async_trait]
impl LlmAdapter for PerplexityAdapter {
    async fn generate(
        &self,
        _client: &Client,
        request: &LlmCompletionRequest,
    ) -> Result<StandardLlmResponse, LlmPortError> {
        let config = &request.config;
        let reqwest_client = port_result(LlmApiUrl::parse(&config.base_url))?
            .client(config.timeout_seconds)
            .map_err(classify_llm_port_error)?;
        let client = perplexity::Client::builder()
            .api_key(&config.api_key)
            .base_url(&config.base_url)
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

pub async fn generate_with_openai_chat_api(
    url: &LlmApiUrl,
    request: &LlmCompletionRequest,
    extra_headers: Vec<(&str, String)>,
) -> Result<StandardLlmResponse, LlmPortError> {
    let config = &request.config;
    let mut headers = vec![];
    if !config.api_key.is_empty() {
        headers.push(("Authorization", format!("Bearer {}", config.api_key)));
    }
    headers.extend(extra_headers);

    let payload = port_result(build_openai_chat_payload_for_request(
        request,
        false,
        config.strategy == LlmProviderStrategy::AzureOpenAi,
    ))?;

    let response = post_json_request(url, headers, payload, config.timeout_seconds).await?;
    Ok(StandardLlmResponse {
        text: port_result(extract_text_from_json_response(&response))?,
        usage: extract_usage_from_json_response(&response),
    })
}

pub fn build_openai_chat_payload_for_request(
    request: &LlmCompletionRequest,
    stream: bool,
    azure: bool,
) -> Result<Value, String> {
    let strategy = if azure {
        LlmProviderStrategy::AzureOpenAi
    } else {
        request.config.strategy
    };
    let input = completion_input(request);
    let mut payload = build_openai_chat_payload(
        OpenAiChatPayloadConfig {
            strategy,
            model: &request.config.model,
            temperature: request.effective_temperature(),
            reasoning_enabled: request.effective_reasoning_enabled(),
            reasoning_level: request.effective_reasoning_level(),
        },
        &input,
        stream,
    );

    if let Some(system_prompt) = request
        .system_prompt
        .as_deref()
        .filter(|prompt| !prompt.trim().is_empty())
        && let Some(messages) = payload.get_mut("messages").and_then(Value::as_array_mut)
    {
        messages.insert(
            0,
            json!({
                "role": "system",
                "content": system_prompt,
            }),
        );
    }
    if let Some(max_output_tokens) = request.options.max_output_tokens {
        payload["max_tokens"] = json!(max_output_tokens);
    }
    match &request.options.response_format {
        LlmResponseFormat::Text => {}
        LlmResponseFormat::JsonObject => {
            payload["response_format"] = json!({"type": "json_object"});
        }
        LlmResponseFormat::JsonSchema { name, schema } => {
            if !schema.is_object() && !schema.is_boolean() {
                return Err("JSON Schema must be an object or boolean".to_string());
            }
            payload["response_format"] = json!({
                "type": "json_schema",
                "json_schema": {
                    "name": name,
                    "strict": true,
                    "schema": schema,
                }
            });
        }
    }

    Ok(payload)
}

pub async fn generate_with_openai_custom_path(
    request: &LlmCompletionRequest,
) -> Result<StandardLlmResponse, LlmPortError> {
    let config = &request.config;
    let base_url = port_result(LlmApiUrl::parse(&config.base_url))?;
    let url =
        port_result(base_url.join(config.api_path.as_deref().unwrap_or("/v1/chat/completions")))?;
    generate_with_openai_chat_api(&url, request, vec![]).await
}
