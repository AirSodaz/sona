use serde_json::{Value, json};
use sona_core::llm::runtime::{LlmCompletionRequest, LlmPromptCachePolicy};
use sona_core::ports::llm::{LlmPortError, LlmPortErrorKind};

use crate::completion::{completion_input, structured_schema};
fn sanitize_anthropic_schema(schema: &mut Value) {
    match schema {
        Value::Object(map) => {
            if let Some(t) = map.get("type").and_then(Value::as_str)
                && t == "object"
                && !map.contains_key("additionalProperties")
            {
                map.insert("additionalProperties".to_string(), Value::Bool(false));
            }
            if let Some(properties) = map.get_mut("properties").and_then(Value::as_object_mut) {
                for prop in properties.values_mut() {
                    sanitize_anthropic_schema(prop);
                }
            }
            if let Some(items) = map.get_mut("items") {
                sanitize_anthropic_schema(items);
            }
            if let Some(defs) = map.get_mut("$defs").and_then(Value::as_object_mut) {
                for def in defs.values_mut() {
                    sanitize_anthropic_schema(def);
                }
            }
            if let Some(definitions) = map.get_mut("definitions").and_then(Value::as_object_mut) {
                for def in definitions.values_mut() {
                    sanitize_anthropic_schema(def);
                }
            }
        }
        Value::Array(arr) => {
            for item in arr {
                sanitize_anthropic_schema(item);
            }
        }
        _ => {}
    }
}

pub fn build_anthropic_payload_for_request(
    request: &LlmCompletionRequest,
    stream: bool,
) -> Result<Value, LlmPortError> {
    let capabilities = request.capabilities();
    let max_tokens = request.options.max_output_tokens.unwrap_or(8192);
    let mut payload = json!({
        "model": request.config.model,
        "messages": [{"role": "user", "content": completion_input(request)}],
        "max_tokens": max_tokens,
        "stream": stream,
    });
    if capabilities.supports_temperature && !request.effective_reasoning_enabled() {
        payload["temperature"] = json!(request.effective_temperature().unwrap_or(0.7));
    }
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
        if capabilities.force_adaptive_thinking {
            payload["thinking"] = json!({
                "type": "adaptive",
                "display": "summarized",
            });
            let thinking = request.effective_thinking_level();
            let effort = match thinking {
                sona_core::llm::runtime::ThinkingLevel::Minimal
                | sona_core::llm::runtime::ThinkingLevel::Low => "low",
                sona_core::llm::runtime::ThinkingLevel::High
                | sona_core::llm::runtime::ThinkingLevel::Xhigh => "high",
                sona_core::llm::runtime::ThinkingLevel::Max => "max",
                _ => "medium",
            };
            let mut output_config = payload
                .get("output_config")
                .cloned()
                .unwrap_or_else(|| json!({}));
            output_config["effort"] = json!(effort);
            payload["output_config"] = output_config;
        } else {
            let max_budget = max_tokens.saturating_sub(1).min(u64::from(u32::MAX)) as u32;
            if max_budget < 1024 {
                return Err(LlmPortError::new(
                    LlmPortErrorKind::InvalidRequest,
                    "Anthropic reasoning requires max_output_tokens to be greater than 1024",
                ));
            }
            let thinking = request.effective_thinking_level();
            let raw_budget = thinking
                .resolve_budget_tokens(1024, 2048, 4096)
                .unwrap_or(2048);
            let budget_tokens = raw_budget.clamp(1024, max_budget);
            payload["thinking"] = json!({
                "type": "enabled",
                "budget_tokens": budget_tokens,
                "display": "summarized",
            });
            payload["temperature"] = json!(1.0);
        }
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
    if let Some(mut schema) = structured_schema(request)? {
        sanitize_anthropic_schema(&mut schema);
        let mut output_config = payload
            .get("output_config")
            .cloned()
            .unwrap_or_else(|| json!({}));
        output_config["format"] = json!({
            "type": "json_schema",
            "schema": schema,
        });
        payload["output_config"] = output_config;
    }
    Ok(payload)
}
