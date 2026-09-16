use serde_json::{Value, json};
use sona_core::llm::runtime::{LlmCompletionRequest, LlmPromptCachePolicy};
use sona_core::ports::llm::{LlmPortError, LlmPortErrorKind};

use crate::completion::{completion_input, reasoning_budget_tokens, structured_schema};
pub fn build_anthropic_payload_for_request(
    request: &LlmCompletionRequest,
    stream: bool,
) -> Result<Value, LlmPortError> {
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
            return Err(LlmPortError::new(
                LlmPortErrorKind::InvalidRequest,
                "Anthropic reasoning requires max_output_tokens to be greater than 1024",
            ));
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
