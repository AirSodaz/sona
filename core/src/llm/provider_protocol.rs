use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::llm::tasks::LlmProviderStrategy;
use crate::llm::usage::TokenUsage;

#[cfg(feature = "specta")]
use specta::Type;

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
pub enum MessageRole {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "assistant")]
    Assistant,
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
pub struct StandardMessage {
    pub role: MessageRole,
    pub content: String,
}

#[derive(Debug)]
#[cfg_attr(feature = "specta", derive(Type))]
pub struct StandardLlmRequest {
    pub messages: Vec<StandardMessage>,
    pub temperature: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
pub struct StandardLlmResponse {
    pub text: String,
    pub usage: Option<TokenUsage>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct LlmModelSummary {
    pub model: String,
    pub input_price: Option<f64>,
    pub output_price: Option<f64>,
    pub context_window: Option<u64>,
    pub max_output_tokens: Option<u64>,
    pub supports_multimodal: Option<bool>,
    pub supports_tools: Option<bool>,
    pub supports_reasoning: Option<bool>,
}

#[derive(Deserialize)]
pub struct OpenAiModel {
    pub id: String,
}

#[derive(Deserialize)]
pub struct OpenAiModelsResponse {
    pub data: Vec<OpenAiModel>,
}

#[derive(Deserialize)]
pub struct OllamaModel {
    pub name: String,
}

#[derive(Deserialize)]
pub struct OllamaTagsResponse {
    pub models: Vec<OllamaModel>,
}

#[derive(Deserialize)]
pub struct GeminiModel {
    pub name: String,
    #[serde(rename = "supportedGenerationMethods")]
    pub supported_generation_methods: Option<Vec<String>>,
    #[serde(rename = "inputTokenLimit")]
    pub input_token_limit: Option<u64>,
    #[serde(rename = "outputTokenLimit")]
    pub output_token_limit: Option<u64>,
}

#[derive(Deserialize)]
pub struct GeminiModelsResponse {
    pub models: Option<Vec<GeminiModel>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeminiGenerateContentRequestParts {
    pub url: String,
    pub headers: Vec<(&'static str, String)>,
}

pub fn strategy_uses_openai_chat_payload(strategy: LlmProviderStrategy) -> bool {
    matches!(
        strategy,
        LlmProviderStrategy::OpenAi
            | LlmProviderStrategy::OpenAiCompatible
            | LlmProviderStrategy::OpenAiCompatibleCustomPath
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
            | LlmProviderStrategy::MistralAi
            | LlmProviderStrategy::Chatglm
            | LlmProviderStrategy::Volcengine
    )
}

pub fn clean_gemini_base_url(base_url: &str) -> &str {
    let base = base_url.trim_end_matches('/');
    let suffixes = [
        "/v1beta/models",
        "/v1/models",
        "/v1beta/openai",
        "/v1/openai",
        "/models",
        "/v1beta",
        "/v1",
    ];

    for suffix in suffixes {
        if let Some(stripped) = base.strip_suffix(suffix) {
            return stripped;
        }
    }

    base
}

pub fn format_gemini_models_url(base_url: &str) -> String {
    let cleaned_base = clean_gemini_base_url(base_url);

    format!("{}/v1beta/models", cleaned_base)
}

pub fn build_gemini_generate_content_request_parts(
    base_url: &str,
    model: &str,
    api_key: &str,
    stream: bool,
) -> Result<GeminiGenerateContentRequestParts, String> {
    let cleaned_base = clean_gemini_base_url(base_url);
    let model = model.trim().trim_start_matches("models/");
    if model.is_empty() {
        return Err("Gemini model cannot be empty".to_string());
    }

    let action = if stream {
        "streamGenerateContent"
    } else {
        "generateContent"
    };
    let mut url = format!("{}/v1beta/models/{}:{}", cleaned_base, model, action);
    if stream {
        url.push_str("?alt=sse");
    }

    let mut headers = Vec::new();
    if !api_key.is_empty() {
        headers.push(("x-goog-api-key", api_key.to_string()));
    }

    Ok(GeminiGenerateContentRequestParts { url, headers })
}

pub fn is_gemini_text_generation_model(model: &GeminiModel) -> bool {
    model
        .supported_generation_methods
        .as_ref()
        .map(|methods| methods.iter().any(|method| method == "generateContent"))
        .unwrap_or(true)
}

pub fn gemini_model_to_summary(model: GeminiModel) -> Option<LlmModelSummary> {
    if !is_gemini_text_generation_model(&model) {
        return None;
    }

    let supports_tools = model
        .supported_generation_methods
        .as_ref()
        .map(|methods| methods.iter().any(|method| method == "generateContent"));

    Some(LlmModelSummary {
        model: model.name.trim_start_matches("models/").to_string(),
        input_price: None,
        output_price: None,
        context_window: model.input_token_limit,
        max_output_tokens: model.output_token_limit,
        supports_multimodal: Some(true),
        supports_tools,
        supports_reasoning: None,
    })
}

pub fn openai_model_to_summary(model: OpenAiModel) -> LlmModelSummary {
    LlmModelSummary {
        model: model.id,
        input_price: None,
        output_price: None,
        context_window: None,
        max_output_tokens: None,
        supports_multimodal: None,
        supports_tools: None,
        supports_reasoning: None,
    }
}

pub fn ollama_model_to_summary(model: OllamaModel) -> LlmModelSummary {
    LlmModelSummary {
        model: model.name,
        input_price: None,
        output_price: None,
        context_window: None,
        max_output_tokens: None,
        supports_multimodal: None,
        supports_tools: None,
        supports_reasoning: None,
    }
}

pub fn format_openai_models_urls(base_url: &str, is_ollama: bool) -> Vec<String> {
    let base = base_url.trim_end_matches('/');

    if is_ollama {
        return vec![format!("{}/api/tags", base), format!("{}/v1/models", base)];
    }

    if base.ends_with("/v1") {
        vec![format!("{}/models", base)]
    } else {
        vec![format!("{}/v1/models", base), format!("{}/models", base)]
    }
}

pub fn strategy_supports_model_listing(strategy: LlmProviderStrategy) -> bool {
    !matches!(
        strategy,
        LlmProviderStrategy::Anthropic
            | LlmProviderStrategy::AzureOpenAi
            | LlmProviderStrategy::Volcengine
            | LlmProviderStrategy::Perplexity
            | LlmProviderStrategy::Copilot
            | LlmProviderStrategy::OpenAiCompatibleCustomPath
            | LlmProviderStrategy::GoogleTranslate
            | LlmProviderStrategy::GoogleTranslateFree
    )
}

pub fn join_url(base_url: &str, path: &str) -> String {
    let base = base_url.trim_end_matches('/');
    let path = path.trim_start_matches('/');

    for prefix in ["v1/", "v1beta/"] {
        let base_suffix = format!("/{}", prefix.trim_end_matches('/'));
        if base.ends_with(&base_suffix) && path.starts_with(prefix) {
            return format!("{}/{}", &base[..base.len() - base_suffix.len()], path);
        }
    }

    format!("{}/{}", base, path)
}

fn extract_text_parts(value: &Value, parts: &mut Vec<String>) {
    match value {
        Value::String(text) if !text.is_empty() => parts.push(text.clone()),
        Value::String(_) => {}
        Value::Array(items) => {
            for item in items {
                extract_text_parts(item, parts);
            }
        }
        Value::Object(map) => {
            if let Some(text) = map
                .get("output_text")
                .and_then(Value::as_str)
                .filter(|text| !text.is_empty())
            {
                parts.push(text.to_string());
            }

            if let Some(text) = map
                .get("text")
                .and_then(Value::as_str)
                .filter(|text| !text.is_empty())
            {
                parts.push(text.to_string());
                return;
            }

            if let Some(content) = map.get("content").or_else(|| map.get("message")) {
                extract_text_parts(content, parts);
                return;
            }

            if let Some(output) = map.get("output") {
                extract_text_parts(output, parts);
            }
        }
        _ => {}
    }
}

pub fn extract_text_from_json_response(response: &Value) -> Result<String, String> {
    let mut parts = Vec::new();

    if let Some(output_text) = response.get("output_text").and_then(Value::as_str)
        && !output_text.is_empty()
    {
        return Ok(output_text.to_string());
    }

    if let Some(choices) = response.get("choices") {
        extract_text_parts(choices, &mut parts);
    }

    if parts.is_empty()
        && let Some(output) = response.get("output")
    {
        extract_text_parts(output, &mut parts);
    }

    if parts.is_empty() {
        extract_text_parts(response, &mut parts);
    }

    let text = parts
        .into_iter()
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    if text.is_empty() {
        return Err("LLM response did not contain text output".to_string());
    }

    Ok(text)
}

pub fn normalize_token_usage(
    prompt_tokens: u64,
    completion_tokens: u64,
    total_tokens: u64,
) -> Option<TokenUsage> {
    let normalized_total = if total_tokens > 0 {
        total_tokens
    } else {
        prompt_tokens.saturating_add(completion_tokens)
    };

    if prompt_tokens == 0 && completion_tokens == 0 && normalized_total == 0 {
        return None;
    }

    Some(TokenUsage {
        prompt_tokens: prompt_tokens.min(u32::MAX as u64) as u32,
        completion_tokens: completion_tokens.min(u32::MAX as u64) as u32,
        total_tokens: normalized_total.min(u32::MAX as u64) as u32,
    })
}

pub fn extract_anthropic_text_response(
    response: &Value,
) -> Result<(String, Option<TokenUsage>), String> {
    let content = response
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(|| "Anthropic response missing content array".to_string())?;

    let text_parts: Vec<&str> = content
        .iter()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|block| block.get("text").and_then(Value::as_str))
        .collect();

    if text_parts.is_empty() {
        return Err("Anthropic response did not contain text output".to_string());
    }

    let usage = response.get("usage").and_then(|u| {
        normalize_token_usage(
            u.get("input_tokens").and_then(Value::as_u64).unwrap_or(0),
            u.get("output_tokens").and_then(Value::as_u64).unwrap_or(0),
            0,
        )
    });

    Ok((text_parts.join("\n"), usage))
}

pub fn extract_usage_from_json_response(response: &Value) -> Option<TokenUsage> {
    let usage = response.get("usage")?;

    let prompt_tokens = usage
        .get("prompt_tokens")
        .or_else(|| usage.get("input_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let completion_tokens = usage
        .get("completion_tokens")
        .or_else(|| usage.get("output_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let total_tokens = usage
        .get("total_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);

    normalize_token_usage(prompt_tokens, completion_tokens, total_tokens)
}

pub fn build_standard_input(req: &StandardLlmRequest) -> String {
    req.messages
        .iter()
        .filter(|message| matches!(message.role, MessageRole::User))
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}
