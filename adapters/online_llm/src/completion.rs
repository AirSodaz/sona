use async_trait::async_trait;
use reqwest::Client;
use rig_core::completion::{CompletionModel, CompletionRequestBuilder};
use serde_json::{Value, json};
use sona_core::llm::provider_protocol::{
    MessageRole, StandardLlmRequest, StandardLlmResponse, build_standard_input,
    normalize_token_usage, strategy_uses_openai_chat_payload,
};
use sona_core::llm::runtime::{LlmCompletionRequest, LlmResponseFormat};
use sona_core::llm::tasks::LlmProviderStrategy;
use sona_core::llm::usage::TokenUsage;
use sona_core::ports::llm::LlmPortError;

use crate::anthropic::AnthropicAdapter;
use crate::gemini::GeminiAdapter;
use crate::ollama::OllamaAdapter;
use crate::openai_compatible::{AzureAdapter, CopilotAdapter, OpenAiAdapter, PerplexityAdapter};
use crate::providers::{GenericHttpAdapter, GoogleTranslateAdapter};
use crate::transport::{LlmApiUrl, classify_llm_port_error};

#[async_trait]
pub(crate) trait LlmAdapter: Send + Sync {
    async fn generate(
        &self,
        client: &Client,
        request: &LlmCompletionRequest,
    ) -> Result<StandardLlmResponse, LlmPortError>;
}

pub(crate) fn port_result<T>(result: Result<T, String>) -> Result<T, LlmPortError> {
    result.map_err(classify_llm_port_error)
}

pub(crate) fn build_rig_completion_request<M>(
    model: M,
    request: &LlmCompletionRequest,
) -> Result<CompletionRequestBuilder<M>, String>
where
    M: CompletionModel,
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

    match &request.options.response_format {
        LlmResponseFormat::Text => {}
        LlmResponseFormat::JsonObject => {
            if let Some(params) = rig_json_object_parameters(request.config.strategy) {
                builder = builder.additional_params(params);
            }
        }
        LlmResponseFormat::JsonSchema { name, schema } => {
            let mut schema = schema.clone();
            if let Some(object) = schema.as_object_mut() {
                object
                    .entry("title".to_string())
                    .or_insert_with(|| Value::String(name.clone()));
            }
            builder = builder.output_schema(
                schemars::Schema::try_from(schema)
                    .map_err(|error| format!("Invalid JSON Schema: {error}"))?,
            );
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

pub(crate) fn structured_schema(request: &LlmCompletionRequest) -> Result<Option<Value>, String> {
    let schema = match &request.options.response_format {
        LlmResponseFormat::Text => return Ok(None),
        LlmResponseFormat::JsonObject => return Ok(None),
        LlmResponseFormat::JsonSchema { schema, .. } => schema.clone(),
    };
    if !schema.is_object() && !schema.is_boolean() {
        return Err("JSON Schema must be an object or boolean".to_string());
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
    choice: &rig_core::OneOrMany<rig_core::completion::AssistantContent>,
) -> Result<String, String> {
    let parts = choice
        .iter()
        .filter_map(|content| match content {
            rig_core::completion::AssistantContent::Text(text) => Some(text.text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();

    if parts.is_empty() {
        return Err("LLM response did not contain text output".to_string());
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

struct AdapterFactory;

impl AdapterFactory {
    fn create(strategy: LlmProviderStrategy) -> Box<dyn LlmAdapter> {
        match strategy {
            LlmProviderStrategy::OpenAi
            | LlmProviderStrategy::DeepSeek
            | LlmProviderStrategy::MoonshotAi
            | LlmProviderStrategy::MoonshotCn
            | LlmProviderStrategy::Xiaomi
            | LlmProviderStrategy::Kimi
            | LlmProviderStrategy::SiliconFlow
            | LlmProviderStrategy::Qwen
            | LlmProviderStrategy::QwenPortal
            | LlmProviderStrategy::MinimaxGlobal
            | LlmProviderStrategy::MinimaxCn
            | LlmProviderStrategy::OpenRouter
            | LlmProviderStrategy::LmStudio
            | LlmProviderStrategy::Groq
            | LlmProviderStrategy::XAi
            | LlmProviderStrategy::MistralAi => Box::new(OpenAiAdapter),
            LlmProviderStrategy::Anthropic => Box::new(AnthropicAdapter),
            LlmProviderStrategy::Ollama => Box::new(OllamaAdapter),
            LlmProviderStrategy::Gemini => Box::new(GeminiAdapter),
            LlmProviderStrategy::GoogleTranslate | LlmProviderStrategy::GoogleTranslateFree => {
                Box::new(GoogleTranslateAdapter)
            }
            LlmProviderStrategy::AzureOpenAi => Box::new(AzureAdapter),
            LlmProviderStrategy::Copilot => Box::new(CopilotAdapter),
            LlmProviderStrategy::Perplexity => Box::new(PerplexityAdapter),
            _ => Box::new(GenericHttpAdapter),
        }
    }
}

pub async fn complete_with_provider(
    request: LlmCompletionRequest,
) -> Result<StandardLlmResponse, LlmPortError> {
    let adapter = AdapterFactory::create(request.config.strategy);
    let url = port_result(LlmApiUrl::parse(&request.config.base_url))?;
    let client = port_result(url.client(request.config.timeout_seconds))?;
    adapter.generate(&client, &request).await
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
}
