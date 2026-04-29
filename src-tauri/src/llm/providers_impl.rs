#[derive(Serialize)]
struct GoogleTranslateRequest {
    q: Vec<String>,
    target: String,
    format: String,
}

#[derive(Deserialize)]
struct GoogleTranslateTranslation {
    #[serde(rename = "translatedText")]
    translated_text: String,
}

#[derive(Deserialize)]
struct GoogleTranslateData {
    translations: Vec<GoogleTranslateTranslation>,
}

#[derive(Deserialize)]
struct GoogleTranslateResponse {
    data: GoogleTranslateData,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LlmTaskType {
    Polish,
    Translate,
    Summary,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LlmUsageCategory {
    Summary,
    Translation,
    Polish,
    TitleGeneration,
    ConnectionTest,
    Generic,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LlmGenerateSource {
    TitleGeneration,
    ConnectionTest,
    Generic,
}

impl From<LlmGenerateSource> for LlmUsageCategory {
    fn from(value: LlmGenerateSource) -> Self {
        match value {
            LlmGenerateSource::TitleGeneration => LlmUsageCategory::TitleGeneration,
            LlmGenerateSource::ConnectionTest => LlmUsageCategory::ConnectionTest,
            LlmGenerateSource::Generic => LlmUsageCategory::Generic,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SummaryTemplateConfig {
    pub id: String,
    pub name: String,
    pub instructions: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LlmConfig {
    pub provider: LlmProvider,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub api_path: Option<String>,
    pub api_version: Option<String>,
    pub temperature: Option<f32>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LlmGenerateRequest {
    pub config: LlmConfig,
    pub input: String,
    pub source: Option<LlmGenerateSource>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum MessageRole {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "assistant")]
    Assistant,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StandardMessage {
    pub role: MessageRole,
    pub content: String,
}

#[derive(Debug)]
pub struct StandardLlmRequest {
    pub messages: Vec<StandardMessage>,
    pub temperature: f32,
    #[allow(dead_code)]
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StandardLlmResponse {
    pub text: String,
    pub usage: Option<TokenUsage>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmUsageEventPayload {
    pub occurred_at: String,
    pub provider: LlmProvider,
    pub model: String,
    pub category: LlmUsageCategory,
    pub usage: Option<TokenUsage>,
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
        let client = openai::Client::builder()
            .api_key(&config.api_key)
            .base_url(&config.base_url)
            .build()
            .map_err(|error| error.to_string())?;

        // For now, we use the first message's content as input to match current behavior
        // or join all user messages.
        let input = req.messages.iter()
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

pub struct AnthropicAdapter;

#[async_trait]
impl LlmAdapter for AnthropicAdapter {
    async fn generate(
        &self,
        _client: &Client,
        req: &StandardLlmRequest,
        config: &LlmConfig,
    ) -> Result<StandardLlmResponse, String> {
        let client = anthropic::Client::builder()
            .api_key(&config.api_key)
            .base_url(&config.base_url)
            .build()
            .map_err(|error| error.to_string())?;

        let input = req.messages.iter()
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

        let input = req.messages.iter()
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
        let client = gemini::Client::builder()
            .api_key(&config.api_key)
            .base_url(clean_gemini_base_url(&config.base_url))
            .build()
            .map_err(|error| error.to_string())?;

        let input = req.messages.iter()
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

pub struct GoogleTranslateAdapter;

#[async_trait]
impl LlmAdapter for GoogleTranslateAdapter {
    async fn generate(
        &self,
        client: &Client,
        req: &StandardLlmRequest,
        config: &LlmConfig,
    ) -> Result<StandardLlmResponse, String> {
        let input = req.messages.iter()
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        if config.provider == LlmProvider::GoogleTranslateFree {
            let base_url = config.base_url.clone();
            let fetch_client = client.clone();

            let (_, text) = execute_google_translate_free_request(
                0,
                input,
                "en".to_string(),
                move |text, target| {
                    let url = format!(
                        "{}?client=gtx&sl=auto&tl={}&dt=t&q={}",
                        base_url.trim_end_matches('/'),
                        target,
                        urlencoding::encode(&text)
                    );
                    let client = fetch_client.clone();
                    async move {
                        let response = client.get(&url).send().await.map_err(|e| GoogleTranslateFreeAttemptError::Message(e.to_string()))?;
                        let status = response.status();
                        if !status.is_success() {
                            return Err(GoogleTranslateFreeAttemptError::HttpStatus {
                                status,
                                retry_after: None
                            });
                        }
                        let body: Value = response.json().await.map_err(|e| GoogleTranslateFreeAttemptError::Message(e.to_string()))?;
                        let mut result = String::new();
                        if let Some(outer_arr) = body.as_array() {
                            if let Some(inner_arr) = outer_arr.get(0).and_then(|v| v.as_array()) {
                                for part in inner_arr {
                                    if let Some(text) = part.get(0).and_then(|v| v.as_str()) {
                                        result.push_str(text);
                                    }
                                }
                            }
                        }
                        if result.is_empty() {
                            return Err(GoogleTranslateFreeAttemptError::Message("No translation returned".to_string()));
                        }
                        Ok(result)
                    }
                },
                tokio::time::sleep,
            ).await?;

            return Ok(StandardLlmResponse { text, usage: None });
        }

        let payload = GoogleTranslateRequest {
            q: vec![input],
            target: "en".to_string(), // Default fallback
            format: "text".to_string(),
        };

        let url = format!(
            "{}?key={}",
            config.base_url.trim_end_matches('/'),
            config.api_key
        );

        let response = post_json_request(&url, vec![], json!(payload)).await?;
        let text = extract_text_from_json_response(&response)?;

        Ok(StandardLlmResponse {
            text,
            usage: None,
        })
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
        let input = req.messages.iter()
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        let response = match config.provider {
            LlmProvider::OpenAiResponses => {
                generate_with_openai_responses_api(
                    &config.base_url,
                    &config.api_key,
                    &config.model,
                    &input,
                    Some(req.temperature),
                    config.api_path.as_deref(),
                ).await?
            }
            LlmProvider::AzureOpenAi => {
                generate_with_azure_openai(
                    &config.base_url,
                    &config.api_key,
                    &config.model,
                    &input,
                    Some(req.temperature),
                    config.api_version.as_deref(),
                ).await?
            }
            LlmProvider::Perplexity => {
                generate_with_perplexity(
                    &config.api_key,
                    &config.model,
                    &input,
                    Some(req.temperature),
                ).await?
            }
            _ => {
                generate_with_openai_custom_path(
                    &config.base_url,
                    &config.api_key,
                    &config.model,
                    &input,
                    Some(req.temperature),
                    config.api_path.as_deref(),
                ).await?
            }
        };

        Ok(response)
    }
}

pub struct AdapterFactory;

impl AdapterFactory {
    pub fn create(provider: LlmProvider) -> Box<dyn LlmAdapter> {
        match provider {
            LlmProvider::OpenAi
            | LlmProvider::DeepSeek
            | LlmProvider::Kimi
            | LlmProvider::SiliconFlow
            | LlmProvider::Qwen
            | LlmProvider::QwenPortal
            | LlmProvider::MinimaxGlobal
            | LlmProvider::MinimaxCn
            | LlmProvider::OpenRouter
            | LlmProvider::LmStudio
            | LlmProvider::Groq
            | LlmProvider::XAi
            | LlmProvider::MistralAi
            | LlmProvider::OpenAiCompatible => Box::new(OpenAiAdapter),
            LlmProvider::Anthropic => Box::new(AnthropicAdapter),
            LlmProvider::Ollama => Box::new(OllamaAdapter),
            LlmProvider::Gemini => Box::new(GeminiAdapter),
            LlmProvider::GoogleTranslate | LlmProvider::GoogleTranslateFree => {
                Box::new(GoogleTranslateAdapter)
            }
            _ => Box::new(GenericHttpAdapter),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LlmModelsRequest {
    pub provider: LlmProvider,
    pub base_url: String,
    pub api_key: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LlmSegmentInput {
    pub id: String,
    pub text: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PolishSegmentsRequest {
    pub task_id: String,
    pub config: LlmConfig,
    pub segments: Vec<LlmSegmentInput>,
    pub chunk_size: Option<usize>,
    pub context: Option<String>,
    pub keywords: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TranslateSegmentsRequest {
    pub task_id: String,
    pub config: LlmConfig,
    pub segments: Vec<LlmSegmentInput>,
    pub chunk_size: Option<usize>,
    pub target_language: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SummarySegmentInput {
    pub id: String,
    pub text: String,
    pub start: f32,
    pub end: f32,
    pub is_final: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SummarizeTranscriptRequest {
    pub task_id: String,
    pub config: LlmConfig,
    pub template: SummaryTemplateConfig,
    pub segments: Vec<SummarySegmentInput>,
    pub chunk_char_budget: Option<usize>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PolishedSegment {
    pub id: String,
    pub text: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranslatedSegment {
    pub id: String,
    pub translation: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSummaryResult {
    pub template_id: String,
    pub content: String,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LlmTaskProgressPayload {
    pub task_id: String,
    pub task_type: LlmTaskType,
    pub completed_chunks: usize,
    pub total_chunks: usize,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LlmTaskChunkPayload<T> {
    pub task_id: String,
    pub task_type: LlmTaskType,
    pub chunk_index: usize,
    pub total_chunks: usize,
    pub items: Vec<T>,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LlmTaskTextPayload {
    pub task_id: String,
    pub task_type: LlmTaskType,
    pub text: String,
    pub delta: String,
}

#[derive(Deserialize)]
struct OpenAiModel {
    id: String,
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
struct GeminiModel {
    name: String,
    #[serde(rename = "supportedGenerationMethods")]
    supported_generation_methods: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct GeminiModelsResponse {
    models: Option<Vec<GeminiModel>>,
}

fn clean_gemini_base_url(base_url: &str) -> &str {
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

fn format_gemini_models_url(base_url: &str, api_key: &str) -> String {
    let cleaned_base = clean_gemini_base_url(base_url);

    format!("{}/v1beta/models?key={}", cleaned_base, api_key)
}

fn is_gemini_text_generation_model(model: &GeminiModel) -> bool {
    model
        .supported_generation_methods
        .as_ref()
        .map(|methods| methods.iter().any(|method| method == "generateContent"))
        .unwrap_or(true)
}

fn format_openai_models_urls(base_url: &str, is_ollama: bool) -> Vec<String> {
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

async fn get_gemini_models(
    client: &Client,
    api_key: &str,
    base_url: &str,
) -> Result<Vec<String>, String> {
    let url = format_gemini_models_url(base_url, api_key);
    let res = client
        .get(&url)
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !res.status().is_success() {
        return Err(format!("Gemini API Error: {}", res.status()));
    }

    let response_body: GeminiModelsResponse = res.json().await.map_err(|e| e.to_string())?;

    Ok(response_body
        .models
        .unwrap_or_default()
        .into_iter()
        .filter(is_gemini_text_generation_model)
        .map(|m| m.name.trim_start_matches("models/").to_string())
        .collect())
}

async fn get_openai_models(
    client: &Client,
    api_key: &str,
    base_url: &str,
    is_ollama: bool,
) -> Result<Vec<String>, String> {
    for url in format_openai_models_urls(base_url, is_ollama) {
        let mut req = client.get(&url).header("Content-Type", "application/json");

        if !api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", api_key));
        }

        if let Ok(res) = req.send().await {
            if res.status().is_success() {
                let text = res.text().await.unwrap_or_default();

                if let Ok(response_body) = serde_json::from_str::<OpenAiModelsResponse>(&text) {
                    return Ok(response_body.data.into_iter().map(|m| m.id).collect());
                }

                if let Ok(response_body) = serde_json::from_str::<OllamaTagsResponse>(&text) {
                    return Ok(response_body.models.into_iter().map(|m| m.name).collect());
                }
            }
        }
    }

    Err("Failed to fetch models from any known endpoint".to_string())
}

fn provider_supports_model_listing(provider: &LlmProvider) -> bool {
    !matches!(
        provider,
        LlmProvider::Anthropic
            | LlmProvider::AzureOpenAi
            | LlmProvider::Volcengine
            | LlmProvider::Perplexity
            | LlmProvider::GoogleTranslate
            | LlmProvider::GoogleTranslateFree
    )
}

fn extract_text_response(
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

/// Keeps the progressively built response text and emits both the full text and
/// the latest delta, because downstream listeners render partial output while
/// also needing the complete accumulated value for replacement-style updates.
struct StreamTextAccumulator<'a, EmitFn>
where
    EmitFn: FnMut(&str, &str) -> Result<(), String>,
{
    text: String,
    emitted_any: bool,
    emit_delta: &'a mut EmitFn,
}

impl<'a, EmitFn> StreamTextAccumulator<'a, EmitFn>
where
    EmitFn: FnMut(&str, &str) -> Result<(), String>,
{
    fn new(emit_delta: &'a mut EmitFn) -> Self {
        Self {
            text: String::new(),
            emitted_any: false,
            emit_delta,
        }
    }

    fn push(&mut self, delta: &str) -> Result<(), String> {
        if delta.is_empty() {
            return Ok(());
        }

        self.text.push_str(delta);
        self.emitted_any = true;
        (self.emit_delta)(&self.text, delta)
    }

    fn text(&self) -> String {
        self.text.clone()
    }
}

/// Reassembles transport chunks into complete lines before higher-level
/// streaming parsers inspect them. HTTP/SSE providers can split a single line
/// across arbitrary network chunks, so chunk boundaries are not message
/// boundaries.
#[derive(Default)]
struct StreamingLineBuffer {
    buffer: String,
}

impl StreamingLineBuffer {
    fn process(&mut self, chunk: &str) -> Vec<String> {
        if chunk.find('\n').is_none() {
            // Keep buffering until we see a line terminator. Partial lines are
            // not safe to parse yet.
            self.buffer.push_str(chunk);
            return Vec::new();
        }

        self.buffer.push_str(chunk);
        let mut lines = self
            .buffer
            .split('\n')
            .map(|line| line.to_string())
            .collect::<Vec<_>>();
        self.buffer = lines.pop().unwrap_or_default();
        lines
    }

    fn flush(&mut self) -> Vec<String> {
        if self.buffer.trim().is_empty() {
            self.buffer.clear();
            return Vec::new();
        }

        let line = self.buffer.clone();
        self.buffer.clear();
        vec![line]
    }
}

/// Collects SSE `data:` lines and emits one logical event per blank-line
/// separator. This keeps us aligned with SSE framing instead of assuming each
/// incoming chunk is already a complete event.
#[derive(Default)]
struct SseEventBuffer {
    line_buffer: StreamingLineBuffer,
    data_lines: Vec<String>,
}

impl SseEventBuffer {
    fn process(&mut self, chunk: &str) -> Vec<String> {
        let mut events = Vec::new();
        for line in self.line_buffer.process(chunk) {
            self.process_line(&line, &mut events);
        }
        events
    }

    fn flush(&mut self) -> Vec<String> {
        let mut events = Vec::new();
        for line in self.line_buffer.flush() {
            self.process_line(&line, &mut events);
        }

        if !self.data_lines.is_empty() {
            events.push(self.data_lines.join("\n"));
            self.data_lines.clear();
        }

        events
    }

    fn process_line(&mut self, raw_line: &str, events: &mut Vec<String>) {
        let line = raw_line.trim_end_matches('\r');
        if line.is_empty() {
            if !self.data_lines.is_empty() {
                events.push(self.data_lines.join("\n"));
                self.data_lines.clear();
            }
            return;
        }

        if let Some(rest) = line.strip_prefix("data:") {
            self.data_lines.push(rest.trim_start().to_string());
        }
        // Ignore other SSE fields such as `event:` or `id:` because current
        // provider integrations only consume the payload carried in `data:`.
    }
}

fn join_url(base_url: &str, path: &str) -> String {
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
        Value::String(text) => {
            if !text.is_empty() {
                parts.push(text.clone());
            }
        }
        Value::Array(items) => {
            for item in items {
                extract_text_parts(item, parts);
            }
        }
        Value::Object(map) => {
            if let Some(text) = map.get("output_text").and_then(Value::as_str) {
                if !text.is_empty() {
                    parts.push(text.to_string());
                }
            }

            if let Some(text) = map.get("text").and_then(Value::as_str) {
                if !text.is_empty() {
                    parts.push(text.to_string());
                    return;
                }
            }

            if let Some(content) = map.get("content") {
                extract_text_parts(content, parts);
                return;
            }

            if let Some(message) = map.get("message") {
                extract_text_parts(message, parts);
                return;
            }

            if let Some(output) = map.get("output") {
                extract_text_parts(output, parts);
            }
        }
        _ => {}
    }
}

fn extract_text_from_json_response(response: &Value) -> Result<String, String> {
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

fn normalize_token_usage(prompt_tokens: u64, completion_tokens: u64, total_tokens: u64) -> Option<TokenUsage> {
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

fn token_usage_from_rig_usage(usage: Option<rig::completion::Usage>) -> Option<TokenUsage> {
    usage.and_then(|usage| {
        normalize_token_usage(usage.input_tokens, usage.output_tokens, usage.total_tokens)
    })
}

fn extract_usage_from_json_response(response: &Value) -> Option<TokenUsage> {
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

fn emit_llm_usage_event(
    app: &AppHandle,
    config: &LlmConfig,
    category: LlmUsageCategory,
    usage: Option<TokenUsage>,
) {
    let payload = LlmUsageEventPayload {
        occurred_at: chrono::Utc::now().to_rfc3339(),
        provider: config.provider,
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

fn build_standard_input(req: &StandardLlmRequest) -> String {
    req.messages
        .iter()
        .filter(|message| matches!(message.role, MessageRole::User))
        .map(|message| message.content.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}
