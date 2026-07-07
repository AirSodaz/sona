use super::network::{LlmApiUrl, post_json_request};
use super::*;
use async_trait::async_trait;
use log::warn;
use reqwest::Client;
use rig_core::client::{CompletionClient, Nothing};
use rig_core::completion::CompletionModel;
use rig_core::providers::{anthropic, azure, copilot, gemini, ollama, openai, perplexity};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sona_core::llm_provider_protocol::{
    GeminiGenerateContentRequestParts as CoreGeminiGenerateContentRequestParts,
    GeminiModelsResponse, OllamaTagsResponse, OpenAiModelsResponse,
    build_gemini_generate_content_request_parts as build_core_gemini_generate_content_request_parts,
    extract_anthropic_text_response, normalize_token_usage, ollama_model_to_summary,
};
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

#[async_trait]
impl LlmAdapter for OpenAiAdapter {
    async fn generate(
        &self,
        _client: &Client,
        req: &StandardLlmRequest,
        config: &LlmConfig,
    ) -> Result<StandardLlmResponse, String> {
        let reqwest_client = LlmApiUrl::parse(&config.base_url)?.client(config.timeout_seconds)?;
        let client = openai::Client::builder()
            .api_key(&config.api_key)
            .base_url(&config.base_url)
            .http_client(reqwest_client)
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
                config.timeout_seconds,
            )
            .await?;

            let (text, usage) = extract_anthropic_text_response(&response)?;

            return Ok(StandardLlmResponse { text, usage });
        }

        let reqwest_client =
            LlmApiUrl::parse("https://api.anthropic.com")?.client(config.timeout_seconds)?;
        let client = anthropic::Client::builder()
            .api_key(&config.api_key)
            .http_client(reqwest_client)
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
        let reqwest_client = LlmApiUrl::parse(&config.base_url)?.client(config.timeout_seconds)?;
        let client = ollama::Client::builder()
            .api_key(Nothing)
            .base_url(config.base_url.trim_end_matches("/v1"))
            .http_client(reqwest_client)
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

pub struct AzureAdapter;

#[async_trait]
impl LlmAdapter for AzureAdapter {
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

        let api_version = config.api_version.as_deref().unwrap_or("2024-10-21");
        let reqwest_client = LlmApiUrl::parse(&config.base_url)?.client(config.timeout_seconds)?;
        let client = azure::Client::builder()
            .api_key(azure::AzureOpenAIAuth::ApiKey(config.api_key.clone()))
            .azure_endpoint(config.base_url.clone())
            .api_version(api_version)
            .http_client(reqwest_client)
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

pub struct CopilotAdapter;

#[async_trait]
impl LlmAdapter for CopilotAdapter {
    async fn generate(
        &self,
        _client: &Client,
        req: &StandardLlmRequest,
        config: &LlmConfig,
    ) -> Result<StandardLlmResponse, String> {
        let reqwest_client = LlmApiUrl::parse(&config.base_url)?.client(config.timeout_seconds)?;
        let client = copilot::Client::builder()
            .api_key(&config.api_key)
            .base_url(&config.base_url)
            .http_client(reqwest_client)
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

pub struct PerplexityAdapter;

#[async_trait]
impl LlmAdapter for PerplexityAdapter {
    async fn generate(
        &self,
        _client: &Client,
        req: &StandardLlmRequest,
        config: &LlmConfig,
    ) -> Result<StandardLlmResponse, String> {
        let reqwest_client = LlmApiUrl::parse(&config.base_url)?.client(config.timeout_seconds)?;
        let client = perplexity::Client::builder()
            .api_key(&config.api_key)
            .base_url(&config.base_url)
            .http_client(reqwest_client)
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
            let is_gemini_2_5 = config.model.contains("gemini-2.5");
            let request_parts = build_gemini_generate_content_request_parts(
                &config.base_url,
                &config.model,
                &config.api_key,
                false,
            )?;

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

            let response = post_json_request(
                &request_parts.url,
                request_parts.headers,
                payload,
                config.timeout_seconds,
            )
            .await?;

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

        let reqwest_client = LlmApiUrl::parse(&config.base_url)?.client(config.timeout_seconds)?;
        let client = gemini::Client::builder()
            .api_key(&config.api_key)
            .base_url(clean_gemini_base_url(&config.base_url))
            .http_client(reqwest_client)
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
            config.timeout_seconds,
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
            LlmProviderStrategy::AzureOpenAi => Box::new(AzureAdapter),
            LlmProviderStrategy::Copilot => Box::new(CopilotAdapter),
            LlmProviderStrategy::Perplexity => Box::new(PerplexityAdapter),
            _ => Box::new(GenericHttpAdapter),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct GeminiGenerateContentRequestParts {
    pub(crate) url: LlmApiUrl,
    pub(crate) headers: Vec<(&'static str, String)>,
}

pub(crate) fn build_gemini_generate_content_request_parts(
    base_url: &str,
    model: &str,
    api_key: &str,
    stream: bool,
) -> Result<GeminiGenerateContentRequestParts, String> {
    let CoreGeminiGenerateContentRequestParts { url, headers } =
        build_core_gemini_generate_content_request_parts(base_url, model, api_key, stream)?;
    let url = LlmApiUrl::parse(&url)?;

    Ok(GeminiGenerateContentRequestParts { url, headers })
}

pub(crate) fn build_gemini_models_url(base_url: &LlmApiUrl) -> Result<LlmApiUrl, String> {
    LlmApiUrl::parse(&format_gemini_models_url(base_url.as_str()))
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

        if let Ok(res) = req.send().await
            && res.status().is_success()
        {
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
                    .map(ollama_model_to_summary)
                    .collect());
            }
        }
    }

    Err("Failed to fetch models from any known endpoint".to_string())
}

pub(crate) fn extract_text_response(
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

pub(crate) fn token_usage_from_rig_usage(
    usage: Option<rig_core::completion::Usage>,
) -> Option<TokenUsage> {
    usage.and_then(|usage| {
        normalize_token_usage(usage.input_tokens, usage.output_tokens, usage.total_tokens)
    })
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
