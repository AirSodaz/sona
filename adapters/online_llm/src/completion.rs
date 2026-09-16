use rig_core::completion::{CompletionModel, CompletionRequestBuilder};
use serde_json::{Value, json};
use sona_core::llm::provider_protocol::{
    MessageRole, StandardLlmRequest, StandardLlmResponse, build_standard_input,
    normalize_token_usage, strategy_uses_openai_chat_payload,
};
use sona_core::llm::runtime::{LlmCompletionRequest, LlmResponseFormat};
use sona_core::llm::tasks::LlmProviderStrategy;
use sona_core::llm::usage::TokenUsage;
use sona_core::ports::llm::{LlmPortError, LlmPortErrorKind};

use crate::providers::GoogleTranslateAdapter;
use crate::transport::LlmApiUrl;

pub(crate) fn build_rig_completion_request<M>(
    model: M,
    request: &LlmCompletionRequest,
) -> Result<CompletionRequestBuilder<M>, LlmPortError>
where
    M: CompletionModel + Clone,
{
    let input = completion_input(request);
    let mut builder = model
        .completion_request(input)
        .temperature_opt(request.effective_temperature().map(f64::from))
        .max_tokens_opt(request.options.max_output_tokens);

    if let Some(system_prompt) = request
        .system_prompt
        .as_deref()
        .filter(|prompt| !prompt.trim().is_empty())
    {
        builder = builder.preamble(system_prompt.to_string());
    }

    let mut extra_params = serde_json::Map::new();

    match &request.options.response_format {
        LlmResponseFormat::Text => {}
        LlmResponseFormat::JsonObject => {
            if let Some(params) = rig_json_object_parameters(request.config.strategy)
                && let Some(obj) = params.as_object()
            {
                for (k, v) in obj {
                    extra_params.insert(k.clone(), v.clone());
                }
            }
        }
        LlmResponseFormat::JsonSchema { name, schema } => {
            if request.config.strategy == LlmProviderStrategy::OpenAi
                || request.config.strategy == LlmProviderStrategy::AzureOpenAi
            {
                let mut schema = schema.clone();
                if let Some(object) = schema.as_object_mut() {
                    object
                        .entry("title".to_string())
                        .or_insert_with(|| Value::String(name.clone()));
                }
                builder =
                    builder.output_schema(schemars::Schema::try_from(schema).map_err(|error| {
                        LlmPortError::new(
                            LlmPortErrorKind::InvalidRequest,
                            format!("Invalid JSON Schema: {error}"),
                        )
                    })?);
            } else if let Some(params) = rig_json_object_parameters(request.config.strategy)
                && let Some(obj) = params.as_object()
            {
                for (k, v) in obj {
                    extra_params.insert(k.clone(), v.clone());
                }
            }
        }
    }

    builder = configure_rig_reasoning(builder, request, &mut extra_params)?;

    if !extra_params.is_empty() {
        builder = builder.additional_params(Value::Object(extra_params));
    }

    Ok(builder)
}

fn configure_rig_reasoning<M>(
    mut builder: CompletionRequestBuilder<M>,
    request: &LlmCompletionRequest,
    extra_params: &mut serde_json::Map<String, Value>,
) -> Result<CompletionRequestBuilder<M>, LlmPortError>
where
    M: CompletionModel + Clone,
{
    if !request.effective_reasoning_enabled() {
        return Ok(builder);
    }

    let level = request.effective_reasoning_level().unwrap_or("medium");
    let budget = reasoning_budget_tokens(request.effective_reasoning_level());

    match request.config.strategy {
        LlmProviderStrategy::Anthropic => {
            let max_tokens = request.options.max_output_tokens.unwrap_or(8192);
            let max_budget = max_tokens.saturating_sub(1).min(u64::from(u32::MAX)) as u32;
            if max_budget < 1024 {
                return Err(LlmPortError::new(
                    LlmPortErrorKind::InvalidRequest,
                    "Anthropic reasoning requires max_output_tokens to be greater than 1024",
                ));
            }
            extra_params.insert(
                "thinking".to_string(),
                json!({
                    "type": "enabled",
                    "budget_tokens": budget.min(max_budget),
                }),
            );
            builder = builder.temperature(1.0);
        }
        LlmProviderStrategy::Gemini => {
            let thinking_config = if request.config.model.contains("gemini-2.5") {
                let budget = request
                    .options
                    .max_output_tokens
                    .map(|limit| budget.min(limit.min(u64::from(u32::MAX)) as u32))
                    .unwrap_or(budget);
                json!({
                    "thinking_budget": budget,
                    "include_thoughts": true,
                })
            } else {
                let gemini_level =
                    reasoning_level_label(request.effective_reasoning_level()).to_lowercase();
                json!({
                    "thinking_level": gemini_level,
                    "include_thoughts": true,
                })
            };
            if let Some(existing) = extra_params
                .get_mut("generation_config")
                .and_then(Value::as_object_mut)
            {
                existing.insert("thinking_config".to_string(), thinking_config);
            } else {
                extra_params.insert(
                    "generation_config".to_string(),
                    json!({
                        "thinking_config": thinking_config,
                    }),
                );
            }
        }
        LlmProviderStrategy::OpenAiResponses => {
            extra_params.insert(
                "reasoning".to_string(),
                json!({
                    "effort": level,
                }),
            );
        }
        LlmProviderStrategy::Ollama => {
            extra_params.insert("think".to_string(), json!(true));
        }
        LlmProviderStrategy::DeepSeek => {
            extra_params.insert(
                "thinking".to_string(),
                json!({
                    "type": "enabled",
                }),
            );
            extra_params.insert("reasoning_effort".to_string(), json!(level));
        }
        _ => {
            extra_params.insert("reasoning_effort".to_string(), json!(level));
        }
    }

    Ok(builder)
}
fn rig_json_object_parameters(strategy: LlmProviderStrategy) -> Option<Value> {
    if strategy_uses_openai_chat_payload(strategy)
        || matches!(
            strategy,
            LlmProviderStrategy::AzureOpenAi
                | LlmProviderStrategy::Copilot
                | LlmProviderStrategy::Perplexity
        )
    {
        return Some(json!({"response_format": {"type": "json_object"}}));
    }
    (strategy == LlmProviderStrategy::Gemini)
        .then(|| json!({"generationConfig": {"responseMimeType": "application/json"}}))
}

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

pub fn extract_text_response(
    choice: &[rig_core::completion::AssistantContent],
) -> Result<String, LlmPortError> {
    let parts = choice
        .iter()
        .filter_map(|content| match content {
            rig_core::completion::AssistantContent::Text(text) => Some(text.text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();

    if parts.is_empty() {
        return Err(LlmPortError::new(
            LlmPortErrorKind::Protocol,
            "LLM response did not contain text output",
        ));
    }

    Ok(parts.join("\n"))
}

pub fn token_usage_from_rig_usage(
    usage: Option<rig_core::completion::Usage>,
) -> Option<TokenUsage> {
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
    match request.config.strategy {
        LlmProviderStrategy::GoogleTranslate | LlmProviderStrategy::GoogleTranslateFree => {
            let url = LlmApiUrl::parse(&request.config.base_url)?;
            let client = url.client(request.config.timeout_seconds)?;
            GoogleTranslateAdapter.generate(&client, &request).await
        }
        _ => crate::rig_adapter::execute_rig_completion(&request).await,
    }
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
    fn rig_json_object_parameters_match_protocols() {
        assert_eq!(
            rig_json_object_parameters(LlmProviderStrategy::OpenAi),
            Some(json!({"response_format": {"type": "json_object"}}))
        );
        assert_eq!(
            rig_json_object_parameters(LlmProviderStrategy::Gemini),
            Some(json!({"generationConfig": {"responseMimeType": "application/json"}}))
        );
        assert_eq!(
            rig_json_object_parameters(LlmProviderStrategy::Anthropic),
            None
        );
    }

    #[test]
    fn configure_rig_reasoning_maps_thinking_budget_and_level_across_models() {
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
