use serde_json::{Value, json};
use sona_core::llm::provider_protocol::{
    StandardLlmResponse, extract_text_from_json_response, extract_usage_from_json_response,
};
use sona_core::llm::runtime::{LlmCompletionRequest, LlmResponseFormat};
use sona_core::ports::llm::LlmPortError;

use crate::completion::{completion_input, port_result};
use crate::transport::{LlmApiUrl, post_json_request};

pub async fn generate_with_openai_responses_api(
    request: &LlmCompletionRequest,
) -> Result<StandardLlmResponse, LlmPortError> {
    let config = &request.config;
    let base_url = port_result(LlmApiUrl::parse(&config.base_url))?;
    let url = port_result(base_url.join(config.api_path.as_deref().unwrap_or("/v1/responses")))?;
    let payload = build_openai_responses_payload(request, false);

    let response = post_json_request(
        &url,
        vec![("Authorization", format!("Bearer {}", config.api_key))],
        payload,
        config.timeout_seconds,
    )
    .await?;

    Ok(StandardLlmResponse {
        text: port_result(extract_text_from_json_response(&response))?,
        usage: extract_usage_from_json_response(&response),
    })
}

pub fn build_openai_responses_payload(request: &LlmCompletionRequest, stream: bool) -> Value {
    let config = &request.config;
    let input = completion_input(request);
    let mut payload = json!({
        "model": config.model,
        "input": input,
        "temperature": request.effective_temperature().unwrap_or(0.7),
        "stream": stream,
    });
    if let Some(system_prompt) = request.system_prompt.as_deref() {
        payload["instructions"] = json!(system_prompt);
    }
    if let Some(max_output_tokens) = request.options.max_output_tokens {
        payload["max_output_tokens"] = json!(max_output_tokens);
    }
    if request.effective_reasoning_enabled() {
        payload["reasoning"] = json!({
            "effort": request.effective_reasoning_level().unwrap_or("medium")
        });
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
