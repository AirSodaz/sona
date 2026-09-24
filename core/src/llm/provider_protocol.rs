use crate::llm::runtime::{ReasoningMode, ThinkingLevel};
use crate::llm::tasks::LlmProviderStrategy;
use crate::llm::usage::TokenUsage;
use crate::ports::llm::{LlmPortError, LlmPortErrorKind};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg(feature = "specta")]
use specta::Type;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "snake_case")]
pub enum LlmModality {
    Text,
    Image,
    Audio,
    Video,
    Pdf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "snake_case")]
pub enum LlmModelMetadataSource {
    Provider,
    ModelsDev,
}

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thought: Option<String>,
    pub usage: Option<TokenUsage>,
}

impl StandardLlmResponse {
    pub fn new(text: impl Into<String>, usage: Option<TokenUsage>) -> Self {
        Self {
            text: text.into(),
            thought: None,
            usage,
        }
    }

    pub fn with_thought(mut self, thought: impl Into<String>) -> Self {
        self.thought = Some(thought.into());
        self
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct LlmModelSummary {
    pub model: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Number>)
    )]
    pub input_price: Option<f64>,
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Number>)
    )]
    pub output_price: Option<f64>,
    #[serde(default)]
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Number>)
    )]
    pub cache_read_price: Option<f64>,
    #[serde(default)]
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Number>)
    )]
    pub cache_write_price: Option<f64>,
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Number>)
    )]
    pub context_window: Option<u64>,
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Number>)
    )]
    pub max_output_tokens: Option<u64>,
    #[serde(default)]
    pub knowledge_cutoff: Option<String>,
    #[serde(default)]
    pub release_date: Option<String>,
    #[serde(default)]
    pub last_updated: Option<String>,
    #[serde(default)]
    pub input_modalities: Vec<LlmModality>,
    #[serde(default)]
    pub output_modalities: Vec<LlmModality>,
    pub supports_multimodal: Option<bool>,
    pub supports_tools: Option<bool>,
    pub supports_reasoning: Option<bool>,
    #[serde(default)]
    pub supports_temperature: Option<bool>,
    pub supports_structured_output: Option<bool>,
    #[serde(default)]
    pub supports_prompt_caching: Option<bool>,
    #[serde(default)]
    pub metadata_sources: Vec<LlmModelMetadataSource>,
    #[serde(default)]
    pub reasoning_mode: Option<ReasoningMode>,
    #[serde(default)]
    pub token_limit_key: Option<String>,
    #[serde(default)]
    pub supported_thinking_levels: Vec<ThinkingLevel>,
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
            | LlmProviderStrategy::Together
            | LlmProviderStrategy::Venice
            | LlmProviderStrategy::Hyperbolic
            | LlmProviderStrategy::Llamafile
    )
}

pub fn strategy_supports_structured_output(strategy: LlmProviderStrategy) -> Option<bool> {
    match strategy {
        LlmProviderStrategy::DeepSeek
        | LlmProviderStrategy::MoonshotAi
        | LlmProviderStrategy::MoonshotCn
        | LlmProviderStrategy::Xiaomi
        | LlmProviderStrategy::Kimi
        | LlmProviderStrategy::SiliconFlow
        | LlmProviderStrategy::Qwen
        | LlmProviderStrategy::QwenPortal
        | LlmProviderStrategy::MinimaxGlobal
        | LlmProviderStrategy::MinimaxCn
        | LlmProviderStrategy::Chatglm
        | LlmProviderStrategy::Volcengine
        | LlmProviderStrategy::Perplexity
        | LlmProviderStrategy::GoogleTranslate
        | LlmProviderStrategy::GoogleTranslateFree
        | LlmProviderStrategy::Cohere
        | LlmProviderStrategy::Together
        | LlmProviderStrategy::Venice
        | LlmProviderStrategy::Hyperbolic
        | LlmProviderStrategy::Llamafile
        | LlmProviderStrategy::Local => Some(false),
        _ => None,
    }
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
) -> Result<GeminiGenerateContentRequestParts, LlmPortError> {
    let cleaned_base = clean_gemini_base_url(base_url);
    let model = model.trim().trim_start_matches("models/");
    if model.is_empty() {
        return Err(LlmPortError::new(
            LlmPortErrorKind::InvalidRequest,
            "Gemini model cannot be empty",
        ));
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
        supports_structured_output: None,
        metadata_sources: vec![LlmModelMetadataSource::Provider],
        ..LlmModelSummary::default()
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
        supports_structured_output: None,
        metadata_sources: vec![LlmModelMetadataSource::Provider],
        ..LlmModelSummary::default()
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
        supports_structured_output: None,
        metadata_sources: vec![LlmModelMetadataSource::Provider],
        ..LlmModelSummary::default()
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

pub fn strip_and_extract_inline_thoughts(input: &str) -> (String, Option<String>) {
    const TAGS: &[(&str, &str)] = &[
        ("<think>", "</think>"),
        ("<thought>", "</thought>"),
        ("<thinking>", "</thinking>"),
        ("<reasoning>", "</reasoning>"),
    ];

    let mut remaining = input;
    let mut clean_text = String::with_capacity(input.len());
    let mut thoughts = Vec::new();

    while !remaining.is_empty() {
        let mut earliest_match: Option<(usize, usize, &'static str)> = None;
        let lower = remaining.to_lowercase();

        for (open_tag, close_tag) in TAGS {
            if let Some(idx) = lower.find(open_tag) {
                let end = idx + open_tag.len();
                if let Some((best_idx, _, _)) = earliest_match {
                    if idx < best_idx {
                        earliest_match = Some((idx, end, close_tag));
                    }
                } else {
                    earliest_match = Some((idx, end, close_tag));
                }
            }
        }

        if let Some((open_start, open_end, close_tag)) = earliest_match {
            if open_start > 0 {
                clean_text.push_str(&remaining[..open_start]);
            }
            remaining = &remaining[open_end..];
            let remaining_lower = remaining.to_lowercase();
            if let Some(close_idx) = remaining_lower.find(close_tag) {
                let thought_content = &remaining[..close_idx];
                if !thought_content.trim().is_empty() {
                    thoughts.push(thought_content.trim().to_string());
                }
                let mut after_close = close_idx + close_tag.len();
                if remaining[after_close..].starts_with("\r\n") {
                    after_close += 2;
                } else if remaining[after_close..].starts_with('\n') {
                    after_close += 1;
                }
                remaining = &remaining[after_close..];
            } else {
                if !remaining.trim().is_empty() {
                    thoughts.push(remaining.trim().to_string());
                }
                remaining = "";
            }
        } else {
            clean_text.push_str(remaining);
            break;
        }
    }

    let thought_opt = if thoughts.is_empty() {
        None
    } else {
        Some(thoughts.join("\n\n"))
    };

    (clean_text, thought_opt)
}

pub fn extract_text_and_thought_from_json_response(
    response: &Value,
) -> Result<(String, Option<String>), LlmPortError> {
    let mut explicit_thoughts = Vec::new();

    // Extract from choices[0].message (OpenAI compatible / DeepSeek)
    if let Some(choice) = response.pointer("/choices/0") {
        let msg = choice.get("message").or_else(|| choice.get("delta"));
        if let Some(msg) = msg {
            let thought_field = msg
                .get("reasoning_content")
                .or_else(|| msg.get("reasoning"))
                .or_else(|| msg.get("thought"))
                .or_else(|| msg.get("thinking"))
                .or_else(|| msg.get("reasoning_text"))
                .and_then(Value::as_str);
            if let Some(t) = thought_field {
                let trimmed = t.trim();
                if !trimmed.is_empty() {
                    explicit_thoughts.push(trimmed.to_string());
                }
            }
        }
    }

    // Extract from Anthropic content blocks
    if let Some(content_blocks) = response.get("content").and_then(Value::as_array) {
        for block in content_blocks {
            if block.get("type").and_then(Value::as_str) == Some("thinking")
                && let Some(t) = block.get("thinking").and_then(Value::as_str)
            {
                let trimmed = t.trim();
                if !trimmed.is_empty() {
                    explicit_thoughts.push(trimmed.to_string());
                }
            }
        }
    }

    // Extract from Gemini candidates[0].content.parts
    if let Some(parts) = response
        .pointer("/candidates/0/content/parts")
        .and_then(Value::as_array)
    {
        for part in parts {
            if part.get("thought").and_then(Value::as_bool) == Some(true)
                && let Some(t) = part.get("text").and_then(Value::as_str)
            {
                let trimmed = t.trim();
                if !trimmed.is_empty() {
                    explicit_thoughts.push(trimmed.to_string());
                }
            }
        }
    }

    let mut raw_parts = Vec::new();
    if let Some(output_text) = response.get("output_text").and_then(Value::as_str)
        && !output_text.is_empty()
    {
        raw_parts.push(output_text.to_string());
    }

    if raw_parts.is_empty()
        && let Some(choices) = response.get("choices")
    {
        extract_text_parts(choices, &mut raw_parts);
    }

    if raw_parts.is_empty()
        && let Some(output) = response.get("output")
    {
        extract_text_parts(output, &mut raw_parts);
    }

    if raw_parts.is_empty() {
        extract_text_parts(response, &mut raw_parts);
    }

    let joined_raw = raw_parts
        .into_iter()
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    let (cleaned_text, inline_thought) = strip_and_extract_inline_thoughts(&joined_raw);
    let cleaned_text = cleaned_text.trim().to_string();

    if let Some(it) = inline_thought {
        explicit_thoughts.push(it);
    }

    let combined_thought = if explicit_thoughts.is_empty() {
        None
    } else {
        Some(explicit_thoughts.join("\n\n"))
    };

    if cleaned_text.is_empty() {
        return Err(LlmPortError::new(
            LlmPortErrorKind::Protocol,
            "LLM response did not contain text output",
        ));
    }

    Ok((cleaned_text, combined_thought))
}

pub fn extract_text_from_json_response(response: &Value) -> Result<String, LlmPortError> {
    extract_text_and_thought_from_json_response(response).map(|(text, _)| text)
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
        prompt_tokens,
        completion_tokens,
        total_tokens: normalized_total,
        ..TokenUsage::default()
    })
}

pub fn extract_anthropic_text_and_thought_response(
    response: &Value,
) -> Result<(String, Option<String>, Option<TokenUsage>), LlmPortError> {
    let content = response
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            LlmPortError::new(
                LlmPortErrorKind::Protocol,
                "Anthropic response missing content array",
            )
        })?;

    let text_parts: Vec<&str> = content
        .iter()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|block| block.get("text").and_then(Value::as_str))
        .collect();

    let thought_parts: Vec<&str> = content
        .iter()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("thinking"))
        .filter_map(|block| block.get("thinking").and_then(Value::as_str))
        .collect();

    if text_parts.is_empty() {
        return Err(LlmPortError::new(
            LlmPortErrorKind::Protocol,
            "Anthropic response did not contain text output",
        ));
    }

    let thought = if thought_parts.is_empty() {
        None
    } else {
        Some(thought_parts.join("\n\n"))
    };

    let usage = response.get("usage").and_then(|u| {
        let input_tokens = u.get("input_tokens").and_then(Value::as_u64).unwrap_or(0);
        let output_tokens = u.get("output_tokens").and_then(Value::as_u64).unwrap_or(0);
        let cached_input_tokens = u
            .get("cache_read_input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let cache_creation_input_tokens = u
            .get("cache_creation_input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let total_tokens = input_tokens
            .saturating_add(output_tokens)
            .saturating_add(cached_input_tokens)
            .saturating_add(cache_creation_input_tokens);
        let mut usage = normalize_token_usage(input_tokens, output_tokens, total_tokens)?;
        usage.cached_input_tokens = cached_input_tokens;
        usage.cache_creation_input_tokens = cache_creation_input_tokens;
        Some(usage)
    });

    Ok((text_parts.join("\n"), thought, usage))
}

pub fn extract_anthropic_text_response(
    response: &Value,
) -> Result<(String, Option<TokenUsage>), LlmPortError> {
    extract_anthropic_text_and_thought_response(response).map(|(text, _, usage)| (text, usage))
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

    let mut normalized = normalize_token_usage(prompt_tokens, completion_tokens, total_tokens)?;
    normalized.cached_input_tokens = usage
        .pointer("/prompt_tokens_details/cached_tokens")
        .or_else(|| usage.pointer("/input_tokens_details/cached_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    normalized.cache_creation_input_tokens = usage
        .get("cache_creation_input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    normalized.reasoning_tokens = usage
        .pointer("/completion_tokens_details/reasoning_tokens")
        .or_else(|| usage.pointer("/output_tokens_details/reasoning_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    Some(normalized)
}

pub fn build_standard_input(req: &StandardLlmRequest) -> String {
    req.messages
        .iter()
        .filter(|message| matches!(message.role, MessageRole::User))
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}
