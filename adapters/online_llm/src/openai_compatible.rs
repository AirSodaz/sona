use serde_json::{Value, json};
use sona_core::llm::provider_protocol::{StandardLlmResponse, extract_usage_from_json_response};
use sona_core::llm::runtime::{LlmCompletionRequest, LlmResponseFormat};
use sona_core::llm::streaming_protocol::{OpenAiChatPayloadConfig, build_openai_chat_payload};
use sona_core::llm::tasks::LlmProviderStrategy;
use sona_core::ports::llm::{LlmPortError, LlmPortErrorKind};

use crate::completion::completion_input;
use crate::transport::{LlmApiUrl, post_json_request};

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

    let payload = build_openai_chat_payload_for_request(
        request,
        false,
        config.strategy == LlmProviderStrategy::AzureOpenAi,
    )?;

    let response = post_json_request(url, headers, payload, config.timeout_seconds).await?;
    let (text, thought) =
        sona_core::llm::provider_protocol::extract_text_and_thought_from_json_response(&response)?;
    Ok(StandardLlmResponse {
        text,
        thought,
        usage: extract_usage_from_json_response(&response),
    })
}

pub fn build_openai_chat_payload_for_request(
    request: &LlmCompletionRequest,
    stream: bool,
    azure: bool,
) -> Result<Value, LlmPortError> {
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
        if sona_core::llm::streaming_protocol::requires_max_completion_tokens(&request.config.model)
        {
            payload["max_completion_tokens"] = json!(max_output_tokens);
        } else {
            payload["max_tokens"] = json!(max_output_tokens);
        }
    }
    match &request.options.response_format {
        LlmResponseFormat::Text => {}
        LlmResponseFormat::JsonObject => {
            payload["response_format"] = json!({"type": "json_object"});
        }
        LlmResponseFormat::JsonSchema { name, schema } => {
            if !schema.is_object() && !schema.is_boolean() {
                return Err(LlmPortError::new(
                    LlmPortErrorKind::InvalidRequest,
                    "JSON Schema must be an object or boolean",
                ));
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
    let base_url = LlmApiUrl::parse(&config.base_url)?;
    let url = base_url.join(config.api_path.as_deref().unwrap_or("/v1/chat/completions"))?;
    generate_with_openai_chat_api(&url, request, vec![]).await
}
