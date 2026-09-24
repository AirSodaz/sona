use serde_json::{Value, json};
use sona_core::llm::provider_protocol::{
    GeminiGenerateContentRequestParts as CoreGeminiGenerateContentRequestParts,
    build_gemini_generate_content_request_parts,
};
use sona_core::llm::runtime::LlmCompletionRequest;
use sona_core::llm::usage::TokenUsage;
use sona_core::ports::llm::LlmPortError;

use crate::completion::{completion_input, structured_schema};
use crate::transport::LlmApiUrl;
pub fn build_gemini_payload_for_request(
    request: &LlmCompletionRequest,
) -> Result<Value, LlmPortError> {
    let mut generation_config = json!({
        "temperature": request.effective_temperature().unwrap_or(0.7),
    });
    if let Some(max_output_tokens) = request.options.max_output_tokens {
        generation_config["maxOutputTokens"] = json!(max_output_tokens);
    }
    if request.effective_reasoning_enabled() {
        let thinking = request.effective_thinking_level();
        let is_budget_model = request.config.model.contains("gemini-2.5")
            || request.config.model.contains("thinking")
            || matches!(thinking, sona_core::llm::runtime::ThinkingLevel::Budget(_));
        generation_config["thinkingConfig"] = if is_budget_model {
            let raw_budget = thinking
                .resolve_budget_tokens(1024, 2048, 4096)
                .unwrap_or(2048);
            let budget = request
                .options
                .max_output_tokens
                .map(|limit| raw_budget.min(limit.min(u64::from(u32::MAX)) as u32))
                .unwrap_or(raw_budget);
            json!({
                "thinkingBudget": budget,
                "includeThoughts": true,
            })
        } else {
            let label = match thinking {
                sona_core::llm::runtime::ThinkingLevel::Minimal
                | sona_core::llm::runtime::ThinkingLevel::Low => "LOW",
                sona_core::llm::runtime::ThinkingLevel::High
                | sona_core::llm::runtime::ThinkingLevel::Xhigh
                | sona_core::llm::runtime::ThinkingLevel::Max => "HIGH",
                _ => "MEDIUM",
            };
            json!({
                "thinkingLevel": label,
                "includeThoughts": true,
            })
        };
    }
    if matches!(
        &request.options.response_format,
        sona_core::llm::runtime::LlmResponseFormat::JsonObject
    ) {
        generation_config["responseMimeType"] = json!("application/json");
    } else if let Some(schema) = structured_schema(request)? {
        generation_config["responseMimeType"] = json!("application/json");
        generation_config["responseJsonSchema"] = schema;
    }

    let mut payload = json!({
        "contents": [{"parts": [{"text": completion_input(request)}]}],
        "generationConfig": generation_config,
    });
    if let Some(system_prompt) = request.system_prompt.as_deref() {
        payload["systemInstruction"] = json!({"parts": [{"text": system_prompt}]});
    }
    Ok(payload)
}

pub fn extract_gemini_visible_text(response: &Value) -> Option<String> {
    let text = response
        .pointer("/candidates/0/content/parts")?
        .as_array()?
        .iter()
        .filter(|part| part.get("thought").and_then(Value::as_bool) != Some(true))
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("");
    (!text.is_empty()).then_some(text)
}

pub fn extract_gemini_thought(response: &Value) -> Option<String> {
    let thought = response
        .pointer("/candidates/0/content/parts")?
        .as_array()?
        .iter()
        .filter(|part| part.get("thought").and_then(Value::as_bool) == Some(true))
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("");
    (!thought.is_empty()).then_some(thought)
}

#[derive(Clone, Debug)]
pub struct GeminiGenerateContentRequestParts {
    pub url: LlmApiUrl,
    pub headers: Vec<(&'static str, String)>,
}

pub fn build_gemini_generate_content_request_parts_for_reqwest(
    base_url: &str,
    model: &str,
    api_key: &str,
    stream: bool,
) -> Result<GeminiGenerateContentRequestParts, LlmPortError> {
    let CoreGeminiGenerateContentRequestParts { url, headers } =
        build_gemini_generate_content_request_parts(base_url, model, api_key, stream)?;
    let url = LlmApiUrl::parse(&url)?;

    Ok(GeminiGenerateContentRequestParts { url, headers })
}

pub fn extract_gemini_usage(usage: &Value) -> Option<TokenUsage> {
    let prompt_tokens = usage
        .get("promptTokenCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let completion_tokens = usage
        .get("candidatesTokenCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let total_tokens = usage
        .get("totalTokenCount")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| prompt_tokens.saturating_add(completion_tokens));
    if prompt_tokens == 0 && completion_tokens == 0 && total_tokens == 0 {
        return None;
    }
    Some(TokenUsage {
        prompt_tokens,
        completion_tokens,
        total_tokens,
        cached_input_tokens: usage
            .get("cachedContentTokenCount")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        reasoning_tokens: usage
            .get("thoughtsTokenCount")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        ..TokenUsage::default()
    })
}
