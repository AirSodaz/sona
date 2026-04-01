use futures_util::future::BoxFuture;
use reqwest::Client;
use rig::client::{CompletionClient, Nothing};
use rig::completion::CompletionModel;
use rig::providers::{anthropic, gemini, ollama, openai};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

const DEFAULT_SEGMENT_CHUNK_SIZE: usize = 30;
const LLM_TASK_PROGRESS_EVENT: &str = "llm-task-progress";
const LLM_TASK_CHUNK_EVENT: &str = "llm-task-chunk";

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LlmProvider {
    OpenAi,
    OpenAiResponses,
    #[serde(rename = "azure_openai")]
    AzureOpenAi,
    Anthropic,
    Gemini,
    Ollama,
    DeepSeek,
    Kimi,
    SiliconFlow,
    Qwen,
    QwenPortal,
    MinimaxGlobal,
    MinimaxCn,
    OpenRouter,
    LmStudio,
    Groq,
    XAi,
    MistralAi,
    Perplexity,
    Volcengine,
    Chatglm,
    OpenAiCompatible,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LlmTaskType {
    Polish,
    Translate,
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
    pub scenario_prompt: Option<String>,
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

fn join_url(base_url: &str, path: &str) -> String {
    format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
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

async fn post_json_request(
    url: &str,
    headers: Vec<(&str, String)>,
    body: Value,
) -> Result<Value, String> {
    let client = Client::new();
    let mut request = client.post(url).header("Content-Type", "application/json");

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

async fn generate_with_openai_chat_api(
    url: &str,
    api_key: &str,
    model: &str,
    input: &str,
    temperature: Option<f32>,
    extra_headers: Vec<(&str, String)>,
) -> Result<String, String> {
    let mut headers = vec![];
    if !api_key.is_empty() {
        headers.push(("Authorization", format!("Bearer {}", api_key)));
    }
    headers.extend(extra_headers);

    let payload = json!({
        "model": model,
        "messages": [
            {
                "role": "user",
                "content": input,
            }
        ],
        "temperature": temperature.unwrap_or(0.7),
    });

    let response = post_json_request(url, headers, payload).await?;
    extract_text_from_json_response(&response)
}

async fn generate_with_openai_responses_api(
    base_url: &str,
    api_key: &str,
    model: &str,
    input: &str,
    temperature: Option<f32>,
    api_path: Option<&str>,
) -> Result<String, String> {
    let url = join_url(base_url, api_path.unwrap_or("/v1/responses"));
    let payload = json!({
        "model": model,
        "input": input,
        "temperature": temperature.unwrap_or(0.7),
    });

    let response = post_json_request(
        &url,
        vec![("Authorization", format!("Bearer {}", api_key))],
        payload,
    )
    .await?;

    extract_text_from_json_response(&response)
}

async fn generate_with_azure_openai(
    base_url: &str,
    api_key: &str,
    deployment: &str,
    input: &str,
    temperature: Option<f32>,
    api_version: Option<&str>,
) -> Result<String, String> {
    let version = api_version.unwrap_or("2024-10-21");
    let deployment = deployment.trim();
    let endpoint = format!(
        "{}/openai/deployments/{}/chat/completions?api-version={}",
        base_url.trim_end_matches('/'),
        deployment,
        version
    );

    let payload = json!({
        "messages": [
            {
                "role": "user",
                "content": input,
            }
        ],
        "temperature": temperature.unwrap_or(0.7),
    });

    let response = post_json_request(
        &endpoint,
        vec![("api-key", api_key.to_string())],
        payload,
    )
    .await?;

    extract_text_from_json_response(&response)
}

async fn generate_with_openai_custom_path(
    base_url: &str,
    api_key: &str,
    model: &str,
    input: &str,
    temperature: Option<f32>,
    api_path: Option<&str>,
) -> Result<String, String> {
    let url = join_url(base_url, api_path.unwrap_or("/v1/chat/completions"));
    generate_with_openai_chat_api(&url, api_key, model, input, temperature, vec![]).await
}

async fn generate_with_perplexity(
    api_key: &str,
    model: &str,
    input: &str,
    temperature: Option<f32>,
) -> Result<String, String> {
    generate_with_openai_chat_api(
        "https://api.perplexity.ai/chat/completions",
        api_key,
        model,
        input,
        temperature,
        vec![],
    )
    .await
}

async fn generate_with_openai_compatible(
    base_url: &str,
    api_key: &str,
    model: &str,
    input: &str,
    temperature: Option<f32>,
) -> Result<String, String> {
    let client = openai::Client::builder()
        .api_key(api_key)
        .base_url(base_url)
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .completion_model(model)
        .completion_request(input)
        .temperature_opt(temperature.map(|value| value as f64))
        .send()
        .await
        .map_err(|error| error.to_string())?;

    extract_text_response(&response.choice)
}

async fn generate_with_anthropic(
    base_url: &str,
    api_key: &str,
    model: &str,
    input: &str,
    temperature: Option<f32>,
) -> Result<String, String> {
    let client = anthropic::Client::builder()
        .api_key(api_key)
        .base_url(base_url)
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .completion_model(model)
        .completion_request(input)
        .temperature_opt(temperature.map(|value| value as f64))
        .send()
        .await
        .map_err(|error| error.to_string())?;

    extract_text_response(&response.choice)
}

async fn generate_with_gemini(
    base_url: &str,
    api_key: &str,
    model: &str,
    input: &str,
    temperature: Option<f32>,
) -> Result<String, String> {
    let client = gemini::Client::builder()
        .api_key(api_key)
        .base_url(clean_gemini_base_url(base_url))
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .completion_model(model)
        .completion_request(input)
        .temperature_opt(temperature.map(|value| value as f64))
        .send()
        .await
        .map_err(|error| error.to_string())?;

    extract_text_response(&response.choice)
}

async fn generate_with_ollama(
    base_url: &str,
    model: &str,
    input: &str,
    temperature: Option<f32>,
) -> Result<String, String> {
    let client = ollama::Client::builder()
        .api_key(Nothing)
        .base_url(base_url.trim_end_matches("/v1"))
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .completion_model(model)
        .completion_request(input)
        .temperature_opt(temperature.map(|value| value as f64))
        .send()
        .await
        .map_err(|error| error.to_string())?;

    extract_text_response(&response.choice)
}

async fn generate_with_rig(request: LlmGenerateRequest) -> Result<String, String> {
    let config = request.config;

    match config.provider {
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
        | LlmProvider::Chatglm
        | LlmProvider::OpenAiCompatible => {
            generate_with_openai_compatible(
                &config.base_url,
                &config.api_key,
                &config.model,
                &request.input,
                config.temperature,
            )
            .await
        }
        LlmProvider::Anthropic => {
            generate_with_anthropic(
                &config.base_url,
                &config.api_key,
                &config.model,
                &request.input,
                config.temperature,
            )
            .await
        }
        LlmProvider::Gemini => {
            generate_with_gemini(
                &config.base_url,
                &config.api_key,
                &config.model,
                &request.input,
                config.temperature,
            )
            .await
        }
        LlmProvider::Ollama => {
            generate_with_ollama(
                &config.base_url,
                &config.model,
                &request.input,
                config.temperature,
            )
            .await
        }
        LlmProvider::OpenAiResponses => {
            generate_with_openai_responses_api(
                &config.base_url,
                &config.api_key,
                &config.model,
                &request.input,
                config.temperature,
                config.api_path.as_deref(),
            )
            .await
        }
        LlmProvider::AzureOpenAi => {
            generate_with_azure_openai(
                &config.base_url,
                &config.api_key,
                &config.model,
                &request.input,
                config.temperature,
                config.api_version.as_deref(),
            )
            .await
        }
        LlmProvider::Perplexity => {
            generate_with_perplexity(
                &config.api_key,
                &config.model,
                &request.input,
                config.temperature,
            )
            .await
        }
        LlmProvider::Volcengine => {
            generate_with_openai_custom_path(
                &config.base_url,
                &config.api_key,
                &config.model,
                &request.input,
                config.temperature,
                config.api_path.as_deref(),
            )
            .await
        }
    }
}

fn normalize_chunk_size(chunk_size: Option<usize>) -> usize {
    chunk_size
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_SEGMENT_CHUNK_SIZE)
}

fn validate_llm_config(config: &LlmConfig) -> Result<(), String> {
    if config.model.trim().is_empty() {
        return Err("Model name cannot be empty".to_string());
    }

    Ok(())
}

fn validate_task_request(task_id: &str, config: &LlmConfig) -> Result<(), String> {
    if task_id.trim().is_empty() {
        return Err("Task ID cannot be empty".to_string());
    }

    validate_llm_config(config)
}

fn clean_json_response(response_text: &str) -> String {
    let mut cleaned = response_text.trim().to_string();

    if cleaned.starts_with("```json") {
        cleaned = cleaned[7..].to_string();
    } else if cleaned.starts_with("```") {
        cleaned = cleaned[3..].to_string();
    }

    if cleaned.ends_with("```") {
        cleaned.truncate(cleaned.len() - 3);
    }

    cleaned.trim().to_string()
}

fn task_label(task_type: LlmTaskType) -> &'static str {
    match task_type {
        LlmTaskType::Polish => "polish",
        LlmTaskType::Translate => "translate",
    }
}

fn chunk_error(task_type: LlmTaskType, chunk_number: usize, error: impl Into<String>) -> String {
    format!(
        "{} chunk {} failed: {}",
        task_label(task_type),
        chunk_number,
        error.into()
    )
}

fn parse_json_array<T: DeserializeOwned>(
    response_text: &str,
    task_type: LlmTaskType,
    chunk_number: usize,
) -> Result<Vec<T>, String> {
    let cleaned = clean_json_response(response_text);

    serde_json::from_str::<Vec<T>>(&cleaned)
        .map_err(|error| chunk_error(task_type, chunk_number, format!("invalid JSON response: {error}")))
}

fn validate_segment_ids<T, GetId>(
    parsed: &[T],
    expected: &[LlmSegmentInput],
    task_type: LlmTaskType,
    chunk_number: usize,
    get_id: GetId,
) -> Result<(), String>
where
    GetId: Fn(&T) -> &str,
{
    if parsed.len() != expected.len() {
        return Err(chunk_error(
            task_type,
            chunk_number,
            format!(
                "expected {} objects but received {}",
                expected.len(),
                parsed.len()
            ),
        ));
    }

    for (index, (actual, expected_segment)) in parsed.iter().zip(expected.iter()).enumerate() {
        let actual_id = get_id(actual);
        if actual_id != expected_segment.id {
            return Err(chunk_error(
                task_type,
                chunk_number,
                format!(
                    "segment {} expected id '{}' but received '{}'",
                    index + 1,
                    expected_segment.id,
                    actual_id
                ),
            ));
        }
    }

    Ok(())
}

fn parse_polish_chunk(
    response_text: &str,
    expected: &[LlmSegmentInput],
    chunk_number: usize,
) -> Result<Vec<PolishedSegment>, String> {
    let parsed = parse_json_array::<PolishedSegment>(response_text, LlmTaskType::Polish, chunk_number)?;
    validate_segment_ids(&parsed, expected, LlmTaskType::Polish, chunk_number, |item| &item.id)?;
    Ok(parsed)
}

fn parse_translate_chunk(
    response_text: &str,
    expected: &[LlmSegmentInput],
    chunk_number: usize,
) -> Result<Vec<TranslatedSegment>, String> {
    let parsed =
        parse_json_array::<TranslatedSegment>(response_text, LlmTaskType::Translate, chunk_number)?;
    validate_segment_ids(&parsed, expected, LlmTaskType::Translate, chunk_number, |item| &item.id)?;
    Ok(parsed)
}

fn build_polish_prompt(
    segments: &[LlmSegmentInput],
    context: Option<&str>,
    keywords: Option<&str>,
    scenario_prompt: Option<&str>,
) -> String {
    let json_str = serde_json::to_string(segments).unwrap_or_else(|_| "[]".to_string());
    let mut prompt = String::new();

    if let Some(value) = scenario_prompt {
        if !value.trim().is_empty() {
            prompt.push_str("[User Context]\n");
            prompt.push_str(value.trim());
            prompt.push_str("\n\n");
        }
    }

    if let Some(value) = context {
        if !value.trim().is_empty() {
            prompt.push_str("[User Context]\n");
            prompt.push_str(value.trim());
            prompt.push_str("\n\n");
        }
    }

    if let Some(value) = keywords {
        if !value.trim().is_empty() {
            prompt.push_str("[User Keywords]\n");
            prompt.push_str(value.trim());
            prompt.push_str("\n\n");
        }
    }

    prompt.push_str("You are a professional editor. The following text segments are from a speech-to-text transcription and may contain errors.\n");
    prompt.push_str("Your task is to:\n");
    prompt.push_str("1. Fix any speech recognition errors.\n");
    prompt.push_str("2. Improve grammar and clarity.\n");
    prompt.push_str("3. Keep the meaning unchanged.\n");
    prompt.push_str("4. Do NOT translate. Keep the original language.\n\n");
    prompt.push_str("CRITICAL INSTRUCTIONS:\n");
    prompt.push_str("1. You MUST maintain the EXACT JSON array structure.\n");
    prompt.push_str("2. The output MUST be valid JSON and ONLY valid JSON. Do not include markdown formatting like ```json.\n");
    prompt.push_str("3. Return an array of objects with the EXACT SAME 'id' field, and the polished text in the 'text' field.\n");
    prompt.push_str(&format!(
        "4. Do not combine or split segments. There must be exactly {} objects in the output.\n\n",
        segments.len()
    ));
    prompt.push_str("Input:\n");
    prompt.push_str(&json_str);

    prompt
}

fn language_name(code: &str) -> String {
    match code {
        "zh" => "Chinese (Simplified)".to_string(),
        "en" => "English".to_string(),
        "ja" => "Japanese".to_string(),
        "ko" => "Korean".to_string(),
        "fr" => "French".to_string(),
        "de" => "German".to_string(),
        "es" => "Spanish".to_string(),
        _ => code.to_string(),
    }
}

fn build_translate_prompt(segments: &[LlmSegmentInput], target_language: &str) -> String {
    let json_str = serde_json::to_string(segments).unwrap_or_else(|_| "[]".to_string());

    format!(
        "You are a professional translator. Translate the following array of text segments into {}.\n\
CRITICAL INSTRUCTIONS:\n\
1. You MUST maintain the EXACT JSON array structure.\n\
2. The output MUST be valid JSON and ONLY valid JSON. Do not include markdown formatting like ```json.\n\
3. Return an array of objects with the EXACT SAME 'id' field, but replace 'text' with 'translation'.\n\
4. Do not combine or split segments. There must be exactly {} objects in the output.\n\n\
Input:\n\
{}",
        language_name(target_language),
        segments.len(),
        json_str
    )
}

async fn run_segment_task<Output, BuildPrompt, ParseChunk, GenerateFn, EmitChunkFn, EmitProgressFn>(
    task_id: &str,
    task_type: LlmTaskType,
    segments: &[LlmSegmentInput],
    chunk_size: Option<usize>,
    mut build_prompt: BuildPrompt,
    mut parse_chunk: ParseChunk,
    mut generate_text: GenerateFn,
    mut emit_chunk: EmitChunkFn,
    mut emit_progress: EmitProgressFn,
) -> Result<Vec<Output>, String>
where
    Output: Serialize + Clone,
    BuildPrompt: FnMut(&[LlmSegmentInput]) -> String,
    ParseChunk: FnMut(&str, &[LlmSegmentInput], usize) -> Result<Vec<Output>, String>,
    GenerateFn: FnMut(String) -> BoxFuture<'static, Result<String, String>>,
    EmitChunkFn: FnMut(LlmTaskChunkPayload<Output>) -> Result<(), String>,
    EmitProgressFn: FnMut(LlmTaskProgressPayload) -> Result<(), String>,
{
    if segments.is_empty() {
        return Ok(Vec::new());
    }

    let normalized_chunk_size = normalize_chunk_size(chunk_size);
    let total_chunks = (segments.len() + normalized_chunk_size - 1) / normalized_chunk_size;
    let mut results = Vec::with_capacity(segments.len());

    for (chunk_index, chunk) in segments.chunks(normalized_chunk_size).enumerate() {
        let chunk_number = chunk_index + 1;
        let prompt = build_prompt(chunk);
        let response_text = generate_text(prompt)
            .await
            .map_err(|error| chunk_error(task_type, chunk_number, error))?;
        let parsed = parse_chunk(&response_text, chunk, chunk_number)?;

        emit_chunk(LlmTaskChunkPayload {
            task_id: task_id.to_string(),
            task_type,
            chunk_index: chunk_number,
            total_chunks,
            items: parsed.clone(),
        })?;

        emit_progress(LlmTaskProgressPayload {
            task_id: task_id.to_string(),
            task_type,
            completed_chunks: chunk_number,
            total_chunks,
        })?;

        results.extend(parsed);
    }

    Ok(results)
}

#[tauri::command]
pub async fn generate_llm_text(request: LlmGenerateRequest) -> Result<String, String> {
    validate_llm_config(&request.config)?;

    if request.input.trim().is_empty() {
        return Err("Input cannot be empty".to_string());
    }

    generate_with_rig(request).await
}

#[tauri::command]
pub async fn polish_transcript_segments(
    app: AppHandle,
    request: PolishSegmentsRequest,
) -> Result<Vec<PolishedSegment>, String> {
    validate_task_request(&request.task_id, &request.config)?;

    let config = request.config.clone();
    let context = request.context.clone();
    let keywords = request.keywords.clone();
    let scenario_prompt = request.scenario_prompt.clone();
    let chunk_app = app.clone();

    run_segment_task(
        &request.task_id,
        LlmTaskType::Polish,
        &request.segments,
        request.chunk_size,
        move |chunk| {
            build_polish_prompt(
                chunk,
                context.as_deref(),
                keywords.as_deref(),
                scenario_prompt.as_deref(),
            )
        },
        parse_polish_chunk,
        move |prompt| {
            let config = config.clone();
            Box::pin(async move {
                generate_with_rig(LlmGenerateRequest {
                    config,
                    input: prompt,
                })
                .await
            })
        },
        move |payload| {
            chunk_app
                .emit(LLM_TASK_CHUNK_EVENT, payload)
                .map_err(|error| error.to_string())
        },
        move |payload| {
            app.emit(LLM_TASK_PROGRESS_EVENT, payload)
                .map_err(|error| error.to_string())
        },
    )
    .await
}

#[tauri::command]
pub async fn translate_transcript_segments(
    app: AppHandle,
    request: TranslateSegmentsRequest,
) -> Result<Vec<TranslatedSegment>, String> {
    validate_task_request(&request.task_id, &request.config)?;

    if request.target_language.trim().is_empty() {
        return Err("Target language cannot be empty".to_string());
    }

    let config = request.config.clone();
    let target_language = request.target_language.clone();
    let chunk_app = app.clone();

    run_segment_task(
        &request.task_id,
        LlmTaskType::Translate,
        &request.segments,
        request.chunk_size,
        move |chunk| build_translate_prompt(chunk, &target_language),
        parse_translate_chunk,
        move |prompt| {
            let config = config.clone();
            Box::pin(async move {
                generate_with_rig(LlmGenerateRequest {
                    config,
                    input: prompt,
                })
                .await
            })
        },
        move |payload| {
            chunk_app
                .emit(LLM_TASK_CHUNK_EVENT, payload)
                .map_err(|error| error.to_string())
        },
        move |payload| {
            app.emit(LLM_TASK_PROGRESS_EVENT, payload)
                .map_err(|error| error.to_string())
        },
    )
    .await
}

#[tauri::command]
pub async fn list_llm_models(request: LlmModelsRequest) -> Result<Vec<String>, String> {
    if !provider_supports_model_listing(&request.provider) {
        return Ok(vec![]);
    }

    let client = Client::new();

    match request.provider {
        LlmProvider::Gemini => {
            get_gemini_models(&client, &request.api_key, &request.base_url).await
        }
        LlmProvider::Ollama => {
            get_openai_models(&client, &request.api_key, &request.base_url, true).await
        }
        _ => get_openai_models(&client, &request.api_key, &request.base_url, false).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_segments() -> Vec<LlmSegmentInput> {
        vec![
            LlmSegmentInput {
                id: "1".to_string(),
                text: "hello".to_string(),
            },
            LlmSegmentInput {
                id: "2".to_string(),
                text: "world".to_string(),
            },
            LlmSegmentInput {
                id: "3".to_string(),
                text: "again".to_string(),
            },
        ]
    }

    #[test]
    fn openai_models_url_accepts_root_or_v1() {
        assert_eq!(
            format_openai_models_urls("https://api.openai.com", false),
            vec![
                "https://api.openai.com/v1/models".to_string(),
                "https://api.openai.com/models".to_string()
            ]
        );
        assert_eq!(
            format_openai_models_urls("https://api.openai.com/v1", false),
            vec!["https://api.openai.com/v1/models".to_string()]
        );
    }

    #[test]
    fn gemini_base_url_is_cleaned() {
        assert_eq!(
            clean_gemini_base_url("https://generativelanguage.googleapis.com/v1beta/models"),
            "https://generativelanguage.googleapis.com"
        );
        assert_eq!(
            clean_gemini_base_url("https://generativelanguage.googleapis.com/v1beta/openai"),
            "https://generativelanguage.googleapis.com"
        );
        assert_eq!(
            clean_gemini_base_url("https://generativelanguage.googleapis.com/v1/openai/"),
            "https://generativelanguage.googleapis.com"
        );
        assert_eq!(
            clean_gemini_base_url("https://generativelanguage.googleapis.com"),
            "https://generativelanguage.googleapis.com"
        );
    }

    #[test]
    fn gemini_models_url_accepts_common_inputs() {
        assert_eq!(
            format_gemini_models_url(
                "https://generativelanguage.googleapis.com/v1beta/openai",
                "test-key"
            ),
            "https://generativelanguage.googleapis.com/v1beta/models?key=test-key"
        );
    }

    #[test]
    fn gemini_model_filter_keeps_generate_content_models() {
        let text_model = GeminiModel {
            name: "models/gemini-2.5-flash".to_string(),
            supported_generation_methods: Some(vec!["generateContent".to_string()]),
        };
        let embedding_model = GeminiModel {
            name: "models/text-embedding-004".to_string(),
            supported_generation_methods: Some(vec!["embedContent".to_string()]),
        };
        let legacy_model = GeminiModel {
            name: "models/gemini-pro".to_string(),
            supported_generation_methods: None,
        };

        assert!(is_gemini_text_generation_model(&text_model));
        assert!(!is_gemini_text_generation_model(&embedding_model));
        assert!(is_gemini_text_generation_model(&legacy_model));
    }

    #[test]
    fn anthropic_listing_is_disabled() {
        assert!(!provider_supports_model_listing(&LlmProvider::Anthropic));
        assert!(!provider_supports_model_listing(&LlmProvider::AzureOpenAi));
        assert!(!provider_supports_model_listing(&LlmProvider::Volcengine));
        assert!(provider_supports_model_listing(&LlmProvider::OpenAi));
    }

    #[test]
    fn join_url_trims_duplicate_slashes() {
        assert_eq!(
            join_url("https://api.openai.com/", "/v1/responses"),
            "https://api.openai.com/v1/responses"
        );
        assert_eq!(
            join_url("https://ark.cn-beijing.volces.com", "api/v3/chat/completions"),
            "https://ark.cn-beijing.volces.com/api/v3/chat/completions"
        );
    }

    #[test]
    fn extract_text_from_chat_completions_response() {
        let response = json!({
            "choices": [
                {
                    "message": {
                        "content": "Hello from chat completions"
                    }
                }
            ]
        });

        assert_eq!(
            extract_text_from_json_response(&response).unwrap(),
            "Hello from chat completions"
        );
    }

    #[test]
    fn extract_text_from_responses_api_payload() {
        let response = json!({
            "output": [
                {
                    "content": [
                        {
                            "type": "output_text",
                            "text": "Hello from responses"
                        }
                    ]
                }
            ]
        });

        assert_eq!(
            extract_text_from_json_response(&response).unwrap(),
            "Hello from responses"
        );
    }

    #[test]
    fn clean_json_response_removes_markdown_fences() {
        assert_eq!(
            clean_json_response("```json\n[{\"id\":\"1\",\"text\":\"Hello\"}]\n```"),
            "[{\"id\":\"1\",\"text\":\"Hello\"}]"
        );
    }

    #[test]
    fn parse_polish_chunk_rejects_length_mismatch() {
        let err = parse_polish_chunk(
            r#"[{"id":"1","text":"Hello"}]"#,
            &sample_segments()[..2],
            1,
        )
        .expect_err("length mismatch should fail");

        assert!(err.contains("polish chunk 1 failed"));
        assert!(err.contains("expected 2 objects but received 1"));
    }

    #[test]
    fn parse_translate_chunk_rejects_id_order_mismatch() {
        let err = parse_translate_chunk(
            r#"[{"id":"2","translation":"B"},{"id":"1","translation":"A"}]"#,
            &sample_segments()[..2],
            2,
        )
        .expect_err("id order mismatch should fail");

        assert!(err.contains("translate chunk 2 failed"));
        assert!(err.contains("expected id '1'"));
    }

    #[test]
    fn build_polish_prompt_contains_context_and_keywords() {
        let prompt = build_polish_prompt(
            &sample_segments()[..2],
            Some("custom context"),
            Some("keyword-a, keyword-b"),
            Some("preset context"),
        );

        assert!(prompt.contains("[User Context]"));
        assert!(prompt.contains("preset context"));
        assert!(prompt.contains("custom context"));
        assert!(prompt.contains("[User Keywords]"));
        assert!(prompt.contains("keyword-a, keyword-b"));
    }

    #[test]
    fn build_translate_prompt_contains_language_name() {
        let prompt = build_translate_prompt(&sample_segments()[..2], "zh");

        assert!(prompt.contains("Chinese (Simplified)"));
        assert!(prompt.contains("replace 'text' with 'translation'"));
    }

    #[test]
    fn chunk_payload_serializes_with_camel_case() {
        let payload = LlmTaskChunkPayload {
            task_id: "task-1".to_string(),
            task_type: LlmTaskType::Polish,
            chunk_index: 1,
            total_chunks: 2,
            items: vec![PolishedSegment {
                id: "1".to_string(),
                text: "Hello".to_string(),
            }],
        };

        let json = serde_json::to_value(payload).expect("payload should serialize");

        assert_eq!(json["taskId"], "task-1");
        assert_eq!(json["taskType"], "polish");
        assert_eq!(json["chunkIndex"], 1);
        assert_eq!(json["totalChunks"], 2);
        assert_eq!(json["items"][0]["id"], "1");
        assert_eq!(json["items"][0]["text"], "Hello");
    }

    #[tokio::test]
    async fn run_segment_task_aggregates_chunks_and_emits_progress() {
        let segments = sample_segments();
        let mut chunk_events = Vec::new();
        let mut progress_events = Vec::new();

        let result = run_segment_task(
            "task-1",
            LlmTaskType::Polish,
            &segments,
            Some(2),
            |chunk| serde_json::to_string(chunk).unwrap(),
            parse_polish_chunk,
            {
                let mut call_count = 0usize;
                move |_prompt| {
                    call_count += 1;
                    let response = match call_count {
                        1 => r#"[{"id":"1","text":"Hello"},{"id":"2","text":"World"}]"#,
                        2 => r#"[{"id":"3","text":"Again"}]"#,
                        _ => unreachable!(),
                    }
                    .to_string();

                    Box::pin(async move { Ok(response) })
                }
            },
            |payload| {
                chunk_events.push(payload);
                Ok(())
            },
            |payload| {
                progress_events.push(payload);
                Ok(())
            },
        )
        .await
        .expect("task should succeed");

        assert_eq!(
            result,
            vec![
                PolishedSegment {
                    id: "1".to_string(),
                    text: "Hello".to_string()
                },
                PolishedSegment {
                    id: "2".to_string(),
                    text: "World".to_string()
                },
                PolishedSegment {
                    id: "3".to_string(),
                    text: "Again".to_string()
                },
            ]
        );
        assert_eq!(
            chunk_events,
            vec![
                LlmTaskChunkPayload {
                    task_id: "task-1".to_string(),
                    task_type: LlmTaskType::Polish,
                    chunk_index: 1,
                    total_chunks: 2,
                    items: vec![
                        PolishedSegment {
                            id: "1".to_string(),
                            text: "Hello".to_string()
                        },
                        PolishedSegment {
                            id: "2".to_string(),
                            text: "World".to_string()
                        },
                    ],
                },
                LlmTaskChunkPayload {
                    task_id: "task-1".to_string(),
                    task_type: LlmTaskType::Polish,
                    chunk_index: 2,
                    total_chunks: 2,
                    items: vec![PolishedSegment {
                        id: "3".to_string(),
                        text: "Again".to_string()
                    }],
                },
            ]
        );
        assert_eq!(
            progress_events,
            vec![
                LlmTaskProgressPayload {
                    task_id: "task-1".to_string(),
                    task_type: LlmTaskType::Polish,
                    completed_chunks: 1,
                    total_chunks: 2,
                },
                LlmTaskProgressPayload {
                    task_id: "task-1".to_string(),
                    task_type: LlmTaskType::Polish,
                    completed_chunks: 2,
                    total_chunks: 2,
                },
            ]
        );
    }

    #[tokio::test]
    async fn run_segment_task_reports_second_chunk_failure() {
        let segments = sample_segments();

        let err = run_segment_task(
            "task-2",
            LlmTaskType::Translate,
            &segments,
            Some(2),
            |chunk| serde_json::to_string(chunk).unwrap(),
            parse_translate_chunk,
            {
                let mut call_count = 0usize;
                move |_prompt| {
                    call_count += 1;
                    Box::pin(async move {
                        match call_count {
                            1 => Ok(r#"[{"id":"1","translation":"A"},{"id":"2","translation":"B"}]"#
                                .to_string()),
                            2 => Err("boom".to_string()),
                            _ => unreachable!(),
                        }
                    })
                }
            },
            |_payload| Ok(()),
            |_payload| Ok(()),
        )
        .await
        .expect_err("second chunk should fail");

        assert_eq!(err, "translate chunk 2 failed: boom");
    }
}
