use std::sync::Arc;

use aimux_core::error::AiMuxError;
use aimux_core::language_model::LanguageModel;
use aimux_core::language_model_message::{LanguageModelMessage, LanguageModelPrompt};
use aimux_core::options::{CallOptions, ResponseFormat};
use aimux_core::provider::Provider;
use aimux_core::result::GenerateContent;
use aimux_core::shared::SharedProviderOptions;
use aimux_core::stream_part::StreamPart;
use aimux_core::types::ReasoningEffort;
use futures_util::StreamExt;
use serde_json::json;
use sona_core::llm::demuxer::ThoughtStreamDemuxer;
use sona_core::llm::provider_protocol::{
    StandardLlmResponse, normalize_token_usage, strip_and_extract_inline_thoughts,
};
use sona_core::llm::requests::LlmConfig;
use sona_core::llm::runtime::{
    LlmCompletionRequest, LlmResponseFormat, LlmStreamDelta, LlmStreamDeltaKind, ThinkingLevel,
};
use sona_core::llm::streaming_protocol::DualStreamAccumulator;
use sona_core::llm::tasks::LlmProviderStrategy;
use sona_core::ports::llm::{LlmPortError, LlmPortErrorKind};

pub fn map_aimux_error(error: AiMuxError) -> LlmPortError {
    let msg = error.to_string();
    let lower = msg.to_ascii_lowercase();

    let status_code = error.status_code().or_else(|| {
        if let Some(idx) = msg.find("HTTP ") {
            let rest = &msg[idx + 5..];
            rest.split(|c: char| !c.is_ascii_digit())
                .next()
                .and_then(|code_str| code_str.parse::<u16>().ok())
        } else {
            None
        }
    });

    let retry_after_ms = error
        .retry_after_hint()
        .map(|ms| ms.max(0) as u64)
        .or_else(|| {
            if let Some(idx) = lower.find("retry-after") {
                let rest = &lower[idx + 11..];
                rest.split(|c: char| !c.is_ascii_digit())
                    .find(|s| !s.is_empty())
                    .and_then(|s| s.parse::<u64>().ok())
                    .map(|sec| sec * 1000)
            } else {
                None
            }
        });
    let kind = match status_code {
        Some(401 | 403) => LlmPortErrorKind::Authentication,
        Some(429) => LlmPortErrorKind::RateLimited,
        Some(408) => LlmPortErrorKind::Timeout,
        Some(400 | 422) => LlmPortErrorKind::InvalidRequest,
        Some(500..=599) => LlmPortErrorKind::Unavailable,
        _ => {
            if lower.contains("401") || lower.contains("unauthorized") || lower.contains("api key")
            {
                LlmPortErrorKind::Authentication
            } else if lower.contains("429") || lower.contains("rate limit") {
                LlmPortErrorKind::RateLimited
            } else if lower.contains("timeout") || lower.contains("timed out") {
                LlmPortErrorKind::Timeout
            } else if lower.contains("400")
                || lower.contains("bad request")
                || lower.contains("invalid")
            {
                LlmPortErrorKind::InvalidRequest
            } else if lower.contains("529")
                || lower.contains("overloaded")
                || lower.contains("unavailable")
            {
                LlmPortErrorKind::Unavailable
            } else {
                LlmPortErrorKind::Network
            }
        }
    };

    let mut port_error = LlmPortError::new(kind, msg);
    port_error.retry_after_ms = retry_after_ms;
    port_error
}

pub fn normalize_openai_base_url(
    strategy: LlmProviderStrategy,
    base_url: &str,
    api_path: Option<&str>,
) -> String {
    let mut trimmed = base_url.trim_end_matches('/');
    if matches!(
        strategy,
        LlmProviderStrategy::OpenAi | LlmProviderStrategy::OpenAiResponses
    ) && trimmed.ends_with("/responses")
    {
        trimmed = trimmed.trim_end_matches("/responses").trim_end_matches('/');
    }
    if let Some(path) = api_path {
        let p = path.trim_start_matches('/');
        if p.ends_with("chat/completions") {
            let base_part = p.trim_end_matches("chat/completions").trim_end_matches('/');
            if base_part.is_empty() || trimmed.ends_with(base_part) {
                trimmed.to_string()
            } else {
                format!("{trimmed}/{base_part}")
            }
        } else if matches!(
            strategy,
            LlmProviderStrategy::OpenAi | LlmProviderStrategy::OpenAiResponses
        ) && p.ends_with("responses")
        {
            let base_part = p.trim_end_matches("responses").trim_end_matches('/');
            if base_part.is_empty() || trimmed.ends_with(base_part) {
                trimmed.to_string()
            } else {
                format!("{trimmed}/{base_part}")
            }
        } else {
            format!("{trimmed}/{p}")
        }
    } else if matches!(
        strategy,
        LlmProviderStrategy::DeepSeek | LlmProviderStrategy::Groq
    ) {
        trimmed.to_string()
    } else if !trimmed.ends_with("/v1")
        && !trimmed.contains("/v1")
        && !trimmed.contains("/v2")
        && !trimmed.contains("/v3")
        && !trimmed.contains("/v4")
    {
        format!("{trimmed}/v1")
    } else {
        trimmed.to_string()
    }
}

pub fn create_aimux_provider(
    strategy: LlmProviderStrategy,
    api_key: &str,
    base_url: &str,
    api_path: Option<&str>,
) -> Result<Arc<dyn Provider>, LlmPortError> {
    create_aimux_provider_with_version(strategy, api_key, base_url, api_path, None)
}

pub fn create_aimux_provider_with_version(
    strategy: LlmProviderStrategy,
    api_key: &str,
    base_url: &str,
    api_path: Option<&str>,
    api_version: Option<&str>,
) -> Result<Arc<dyn Provider>, LlmPortError> {
    let base_url = base_url.trim();

    match strategy {
        LlmProviderStrategy::AzureOpenAi => {
            let mut azure_settings = aimux_providers::AzureOpenAIProviderSettings::default();
            if !api_key.is_empty() {
                azure_settings.api_key = Some(api_key.to_string().into());
            }
            if let Some(version) = api_version {
                azure_settings.api_version = Some(version.to_string());
            }
            if !base_url.is_empty() {
                let trimmed = base_url.trim_end_matches('/');
                let effective_base =
                    if trimmed.contains(".openai.azure.com") && !trimmed.ends_with("/openai") {
                        format!("{trimmed}/openai")
                    } else {
                        trimmed.to_string()
                    };
                azure_settings.base_url = Some(effective_base);
            }
            let provider =
                aimux_providers::create_azure(azure_settings).map_err(map_aimux_error)?;
            Ok(Arc::new(provider))
        }
        LlmProviderStrategy::Anthropic => {
            let mut anthropic_settings = aimux_providers::AnthropicProviderSettings::default();
            if !api_key.is_empty() {
                anthropic_settings.api_key = Some(api_key.to_string());
            }
            if !base_url.is_empty() {
                anthropic_settings.base_url = Some(base_url.to_string());
            }
            let provider =
                aimux_providers::create_anthropic(anthropic_settings).map_err(map_aimux_error)?;
            Ok(Arc::new(provider))
        }
        LlmProviderStrategy::Gemini => {
            let mut google_settings = aimux_providers::GoogleProviderSettings::default();
            if !api_key.is_empty() {
                google_settings.api_key = Some(api_key.to_string());
            }
            if !base_url.is_empty() {
                google_settings.base_url = Some(base_url.to_string());
            }
            let provider =
                aimux_providers::create_google(google_settings).map_err(map_aimux_error)?;
            Ok(Arc::new(provider))
        }
        LlmProviderStrategy::Cohere => {
            let effective_base = if base_url.is_empty() {
                "https://api.cohere.com/v2".to_string()
            } else {
                let trimmed = base_url.trim_end_matches('/');
                if !trimmed.ends_with("/v2") {
                    format!("{trimmed}/v2")
                } else {
                    trimmed.to_string()
                }
            };
            let mut cohere_settings = aimux_providers::CohereProviderSettings::default();
            if !api_key.is_empty() {
                cohere_settings.api_key = Some(api_key.to_string());
            }
            cohere_settings.base_url = Some(effective_base);
            let provider =
                aimux_providers::create_cohere(cohere_settings).map_err(map_aimux_error)?;
            Ok(Arc::new(provider))
        }
        LlmProviderStrategy::XAi => {
            let mut xai_settings = aimux_providers::XAIProviderSettings::default();
            if !api_key.is_empty() {
                xai_settings.api_key = Some(api_key.to_string());
            }
            if !base_url.is_empty() {
                let effective_base = normalize_openai_base_url(strategy, base_url, api_path);
                xai_settings.base_url = Some(effective_base);
            }
            let provider = aimux_providers::create_xai(xai_settings).map_err(map_aimux_error)?;
            Ok(Arc::new(provider))
        }
        LlmProviderStrategy::MistralAi => {
            let mut mistral_settings = aimux_providers::MistralProviderSettings::default();
            if !api_key.is_empty() {
                mistral_settings.api_key = Some(api_key.to_string());
            }
            if !base_url.is_empty() {
                let effective_base = normalize_openai_base_url(strategy, base_url, api_path);
                mistral_settings.base_url = Some(effective_base);
            }
            let provider =
                aimux_providers::create_mistral(mistral_settings).map_err(map_aimux_error)?;
            Ok(Arc::new(provider))
        }
        LlmProviderStrategy::OpenRouter => {
            let effective_base = if !base_url.is_empty() {
                normalize_openai_base_url(strategy, base_url, api_path)
            } else {
                "https://openrouter.ai/api/v1".to_string()
            };
            let settings = aimux_providers::PresetSettings {
                api_key: if !api_key.is_empty() {
                    Some(api_key.to_string().into())
                } else {
                    None
                },
                base_url: Some(effective_base),
                ..Default::default()
            };
            aimux_providers::create_provider("openrouter", settings).map_err(map_aimux_error)
        }
        LlmProviderStrategy::Ollama => {
            let effective_base = if !base_url.is_empty() {
                Some(normalize_openai_base_url(strategy, base_url, api_path))
            } else {
                None
            };
            let key = if api_key.is_empty() {
                "ollama"
            } else {
                api_key
            };
            let settings = aimux_providers::PresetSettings {
                api_key: Some(key.to_string().into()),
                base_url: effective_base,
                ..Default::default()
            };
            aimux_providers::create_provider("ollama", settings).map_err(map_aimux_error)
        }
        LlmProviderStrategy::LmStudio => {
            let effective_base = if !base_url.is_empty() {
                normalize_openai_base_url(strategy, base_url, api_path)
            } else {
                "http://localhost:1234/v1".to_string()
            };
            let key = if api_key.is_empty() {
                "lmstudio"
            } else {
                api_key
            };
            let settings = aimux_providers::PresetSettings {
                api_key: Some(key.to_string().into()),
                base_url: Some(effective_base),
                ..Default::default()
            };
            aimux_providers::create_provider("lmstudio", settings).map_err(map_aimux_error)
        }
        LlmProviderStrategy::Llamafile => {
            let effective_base = if !base_url.is_empty() {
                normalize_openai_base_url(strategy, base_url, api_path)
            } else {
                "http://localhost:8080/v1".to_string()
            };
            let key = if api_key.is_empty() {
                None
            } else {
                Some(api_key.to_string())
            };
            let settings = aimux_providers::openai_compatible::OpenAICompatibleProviderSettings {
                name: "llamafile".to_string(),
                base_url: effective_base,
                api_key: key,
                ..Default::default()
            };
            let provider = aimux_providers::openai_compatible::create_openai_compatible(settings)
                .map_err(map_aimux_error)?;
            Ok(Arc::new(provider))
        }
        _ => {
            let registry_name = match strategy {
                LlmProviderStrategy::DeepSeek => Some("deepseek"),
                LlmProviderStrategy::Groq => Some("groq"),
                LlmProviderStrategy::MoonshotAi => Some("moonshotai"),
                LlmProviderStrategy::MoonshotCn => Some("moonshotai"),
                LlmProviderStrategy::Kimi => Some("kimi"),
                LlmProviderStrategy::Xiaomi => Some("xiaomimimo"),
                LlmProviderStrategy::SiliconFlow => Some("siliconflow"),
                LlmProviderStrategy::Qwen | LlmProviderStrategy::QwenPortal => Some("alibaba"),
                LlmProviderStrategy::MinimaxGlobal => Some("minimax"),
                LlmProviderStrategy::MinimaxCn => Some("minimax_cn"),
                LlmProviderStrategy::Together => Some("togetherai"),
                LlmProviderStrategy::Venice => Some("venice"),
                LlmProviderStrategy::Hyperbolic => Some("hyperbolic"),
                LlmProviderStrategy::Perplexity => Some("perplexity"),
                LlmProviderStrategy::Copilot => Some("copilot"),
                LlmProviderStrategy::Chatglm => Some("bigmodel"),
                LlmProviderStrategy::Volcengine => Some("volc_engine"),
                LlmProviderStrategy::Baidu => Some("baidu"),
                LlmProviderStrategy::Tencent => Some("tencent"),
                LlmProviderStrategy::Stepfun => Some("stepfun"),
                LlmProviderStrategy::Lingyiwanwu => Some("lingyiwanwu"),
                LlmProviderStrategy::Cerebras => Some("cerebras"),
                _ => None,
            };

            let effective_base = if base_url.is_empty() {
                None
            } else {
                Some(normalize_openai_base_url(strategy, base_url, api_path))
            };

            if let Some(name) = registry_name {
                let settings = aimux_providers::PresetSettings {
                    api_key: if !api_key.is_empty() {
                        Some(api_key.to_string().into())
                    } else {
                        None
                    },
                    base_url: effective_base.clone(),
                    ..Default::default()
                };
                if let Ok(handle) = aimux_providers::create_provider(name, settings) {
                    return Ok(handle);
                }
            }

            let fallback_base =
                effective_base.unwrap_or_else(|| "https://api.openai.com/v1".to_string());
            let openai_settings = aimux_providers::OpenAIProviderSettings {
                api_key: if !api_key.is_empty() {
                    Some(api_key.to_string())
                } else {
                    None
                },
                base_url: Some(fallback_base),
                name: Some("openai".to_string()),
                ..Default::default()
            };
            let provider =
                aimux_providers::create_openai(openai_settings).map_err(map_aimux_error)?;
            Ok(Arc::new(provider))
        }
    }
}

pub fn create_aimux_model(config: &LlmConfig) -> Result<Arc<dyn LanguageModel>, LlmPortError> {
    let base_url = config.base_url.trim();
    let api_key = &config.api_key;
    let model = &config.model;

    let provider = create_aimux_provider_with_version(
        config.strategy,
        api_key,
        base_url,
        config.api_path.as_deref(),
        config.api_version.as_deref(),
    )?;
    provider.language_model(model).map_err(map_aimux_error)
}

pub fn build_aimux_call_options(
    request: &LlmCompletionRequest,
) -> Result<CallOptions, LlmPortError> {
    let mut prompt: LanguageModelPrompt = Vec::new();

    if let Some(sys) = &request.system_prompt {
        prompt.push(LanguageModelMessage::System {
            content: sys.clone(),
            provider_options: None,
        });
    }

    prompt.push(LanguageModelMessage::user_text(&request.input));

    let mut options = CallOptions::new(prompt);
    options.temperature = if request.capabilities().supports_temperature {
        request.options.temperature.map(f64::from)
    } else {
        None
    };
    options.max_output_tokens = request
        .options
        .max_output_tokens
        .map(|v| v.min(u64::from(u32::MAX)) as u32);

    match &request.options.response_format {
        LlmResponseFormat::Text => {
            options.response_format = Some(ResponseFormat::Text);
        }
        LlmResponseFormat::JsonObject => {
            options.response_format = Some(ResponseFormat::Json {
                schema: None,
                name: None,
                description: None,
            });
        }
        LlmResponseFormat::JsonSchema { name, schema } => {
            options.response_format = Some(ResponseFormat::Json {
                schema: Some(schema.clone()),
                name: Some(name.clone()),
                description: None,
            });
        }
    }

    let mut provider_options = SharedProviderOptions::new();

    if request.effective_reasoning_enabled() {
        let level = request.effective_reasoning_level();
        let thinking_level = ThinkingLevel::from_legacy_options(Some(true), level);
        match thinking_level {
            ThinkingLevel::None => {
                options.reasoning = Some(ReasoningEffort::None);
            }
            ThinkingLevel::Minimal => {
                options.reasoning = Some(ReasoningEffort::Minimal);
            }
            ThinkingLevel::Low => {
                options.reasoning = Some(ReasoningEffort::Low);
            }
            ThinkingLevel::Medium | ThinkingLevel::Auto => {
                options.reasoning = Some(ReasoningEffort::Medium);
            }
            ThinkingLevel::High => {
                options.reasoning = Some(ReasoningEffort::High);
            }
            ThinkingLevel::Xhigh | ThinkingLevel::Max => {
                options.reasoning = Some(ReasoningEffort::Xhigh);
            }
            ThinkingLevel::Budget(budget) => {
                options.reasoning = Some(ReasoningEffort::Medium);
                let mut anthropic_map = serde_json::Map::new();
                anthropic_map.insert(
                    "thinking".to_string(),
                    json!({
                        "type": "enabled",
                        "budget_tokens": budget
                    }),
                );
                provider_options.insert("anthropic".to_string(), anthropic_map);
            }
        }
    } else {
        options.reasoning = Some(ReasoningEffort::None);
        if request.config.strategy == LlmProviderStrategy::Gemini {
            let mut google_map = serde_json::Map::new();
            google_map.insert(
                "thinkingConfig".to_string(),
                json!({
                    "thinkingBudget": 0
                }),
            );
            provider_options.insert("google".to_string(), google_map);
        }
    }

    if !provider_options.is_empty() {
        options.provider_options = Some(provider_options);
    }

    Ok(options)
}

pub async fn execute_aimux_completion(
    request: &LlmCompletionRequest,
) -> Result<StandardLlmResponse, LlmPortError> {
    let model = create_aimux_model(&request.config)?;
    let options = build_aimux_call_options(request)?;

    let result = model.do_generate(&options).await.map_err(map_aimux_error)?;

    let mut text_acc = String::new();
    let mut thought_acc = String::new();

    for item in result.content {
        match item {
            GenerateContent::Text { text, .. } => {
                text_acc.push_str(&text);
            }
            GenerateContent::Reasoning(reasoning) => {
                thought_acc.push_str(&reasoning.text);
            }
            _ => {}
        }
    }

    let (clean_text, inline_thought) = strip_and_extract_inline_thoughts(&text_acc);
    if let Some(inline) = inline_thought
        && !inline.is_empty()
    {
        if !thought_acc.is_empty() {
            thought_acc.push('\n');
        }
        thought_acc.push_str(&inline);
    }

    let thought = if thought_acc.is_empty() {
        None
    } else {
        Some(thought_acc)
    };

    let input_tokens = result.usage.input_tokens.total.unwrap_or(0) as u64;
    let output_tokens = result.usage.output_tokens.total.unwrap_or(0) as u64;
    let total_tokens = input_tokens.saturating_add(output_tokens);

    let usage = normalize_token_usage(input_tokens, output_tokens, total_tokens).map(|mut u| {
        u.cached_input_tokens = result.usage.input_tokens.cache_read.unwrap_or(0) as u64;
        u.cache_creation_input_tokens = result.usage.input_tokens.cache_write.unwrap_or(0) as u64;
        u.reasoning_tokens = result.usage.output_tokens.reasoning.unwrap_or(0) as u64;
        u
    });

    Ok(StandardLlmResponse {
        text: clean_text,
        thought,
        usage,
    })
}

pub async fn execute_aimux_stream<EmitFn>(
    request: &LlmCompletionRequest,
    dual: &mut DualStreamAccumulator<'_, EmitFn, LlmPortError>,
) -> Result<StandardLlmResponse, LlmPortError>
where
    EmitFn: FnMut(LlmStreamDelta) -> Result<(), LlmPortError> + Send + ?Sized,
{
    let model = create_aimux_model(&request.config)?;
    let options = build_aimux_call_options(request)?;

    let stream_result = model.do_stream(&options).await.map_err(map_aimux_error)?;

    let mut stream = stream_result.stream;
    let mut last_usage = None;
    let mut demuxer = ThoughtStreamDemuxer::new();

    while let Some(part_result) = stream.next().await {
        let part = part_result.map_err(map_aimux_error)?;
        match part {
            StreamPart::ReasoningDelta { delta, .. } => {
                dual.push_thought(&delta)?;
            }
            StreamPart::TextDelta { delta, .. } => {
                for item in demuxer.process(&delta) {
                    match item.kind {
                        LlmStreamDeltaKind::Content => {
                            dual.push_content(&item.text)?;
                        }
                        LlmStreamDeltaKind::Thought => {
                            dual.push_thought(&item.text)?;
                        }
                    }
                }
            }
            StreamPart::Finish { usage, .. } => {
                let input_tokens = usage.input_tokens.total.unwrap_or(0) as u64;
                let output_tokens = usage.output_tokens.total.unwrap_or(0) as u64;
                let total_tokens = input_tokens.saturating_add(output_tokens);
                last_usage = normalize_token_usage(input_tokens, output_tokens, total_tokens).map(
                    |mut u| {
                        u.cached_input_tokens = usage.input_tokens.cache_read.unwrap_or(0) as u64;
                        u.cache_creation_input_tokens =
                            usage.input_tokens.cache_write.unwrap_or(0) as u64;
                        u.reasoning_tokens = usage.output_tokens.reasoning.unwrap_or(0) as u64;
                        u
                    },
                );
            }
            StreamPart::Error { error } => {
                return Err(map_aimux_error(error));
            }
            _ => {}
        }
    }

    for item in demuxer.flush() {
        match item.kind {
            LlmStreamDeltaKind::Content => {
                dual.push_content(&item.text)?;
            }
            LlmStreamDeltaKind::Thought => {
                dual.push_thought(&item.text)?;
            }
        }
    }

    let thought = if dual.thought_text().is_empty() {
        None
    } else {
        Some(dual.thought_text().to_string())
    };
    Ok(StandardLlmResponse {
        text: dual.content_text().to_string(),
        thought,
        usage: last_usage,
    })
}
