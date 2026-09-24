use serde_json::{Value, json};
use sona_core::llm::provider_protocol::{StandardLlmResponse, extract_usage_from_json_response};
use sona_core::llm::runtime::{LlmCompletionRequest, LlmResponseFormat};
use sona_core::ports::llm::LlmPortError;

use crate::completion::completion_input;
use crate::transport::{LlmApiUrl, post_json_request};

pub async fn generate_with_openai_responses_api(
    request: &LlmCompletionRequest,
) -> Result<StandardLlmResponse, LlmPortError> {
    let config = &request.config;
    let base_url = LlmApiUrl::parse(&config.base_url)?;
    let url = base_url.join(config.api_path.as_deref().unwrap_or("/v1/responses"))?;
    let payload = build_openai_responses_payload(request, false);

    let response = post_json_request(
        &url,
        vec![("Authorization", format!("Bearer {}", config.api_key))],
        payload,
        config.timeout_seconds,
    )
    .await?;

    let (text, thought) =
        sona_core::llm::provider_protocol::extract_text_and_thought_from_json_response(&response)?;
    Ok(StandardLlmResponse {
        text,
        thought,
        usage: extract_usage_from_json_response(&response),
    })
}

pub fn build_openai_responses_payload(request: &LlmCompletionRequest, stream: bool) -> Value {
    let config = &request.config;
    let input = completion_input(request);
    let mut payload = json!({
        "model": config.model,
        "input": input,
        "stream": stream,
    });
    if !sona_core::llm::streaming_protocol::is_temperature_prohibited_for_model(&config.model) {
        payload["temperature"] = json!(request.effective_temperature().unwrap_or(0.7));
    }
    if let Some(system_prompt) = request.system_prompt.as_deref() {
        payload["instructions"] = json!(system_prompt);
    }
    if let Some(max_output_tokens) = request.options.max_output_tokens {
        payload["max_output_tokens"] = json!(max_output_tokens);
    }
    if request.effective_reasoning_enabled() {
        let thinking = request.effective_thinking_level();
        let effort = match thinking {
            sona_core::llm::runtime::ThinkingLevel::None
            | sona_core::llm::runtime::ThinkingLevel::Auto => None,
            _ => thinking
                .clamp_to_supported(&[
                    sona_core::llm::runtime::ThinkingLevel::Low,
                    sona_core::llm::runtime::ThinkingLevel::Medium,
                    sona_core::llm::runtime::ThinkingLevel::High,
                ])
                .and_then(|t| t.as_effort_str()),
        };
        if let Some(effort) = effort {
            payload["reasoning"] = json!({
                "effort": effort
            });
        }
    }
    let schema_format = match &request.options.response_format {
        LlmResponseFormat::Text => None,
        LlmResponseFormat::JsonObject => Some(json!({
            "type": "json_object"
        })),
        LlmResponseFormat::JsonSchema { name, schema } => Some(json!({
            "type": "json_schema",
            "name": name,
            "strict": true,
            "schema": schema
        })),
    };
    if let Some(format) = schema_format {
        payload["text"] = json!({"format": format});
    }
    payload
}
