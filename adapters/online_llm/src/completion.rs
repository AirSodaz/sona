use serde_json::Value;
use sona_core::llm::provider_protocol::{
    MessageRole, StandardLlmRequest, StandardLlmResponse, build_standard_input,
    normalize_token_usage,
};
use sona_core::llm::runtime::{LlmCompletionRequest, LlmResponseFormat};
use sona_core::llm::usage::TokenUsage;
use sona_core::ports::llm::{LlmPortError, LlmPortErrorKind};

pub(crate) fn completion_input(request: &LlmCompletionRequest) -> String {
    if matches!(
        &request.options.response_format,
        LlmResponseFormat::JsonObject
    ) {
        format!(
            "{}\n\nReturn only valid JSON with an object as the top-level value. Do not use Markdown fences.",
            request.input
        )
    } else {
        request.input.clone()
    }
}

pub(crate) fn structured_schema(
    request: &LlmCompletionRequest,
) -> Result<Option<Value>, LlmPortError> {
    let schema = match &request.options.response_format {
        LlmResponseFormat::Text => return Ok(None),
        LlmResponseFormat::JsonObject => return Ok(None),
        LlmResponseFormat::JsonSchema { schema, .. } => schema.clone(),
    };
    if !schema.is_object() && !schema.is_boolean() {
        return Err(LlmPortError::new(
            LlmPortErrorKind::InvalidRequest,
            "JSON Schema must be an object or boolean",
        ));
    }
    Ok(Some(schema))
}

pub(crate) fn reasoning_budget_tokens(reasoning_level: Option<&str>) -> u32 {
    match reasoning_level {
        Some("low") => 1024,
        Some("high") => 4096,
        _ => 2048,
    }
}

pub(crate) fn reasoning_level_label(reasoning_level: Option<&str>) -> &'static str {
    match reasoning_level {
        Some("low") => "LOW",
        Some("high") => "HIGH",
        _ => "MEDIUM",
    }
}

#[derive(Clone, Debug, Default)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub cached_input_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub tool_use_prompt_tokens: u64,
    pub reasoning_tokens: u64,
}

pub fn token_usage_from_rig_usage(usage: Option<Usage>) -> Option<TokenUsage> {
    usage.and_then(|usage| {
        let mut normalized =
            normalize_token_usage(usage.input_tokens, usage.output_tokens, usage.total_tokens)?;
        normalized.cached_input_tokens = usage.cached_input_tokens;
        normalized.cache_creation_input_tokens = usage.cache_creation_input_tokens;
        normalized.reasoning_tokens = usage.reasoning_tokens;
        Some(normalized)
    })
}

pub async fn complete_with_provider(
    request: LlmCompletionRequest,
) -> Result<StandardLlmResponse, LlmPortError> {
    crate::native_completion::execute_native_completion(&request).await
}

pub fn build_standard_user_input(input: impl Into<String>, temperature: f32) -> String {
    build_standard_input(&StandardLlmRequest {
        messages: vec![sona_core::llm::provider_protocol::StandardMessage {
            role: MessageRole::User,
            content: input.into(),
        }],
        temperature,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configure_reasoning_maps_thinking_budget_and_level_across_models() {
        assert_eq!(reasoning_budget_tokens(Some("low")), 1024);
        assert_eq!(reasoning_budget_tokens(Some("high")), 4096);
        assert_eq!(reasoning_budget_tokens(Some("medium")), 2048);
        assert_eq!(reasoning_budget_tokens(None), 2048);

        assert_eq!(reasoning_level_label(Some("low")), "LOW");
        assert_eq!(reasoning_level_label(Some("high")), "HIGH");
        assert_eq!(reasoning_level_label(Some("medium")), "MEDIUM");
        assert_eq!(reasoning_level_label(None), "MEDIUM");
    }
}
