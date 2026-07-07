use async_trait::async_trait;
use log::{info, warn};
use reqwest::{Client, StatusCode, Url, header::RETRY_AFTER};
use rig_core::client::{CompletionClient, Nothing};
use rig_core::completion::CompletionModel;
use rig_core::providers::{anthropic, azure, copilot, gemini, ollama, openai, perplexity};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sona_core::llm_provider_protocol::{
    GeminiGenerateContentRequestParts as CoreGeminiGenerateContentRequestParts,
    GeminiModelsResponse, LlmModelSummary, MessageRole, OllamaTagsResponse, OpenAiModelsResponse,
    StandardLlmRequest, StandardLlmResponse, build_gemini_generate_content_request_parts,
    build_standard_input, clean_gemini_base_url, extract_anthropic_text_response,
    extract_text_from_json_response, extract_usage_from_json_response, format_gemini_models_url,
    format_openai_models_urls, gemini_model_to_summary, join_url, normalize_token_usage,
    ollama_model_to_summary, openai_model_to_summary, strategy_supports_model_listing,
    strategy_uses_openai_chat_payload,
};
use sona_core::llm_requests::{LlmConfig, LlmGenerateRequest, LlmModelsRequest};
use sona_core::llm_streaming_protocol::{OpenAiChatPayloadConfig, build_openai_chat_payload};
use sona_core::llm_tasks::LlmProviderStrategy;
use sona_core::llm_usage::TokenUsage;
use sona_core::ports::llm::{LlmModelLister, LlmTextGenerator};
use std::future::Future;
use std::time::Duration;

const GOOGLE_TRANSLATE_FREE_MAX_RETRIES: usize = 2;
const GOOGLE_TRANSLATE_FREE_MAX_RETRY_AFTER_SECS: u64 = 5;
const GOOGLE_TRANSLATE_FREE_RETRY_DELAYS_MS: [u64; GOOGLE_TRANSLATE_FREE_MAX_RETRIES] = [500, 1000];

pub fn parse_llm_api_host(base_url: &str) -> Result<Url, String> {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return Err("LLM API host cannot be empty".to_string());
    }

    let url = Url::parse(trimmed).map_err(|error| format!("LLM API host is invalid: {error}"))?;
    match url.scheme() {
        "https" => Ok(url),
        "http" if is_loopback_host(&url) => Ok(url),
        "http" => Err("LLM API host must use https:// unless it points to localhost.".to_string()),
        _ => Err("LLM API host must start with https:// or localhost http://.".to_string()),
    }
}

pub fn validate_llm_api_host(base_url: &str) -> Result<(), String> {
    parse_llm_api_host(base_url).map(|_| ())
}

fn is_loopback_host(url: &Url) -> bool {
    url.host_str()
        .map(|host| {
            let normalized = host.trim_matches(['[', ']']).to_ascii_lowercase();
            normalized == "localhost" || normalized == "127.0.0.1" || normalized == "::1"
        })
        .unwrap_or(false)
}

#[derive(Clone, Debug)]
pub struct LlmApiUrl {
    value: Url,
    https_only: bool,
}

impl LlmApiUrl {
    pub fn parse(value: &str) -> Result<Self, String> {
        let url = parse_llm_api_host(value)?;
        let https_only = url.scheme() == "https";
        Ok(Self {
            value: url,
            https_only,
        })
    }

    pub fn as_str(&self) -> &str {
        self.value.as_str()
    }

    pub fn reqwest_url(&self) -> Url {
        self.value.clone()
    }

    pub fn join(&self, path: &str) -> Result<Self, String> {
        let joined = join_url(self.value.as_str(), path);
        Self::parse(&joined)
    }

    pub fn with_query(&self, query: &str) -> Result<Self, String> {
        let mut url = self.value.clone();
        url.set_query(Some(query));
        Self::parse(url.as_str())
    }

    pub fn client(&self, timeout_seconds: Option<u64>) -> Result<Client, String> {
        use std::collections::HashMap;
        use std::sync::{Mutex, OnceLock};

        type ClientKey = (bool, Option<u64>);
        static CLIENTS: OnceLock<Mutex<HashMap<ClientKey, Client>>> = OnceLock::new();
        let map = CLIENTS.get_or_init(|| Mutex::new(HashMap::new()));

        let key = (self.https_only, timeout_seconds);

        {
            let lock = map.lock().unwrap();
            if let Some(client) = lock.get(&key) {
                return Ok(client.clone());
            }
        }

        let mut builder = Client::builder();
        if self.https_only {
            builder = builder.https_only(true);
        }
        if let Some(secs) = timeout_seconds {
            builder = builder.timeout(Duration::from_secs(secs));
        }
        let client = builder.build().map_err(|error| error.to_string())?;

        let mut lock = map.lock().unwrap();
        lock.insert(key, client.clone());
        Ok(client)
    }
}

pub async fn post_json_request(
    url: &LlmApiUrl,
    headers: Vec<(&str, String)>,
    body: Value,
    timeout_seconds: Option<u64>,
) -> Result<Value, String> {
    let client = url.client(timeout_seconds)?;
    let mut request = client
        .post(url.reqwest_url())
        .header("Content-Type", "application/json");

    for (key, value) in headers {
        if !value.is_empty() {
            request = request.header(key, value);
        }
    }

    let response = request
        .json(&body)
        .send()
        .await
        .map_err(|error| error.to_string())?;

    let status = response.status();
    let text = response.text().await.map_err(|error| error.to_string())?;

    if !status.is_success() {
        return Err(format!("LLM API Error: {} {}", status, text));
    }

    serde_json::from_str(&text).map_err(|error| error.to_string())
}

#[derive(Serialize)]
pub struct GoogleTranslateRequest {
    pub q: Vec<String>,
    pub target: String,
    pub format: String,
}

#[derive(Deserialize)]
pub struct GoogleTranslateTranslation {
    #[serde(rename = "translatedText")]
    pub translated_text: String,
}

#[derive(Deserialize)]
pub struct GoogleTranslateData {
    pub translations: Vec<GoogleTranslateTranslation>,
}

#[derive(Deserialize)]
pub struct GoogleTranslateResponse {
    pub data: GoogleTranslateData,
}

#[derive(Debug, Clone)]
pub enum GoogleTranslateFreeAttemptError {
    HttpStatus {
        status: StatusCode,
        retry_after: Option<Duration>,
    },
    Message(String),
}

fn format_attempt_label(attempts: usize) -> &'static str {
    if attempts == 1 { "attempt" } else { "attempts" }
}

pub fn parse_google_translate_free_retry_after(
    headers: &reqwest::header::HeaderMap,
) -> Option<Duration> {
    headers
        .get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(|seconds| clamp_google_translate_free_retry_after(Duration::from_secs(seconds)))
}

fn clamp_google_translate_free_retry_after(duration: Duration) -> Duration {
    duration.min(Duration::from_secs(
        GOOGLE_TRANSLATE_FREE_MAX_RETRY_AFTER_SECS,
    ))
}

fn default_google_translate_free_retry_delay(failed_attempt: usize) -> Duration {
    let delay_ms = GOOGLE_TRANSLATE_FREE_RETRY_DELAYS_MS
        .get(failed_attempt.saturating_sub(1))
        .copied()
        .unwrap_or(
            *GOOGLE_TRANSLATE_FREE_RETRY_DELAYS_MS
                .last()
                .unwrap_or(&1000),
        );
    Duration::from_millis(delay_ms)
}

fn google_translate_free_retry_delay(
    error: &GoogleTranslateFreeAttemptError,
    failed_attempt: usize,
) -> Option<Duration> {
    match error {
        GoogleTranslateFreeAttemptError::HttpStatus {
            status,
            retry_after,
        } if *status == StatusCode::TOO_MANY_REQUESTS
            && failed_attempt <= GOOGLE_TRANSLATE_FREE_MAX_RETRIES =>
        {
            Some(
                retry_after
                    .map(clamp_google_translate_free_retry_after)
                    .unwrap_or_else(|| default_google_translate_free_retry_delay(failed_attempt)),
            )
        }
        _ => None,
    }
}

fn google_translate_free_error_summary(error: &GoogleTranslateFreeAttemptError) -> String {
    match error {
        GoogleTranslateFreeAttemptError::HttpStatus { status, .. } => {
            format!("Free API Error: {}", status)
        }
        GoogleTranslateFreeAttemptError::Message(message) => {
            format!("Free translation request failed: {}", message)
        }
    }
}

fn google_translate_free_error_message(
    error: &GoogleTranslateFreeAttemptError,
    attempts: usize,
) -> String {
    format!(
        "{} after {} {}",
        google_translate_free_error_summary(error),
        attempts,
        format_attempt_label(attempts)
    )
}

fn extract_google_translate_free_translation(
    body: &Value,
) -> Result<String, GoogleTranslateFreeAttemptError> {
    let mut translated = String::new();

    if let Some(outer_arr) = body.as_array()
        && let Some(inner_arr) = outer_arr.first().and_then(|value| value.as_array())
    {
        for part in inner_arr {
            if let Some(text) = part.get(0).and_then(|value| value.as_str()) {
                translated.push_str(text);
            }
        }
    }

    if translated.is_empty() {
        return Err(GoogleTranslateFreeAttemptError::Message(
            "Empty translation".to_string(),
        ));
    }

    Ok(translated)
}

pub async fn fetch_google_translate_free_translation(
    client: &Client,
    base_url: &LlmApiUrl,
    target_language: &str,
    text: &str,
) -> Result<String, GoogleTranslateFreeAttemptError> {
    let url = base_url
        .with_query(&format!(
            "client=gtx&sl=auto&tl={}&dt=t&q={}",
            target_language,
            urlencoding::encode(text)
        ))
        .map_err(GoogleTranslateFreeAttemptError::Message)?;
    let response = client
        .get(url.reqwest_url())
        .send()
        .await
        .map_err(|error| GoogleTranslateFreeAttemptError::Message(error.to_string()))?;

    if !response.status().is_success() {
        return Err(GoogleTranslateFreeAttemptError::HttpStatus {
            status: response.status(),
            retry_after: parse_google_translate_free_retry_after(response.headers()),
        });
    }

    let body: Value = response
        .json()
        .await
        .map_err(|error| GoogleTranslateFreeAttemptError::Message(error.to_string()))?;
    extract_google_translate_free_translation(&body)
}

pub async fn execute_google_translate_free_request<FetchFn, FetchFuture, SleepFn, SleepFuture>(
    index: usize,
    text: String,
    target_language: String,
    mut fetch: FetchFn,
    mut sleep_fn: SleepFn,
) -> Result<(usize, String), String>
where
    FetchFn: FnMut(String, String) -> FetchFuture,
    FetchFuture: Future<Output = Result<String, GoogleTranslateFreeAttemptError>>,
    SleepFn: FnMut(Duration) -> SleepFuture,
    SleepFuture: Future<Output = ()>,
{
    let mut attempts = 0usize;

    loop {
        attempts += 1;

        match fetch(text.clone(), target_language.clone()).await {
            Ok(translation) => return Ok((index, translation)),
            Err(error) => {
                if let Some(delay) = google_translate_free_retry_delay(&error, attempts) {
                    info!(
                        "[LLM] google_translate_free request hit 429 and will retry: index={} failed_attempt={} next_attempt={} delay_ms={}",
                        index,
                        attempts,
                        attempts + 1,
                        delay.as_millis()
                    );
                    sleep_fn(delay).await;
                    continue;
                }

                warn!(
                    "[LLM] google_translate_free request failed after retries: index={} attempts={} error={}",
                    index,
                    attempts,
                    google_translate_free_error_summary(&error)
                );
                return Err(google_translate_free_error_message(&error, attempts));
            }
        }
    }
}

#[async_trait]
trait LlmAdapter: Send + Sync {
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

        let input = user_input_from_request(req);

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
        let input = user_input_from_request(req);

        if config.reasoning_enabled.unwrap_or(false) {
            let url = LlmApiUrl::parse(&join_url(&config.base_url, "/v1/messages"))?;
            let budget_tokens = reasoning_budget_tokens(config.reasoning_level.as_deref());

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

        let response = client
            .completion_model(&config.model)
            .completion_request(&user_input_from_request(req))
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
            .completion_request(&user_input_from_request(req))
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

        let response = client
            .completion_model(&config.model)
            .completion_request(&user_input_from_request(req))
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

        let response = client
            .completion_model(&config.model)
            .completion_request(&user_input_from_request(req))
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
        let input = user_input_from_request(req);

        if config.reasoning_enabled.unwrap_or(false) {
            let is_gemini_2_5 = config.model.contains("gemini-2.5");
            let request_parts = build_gemini_generate_content_request_parts_for_reqwest(
                &config.base_url,
                &config.model,
                &config.api_key,
                false,
            )?;

            let thinking_config = if is_gemini_2_5 {
                json!({
                    "thinkingBudget": reasoning_budget_tokens(config.reasoning_level.as_deref()),
                    "includeThoughts": true
                })
            } else {
                json!({
                    "thinkingLevel": reasoning_level_label(config.reasoning_level.as_deref()),
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
            target: "en".to_string(),
            format: "text".to_string(),
        };

        let response = post_json_request(
            &base_url,
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

#[derive(Clone, Copy, Debug, Default)]
pub struct OnlineLlmAdapter;

#[async_trait]
impl LlmTextGenerator for OnlineLlmAdapter {
    async fn generate_text(
        &self,
        request: LlmGenerateRequest,
    ) -> Result<StandardLlmResponse, String> {
        generate_text_with_provider(request).await
    }
}

#[async_trait]
impl LlmModelLister for OnlineLlmAdapter {
    async fn list_models(&self, request: LlmModelsRequest) -> Result<Vec<LlmModelSummary>, String> {
        list_models_with_provider(request).await
    }
}

pub async fn generate_text_with_provider(
    request: LlmGenerateRequest,
) -> Result<StandardLlmResponse, String> {
    let adapter = AdapterFactory::create(request.config.strategy);
    let std_req = StandardLlmRequest {
        messages: vec![sona_core::llm_provider_protocol::StandardMessage {
            role: MessageRole::User,
            content: request.input,
        }],
        temperature: request.config.temperature.unwrap_or(0.7),
    };

    let url = LlmApiUrl::parse(&request.config.base_url)?;
    let client = url.client(request.config.timeout_seconds)?;
    adapter.generate(&client, &std_req, &request.config).await
}

pub async fn list_models_with_provider(
    request: LlmModelsRequest,
) -> Result<Vec<LlmModelSummary>, String> {
    let strategy = request
        .strategy
        .unwrap_or_else(|| LlmProviderStrategy::from_provider(&request.provider));
    if !strategy_supports_model_listing(strategy) {
        return Ok(vec![]);
    }
    validate_llm_api_host(&request.base_url)?;

    let base_url = LlmApiUrl::parse(&request.base_url)?;
    let client = base_url.client(None)?;

    match strategy {
        LlmProviderStrategy::Gemini => {
            get_gemini_models(&client, &request.api_key, &base_url).await
        }
        LlmProviderStrategy::Ollama => {
            get_openai_models(&client, &request.api_key, &base_url, true).await
        }
        _ => get_openai_models(&client, &request.api_key, &base_url, false).await,
    }
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
) -> Result<GeminiGenerateContentRequestParts, String> {
    let CoreGeminiGenerateContentRequestParts { url, headers } =
        build_gemini_generate_content_request_parts(base_url, model, api_key, stream)?;
    let url = LlmApiUrl::parse(&url)?;

    Ok(GeminiGenerateContentRequestParts { url, headers })
}

pub fn build_gemini_models_url(base_url: &LlmApiUrl) -> Result<LlmApiUrl, String> {
    LlmApiUrl::parse(&format_gemini_models_url(base_url.as_str()))
}

pub fn build_openai_models_urls(
    base_url: &LlmApiUrl,
    is_ollama: bool,
) -> Result<Vec<LlmApiUrl>, String> {
    format_openai_models_urls(base_url.as_str(), is_ollama)
        .into_iter()
        .map(|url| LlmApiUrl::parse(&url))
        .collect()
}

pub async fn get_gemini_models(
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

pub async fn get_openai_models(
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
        normalize_token_usage(usage.input_tokens, usage.output_tokens, usage.total_tokens)
    })
}

pub async fn generate_with_openai_chat_api(
    url: &LlmApiUrl,
    config: &LlmConfig,
    input: &str,
    extra_headers: Vec<(&str, String)>,
) -> Result<StandardLlmResponse, String> {
    let mut headers = vec![];
    if !config.api_key.is_empty() {
        headers.push(("Authorization", format!("Bearer {}", config.api_key)));
    }
    headers.extend(extra_headers);

    let payload = build_openai_chat_payload(openai_chat_payload_config(config), input, false);

    let response = post_json_request(url, headers, payload, config.timeout_seconds).await?;
    Ok(StandardLlmResponse {
        text: extract_text_from_json_response(&response)?,
        usage: extract_usage_from_json_response(&response),
    })
}

pub async fn generate_with_openai_responses_api(
    base_url: &str,
    api_key: &str,
    model: &str,
    input: &str,
    temperature: Option<f32>,
    api_path: Option<&str>,
) -> Result<StandardLlmResponse, String> {
    let base_url = LlmApiUrl::parse(base_url)?;
    let url = base_url.join(api_path.unwrap_or("/v1/responses"))?;
    let payload = json!({
        "model": model,
        "input": input,
        "temperature": temperature.unwrap_or(0.7),
    });

    let response = post_json_request(
        &url,
        vec![("Authorization", format!("Bearer {}", api_key))],
        payload,
        None,
    )
    .await?;

    Ok(StandardLlmResponse {
        text: extract_text_from_json_response(&response)?,
        usage: extract_usage_from_json_response(&response),
    })
}

pub async fn generate_with_openai_custom_path(
    config: &LlmConfig,
    input: &str,
) -> Result<StandardLlmResponse, String> {
    let base_url = LlmApiUrl::parse(&config.base_url)?;
    let url = base_url.join(config.api_path.as_deref().unwrap_or("/v1/chat/completions"))?;
    generate_with_openai_chat_api(&url, config, input, vec![]).await
}

fn user_input_from_request(req: &StandardLlmRequest) -> String {
    req.messages
        .iter()
        .filter(|m| matches!(m.role, MessageRole::User))
        .map(|m| m.content.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn openai_chat_payload_config(config: &LlmConfig) -> OpenAiChatPayloadConfig<'_> {
    OpenAiChatPayloadConfig {
        strategy: config.strategy,
        model: &config.model,
        temperature: config.temperature,
        reasoning_enabled: config.reasoning_enabled.unwrap_or(false),
        reasoning_level: config.reasoning_level.as_deref(),
    }
}

fn reasoning_budget_tokens(reasoning_level: Option<&str>) -> u32 {
    match reasoning_level {
        Some("low") => 1024,
        Some("high") => 4096,
        _ => 2048,
    }
}

fn reasoning_level_label(reasoning_level: Option<&str>) -> &'static str {
    match reasoning_level {
        Some("low") => "LOW",
        Some("high") => "HIGH",
        _ => "MEDIUM",
    }
}

pub fn build_standard_user_input(input: impl Into<String>, temperature: f32) -> String {
    build_standard_input(&StandardLlmRequest {
        messages: vec![sona_core::llm_provider_protocol::StandardMessage {
            role: MessageRole::User,
            content: input.into(),
        }],
        temperature,
    })
}
