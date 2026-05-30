use super::network::{LlmApiUrl, post_json_request};
use super::*;
use async_trait::async_trait;
use log::warn;
use reqwest::Client;
use rig::client::{CompletionClient, Nothing};
use rig::completion::CompletionModel;
use rig::providers::{anthropic, gemini, ollama, openai};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter};

#[derive(Serialize)]
pub(crate) struct GoogleTranslateRequest {
    pub(crate) q: Vec<String>,
    pub(crate) target: String,
    pub(crate) format: String,
}

#[derive(Deserialize)]
pub(crate) struct GoogleTranslateTranslation {
    #[serde(rename = "translatedText")]
    pub(crate) translated_text: String,
}

#[derive(Deserialize)]
pub(crate) struct GoogleTranslateData {
    pub(crate) translations: Vec<GoogleTranslateTranslation>,
}

#[derive(Deserialize)]
pub(crate) struct GoogleTranslateResponse {
    pub(crate) data: GoogleTranslateData,
}

#[async_trait]
pub trait LlmAdapter: Send + Sync {
    async fn generate(
        &self,
        client: &Client,
        req: &StandardLlmRequest,
        config: &LlmConfig,
    ) -> Result<StandardLlmResponse, String>;
}

pub struct OpenAiAdapter;

fn strategy_uses_openai_chat_payload(strategy: LlmProviderStrategy) -> bool {
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

#[async_trait]
impl LlmAdapter for OpenAiAdapter {
    async fn generate(
        &self,
        _client: &Client,
        req: &StandardLlmRequest,
        config: &LlmConfig,
    ) -> Result<StandardLlmResponse, String> {
        let client = openai::Client::builder()
            .api_key(&config.api_key)
            .base_url(&config.base_url)
            .build()
            .map_err(|error| error.to_string())?;

        // For now, we use the first message's content as input to match current behavior
        // or join all user messages.
        let input = req
            .messages
            .iter()
            .filter(|m| matches!(m.role, MessageRole::User))
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        if config.reasoning_enabled.unwrap_or(false)
            && strategy_uses_openai_chat_payload(config.strategy)
        {
            let base_url = LlmApiUrl::parse(&config.base_url)?;
            let url =
                base_url.join(config.api_path.as_deref().unwrap_or("/v1/chat/completions"))?;
            return generate_with_openai_chat_api(&url, config, &input, vec![]).await;
        }

        let response = client
            .completion_model(&config.model)
            .completion_request(&input)
            .temperature_opt(Some(req.temperature as f64))
            .send()
            .await
            .map_err(|error| error.to_string())?;

        Ok(StandardLlmResponse {
            text: extract_text_response(&response.choice)?,
            usage: token_usage_from_rig_usage(Some(response.usage)),
        })
    }
}

pub struct AnthropicAdapter;

#[async_trait]
impl LlmAdapter for AnthropicAdapter {
    async fn generate(
        &self,
        _client: &Client,
        req: &StandardLlmRequest,
        config: &LlmConfig,
    ) -> Result<StandardLlmResponse, String> {
        let input = req
            .messages
            .iter()
            .filter(|m| matches!(m.role, MessageRole::User))
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        if config.reasoning_enabled.unwrap_or(false) {
            let url = LlmApiUrl::parse(&join_url(&config.base_url, "/v1/messages"))?;
            let budget_tokens = match config.reasoning_level.as_deref() {
                Some("low") => 1024,
                Some("high") => 4096,
                _ => 2048, // medium
            };

            let payload = json!({
                "model": config.model,
                "messages": [
                    {
                        "role": "user",
                        "content": input,
                    }
                ],
                "max_tokens": 8192,
                "thinking": {
                    "type": "enabled",
                    "budget_tokens": budget_tokens,
                },
                "temperature": 1.0,
            });

            let response = post_json_request(
                &url,
                vec![
                    ("x-api-key", config.api_key.clone()),
                    ("anthropic-version", "2023-06-01".to_string()),
                ],
                payload,
            )
            .await?;

            let text = response
                .pointer("/content/0/text")
                .and_then(Value::as_str)
                .ok_or_else(|| "Anthropic response did not contain text output".to_string())?
                .to_string();

            return Ok(StandardLlmResponse { text, usage: None });
        }

        let client = anthropic::Client::builder()
            .api_key(&config.api_key)
            .base_url(&config.base_url)
            .build()
            .map_err(|error| error.to_string())?;

        let response = client
            .completion_model(&config.model)
            .completion_request(&input)
            .temperature_opt(Some(req.temperature as f64))
            .send()
            .await
            .map_err(|error| error.to_string())?;

        Ok(StandardLlmResponse {
            text: extract_text_response(&response.choice)?,
            usage: token_usage_from_rig_usage(Some(response.usage)),
        })
    }
}

pub struct OllamaAdapter;

#[async_trait]
impl LlmAdapter for OllamaAdapter {
    async fn generate(
        &self,
        _client: &Client,
        req: &StandardLlmRequest,
        config: &LlmConfig,
    ) -> Result<StandardLlmResponse, String> {
        let client = ollama::Client::builder()
            .api_key(Nothing)
            .base_url(config.base_url.trim_end_matches("/v1"))
            .build()
            .map_err(|error| error.to_string())?;

        let input = req
            .messages
            .iter()
            .filter(|m| matches!(m.role, MessageRole::User))
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        let response = client
            .completion_model(&config.model)
            .completion_request(&input)
            .temperature_opt(Some(req.temperature as f64))
            .send()
            .await
            .map_err(|error| error.to_string())?;

        Ok(StandardLlmResponse {
            text: extract_text_response(&response.choice)?,
            usage: token_usage_from_rig_usage(Some(response.usage)),
        })
    }
}

pub struct GeminiAdapter;

#[async_trait]
impl LlmAdapter for GeminiAdapter {
    async fn generate(
        &self,
        _client: &Client,
        req: &StandardLlmRequest,
        config: &LlmConfig,
    ) -> Result<StandardLlmResponse, String> {
        let input = req
            .messages
            .iter()
            .filter(|m| matches!(m.role, MessageRole::User))
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        if config.reasoning_enabled.unwrap_or(false) {
            let cleaned_base = clean_gemini_base_url(&config.base_url);
            let is_gemini_2_5 = config.model.contains("gemini-2.5");

            let url_str = format!(
                "{}/v1beta/models/{}:generateContent?key={}",
                cleaned_base, config.model, config.api_key
            );
            let url = LlmApiUrl::parse(&url_str)?;

            let budget_tokens = match config.reasoning_level.as_deref() {
                Some("low") => 1024,
                Some("high") => 4096,
                _ => 2048, // medium
            };

            let thinking_level = match config.reasoning_level.as_deref() {
                Some("low") => "LOW",
                Some("high") => "HIGH",
                _ => "MEDIUM",
            };

            let thinking_config = if is_gemini_2_5 {
                json!({
                    "thinkingBudget": budget_tokens,
                    "includeThoughts": true
                })
            } else {
                json!({
                    "thinkingLevel": thinking_level,
                    "includeThoughts": true
                })
            };

            let payload = json!({
                "contents": [{
                    "parts": [{"text": input}]
                }],
                "generationConfig": {
                    "temperature": req.temperature,
                    "thinkingConfig": thinking_config
                }
            });

            let response = post_json_request(&url, vec![], payload).await?;

            let text = response
                .pointer("/candidates/0/content/parts/0/text")
                .and_then(Value::as_str)
                .ok_or_else(|| "Gemini response did not contain text output".to_string())?
                .to_string();

            let usage = response.get("usageMetadata").map(|u| TokenUsage {
                prompt_tokens: u
                    .get("promptTokenCount")
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as u32,
                completion_tokens: u
                    .get("candidatesTokenCount")
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as u32,
                total_tokens: u
                    .get("totalTokenCount")
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as u32,
            });

            return Ok(StandardLlmResponse { text, usage });
        }

        let client = gemini::Client::builder()
            .api_key(&config.api_key)
            .base_url(clean_gemini_base_url(&config.base_url))
            .build()
            .map_err(|error| error.to_string())?;

        let response = client
            .completion_model(&config.model)
            .completion_request(&input)
            .temperature_opt(Some(req.temperature as f64))
            .send()
            .await
            .map_err(|error| error.to_string())?;

        Ok(StandardLlmResponse {
            text: extract_text_response(&response.choice)?,
            usage: token_usage_from_rig_usage(Some(response.usage)),
        })
    }
}

pub struct GoogleTranslateAdapter;

#[async_trait]
impl LlmAdapter for GoogleTranslateAdapter {
    async fn generate(
        &self,
        client: &Client,
        req: &StandardLlmRequest,
        config: &LlmConfig,
    ) -> Result<StandardLlmResponse, String> {
        let input = req
            .messages
            .iter()
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let base_url = LlmApiUrl::parse(&config.base_url)?;

        if config.strategy == LlmProviderStrategy::GoogleTranslateFree {
            let fetch_client = client.clone();

            let (_, text) = execute_google_translate_free_request(
                0,
                input,
                "en".to_string(),
                move |text, target| {
                    let client = fetch_client.clone();
                    let base_url = base_url.clone();
                    async move {
                        fetch_google_translate_free_translation(&client, &base_url, &target, &text)
                            .await
                    }
                },
                tokio::time::sleep,
            )
            .await?;

            return Ok(StandardLlmResponse { text, usage: None });
        }

        let payload = GoogleTranslateRequest {
            q: vec![input],
            target: "en".to_string(), // Default fallback
            format: "text".to_string(),
        };

        let url = base_url;

        let response = post_json_request(
            &url,
            vec![("x-goog-api-key", config.api_key.clone())],
            json!(payload),
        )
        .await?;
        let text = extract_text_from_json_response(&response)?;

        Ok(StandardLlmResponse { text, usage: None })
    }
}
pub struct GenericHttpAdapter;

#[async_trait]
impl LlmAdapter for GenericHttpAdapter {
    async fn generate(
        &self,
        _client: &Client,
        req: &StandardLlmRequest,
        config: &LlmConfig,
    ) -> Result<StandardLlmResponse, String> {
        let input = req
            .messages
            .iter()
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        let response = match config.strategy {
            LlmProviderStrategy::OpenAiResponses => {
                generate_with_openai_responses_api(
                    &config.base_url,
                    &config.api_key,
                    &config.model,
                    &input,
                    Some(req.temperature),
                    config.api_path.as_deref(),
                )
                .await?
            }
            LlmProviderStrategy::AzureOpenAi => generate_with_azure_openai(config, &input).await?,
            LlmProviderStrategy::Perplexity => generate_with_perplexity(config, &input).await?,
            _ => generate_with_openai_custom_path(config, &input).await?,
        };

        Ok(response)
    }
}

pub(crate) struct AdapterFactory;

impl AdapterFactory {
    pub(crate) fn create(strategy: LlmProviderStrategy) -> Box<dyn LlmAdapter> {
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
            _ => Box::new(GenericHttpAdapter),
        }
    }
}

#[derive(Deserialize)]
pub(crate) struct OpenAiModel {
    pub(crate) id: String,
}

#[derive(Deserialize)]
struct OpenAiModelsResponse {
    data: Vec<OpenAiModel>,
}

#[derive(Deserialize)]
struct OllamaModel {
    name: String,
}

#[derive(Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModel>,
}

#[derive(Deserialize)]
pub(crate) struct GeminiModel {
    pub(crate) name: String,
    #[serde(rename = "supportedGenerationMethods")]
    pub(crate) supported_generation_methods: Option<Vec<String>>,
    #[serde(rename = "inputTokenLimit")]
    pub(crate) input_token_limit: Option<u64>,
    #[serde(rename = "outputTokenLimit")]
    pub(crate) output_token_limit: Option<u64>,
}

#[derive(Deserialize)]
struct GeminiModelsResponse {
    models: Option<Vec<GeminiModel>>,
}

pub(crate) fn clean_gemini_base_url(base_url: &str) -> &str {
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

pub(crate) fn format_gemini_models_url(base_url: &str) -> String {
    let cleaned_base = clean_gemini_base_url(base_url);

    format!("{}/v1beta/models", cleaned_base)
}

pub(crate) fn build_gemini_models_url(base_url: &LlmApiUrl) -> Result<LlmApiUrl, String> {
    LlmApiUrl::parse(&format_gemini_models_url(base_url.as_str()))
}

pub(crate) fn is_gemini_text_generation_model(model: &GeminiModel) -> bool {
    model
        .supported_generation_methods
        .as_ref()
        .map(|methods| methods.iter().any(|method| method == "generateContent"))
        .unwrap_or(true)
}

pub(crate) fn gemini_model_to_summary(model: GeminiModel) -> Option<LlmModelSummary> {
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

pub(crate) fn openai_model_to_summary(model: OpenAiModel) -> LlmModelSummary {
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

pub(crate) fn format_openai_models_urls(base_url: &str, is_ollama: bool) -> Vec<String> {
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

pub(crate) fn build_openai_models_urls(
    base_url: &LlmApiUrl,
    is_ollama: bool,
) -> Result<Vec<LlmApiUrl>, String> {
    format_openai_models_urls(base_url.as_str(), is_ollama)
        .into_iter()
        .map(|url| LlmApiUrl::parse(&url))
        .collect()
}

pub(crate) async fn get_gemini_models(
    client: &Client,
    api_key: &str,
    base_url: &LlmApiUrl,
) -> Result<Vec<LlmModelSummary>, String> {
    let url = build_gemini_models_url(base_url)?;
    let mut request = client
        .get(url.reqwest_url())
        .header("Content-Type", "application/json");

    if !api_key.is_empty() {
        request = request.header("x-goog-api-key", api_key);
    }

    let res = request.send().await.map_err(|e| e.to_string())?;

    if !res.status().is_success() {
        return Err(format!("Gemini API Error: {}", res.status()));
    }

    let response_body: GeminiModelsResponse = res.json().await.map_err(|e| e.to_string())?;

    Ok(response_body
        .models
        .unwrap_or_default()
        .into_iter()
        .filter_map(gemini_model_to_summary)
        .collect())
}

pub(crate) async fn get_openai_models(
    client: &Client,
    api_key: &str,
    base_url: &LlmApiUrl,
    is_ollama: bool,
) -> Result<Vec<LlmModelSummary>, String> {
    for url in build_openai_models_urls(base_url, is_ollama)? {
        let mut req = client
            .get(url.reqwest_url())
            .header("Content-Type", "application/json");

        if !api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", api_key));
        }

        if let Ok(res) = req.send().await {
            if res.status().is_success() {
                let text = res.text().await.unwrap_or_default();

                if let Ok(response_body) = serde_json::from_str::<OpenAiModelsResponse>(&text) {
                    return Ok(response_body
                        .data
                        .into_iter()
                        .map(openai_model_to_summary)
                        .collect());
                }

                if let Ok(response_body) = serde_json::from_str::<OllamaTagsResponse>(&text) {
                    return Ok(response_body
                        .models
                        .into_iter()
                        .map(|model| LlmModelSummary {
                            model: model.name,
                            input_price: None,
                            output_price: None,
                            context_window: None,
                            max_output_tokens: None,
                            supports_multimodal: None,
                            supports_tools: None,
                            supports_reasoning: None,
                        })
                        .collect());
                }
            }
        }
    }

    Err("Failed to fetch models from any known endpoint".to_string())
}

pub(crate) fn strategy_supports_model_listing(strategy: LlmProviderStrategy) -> bool {
    !matches!(
        strategy,
        LlmProviderStrategy::Anthropic
            | LlmProviderStrategy::AzureOpenAi
            | LlmProviderStrategy::Volcengine
            | LlmProviderStrategy::Perplexity
            | LlmProviderStrategy::OpenAiCompatibleCustomPath
            | LlmProviderStrategy::GoogleTranslate
            | LlmProviderStrategy::GoogleTranslateFree
    )
}

pub(crate) fn extract_text_response(
    choice: &rig::OneOrMany<rig::completion::AssistantContent>,
) -> Result<String, String> {
    let parts = choice
        .iter()
        .filter_map(|content| match content {
            rig::completion::AssistantContent::Text(text) => Some(text.text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();

    if parts.is_empty() {
        return Err("LLM response did not contain text output".to_string());
    }

    Ok(parts.join("\n"))
}

pub(crate) fn join_url(base_url: &str, path: &str) -> String {
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

pub(crate) fn extract_text_from_json_response(response: &Value) -> Result<String, String> {
    let mut parts = Vec::new();

    if let Some(output_text) = response.get("output_text").and_then(Value::as_str) {
        if !output_text.is_empty() {
            return Ok(output_text.to_string());
        }
    }

    if let Some(choices) = response.get("choices") {
        extract_text_parts(choices, &mut parts);
    }

    if parts.is_empty() {
        if let Some(output) = response.get("output") {
            extract_text_parts(output, &mut parts);
        }
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

fn normalize_token_usage(
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

pub(crate) fn token_usage_from_rig_usage(
    usage: Option<rig::completion::Usage>,
) -> Option<TokenUsage> {
    usage.and_then(|usage| {
        normalize_token_usage(usage.input_tokens, usage.output_tokens, usage.total_tokens)
    })
}

pub(crate) fn extract_usage_from_json_response(response: &Value) -> Option<TokenUsage> {
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

pub(crate) fn emit_llm_usage_event(
    app: &AppHandle,
    config: &LlmConfig,
    category: LlmUsageCategory,
    occurred_at: String,
    usage: Option<TokenUsage>,
) {
    let payload = LlmUsageEventPayload {
        occurred_at,
        provider: config.provider.clone(),
        model: config.model.clone(),
        category,
        usage,
    };

    if let Err(error) = app.emit(LLM_USAGE_RECORDED_EVENT, payload) {
        warn!(
            "[LLM] failed to emit usage event: provider={:?} category={:?} error={}",
            config.provider, category, error
        );
    }
}

pub(crate) fn build_standard_input(req: &StandardLlmRequest) -> String {
    req.messages
        .iter()
        .filter(|message| matches!(message.role, MessageRole::User))
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}
