use super::network::{post_json_request, LlmApiUrl};
use super::*;
use futures_util::StreamExt;
use log::warn;
use reqwest::{header::CONTENT_TYPE, Client};
use rig::client::{CompletionClient, Nothing};
use rig::completion::{CompletionModel, GetTokenUsage};
use rig::providers::{anthropic, gemini, ollama};
use rig::streaming::StreamedAssistantContent;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

/// Keeps the progressively built response text and emits both the full text and
/// the latest delta, because downstream listeners render partial output while
/// also needing the complete accumulated value for replacement-style updates.
pub(crate) struct StreamTextAccumulator<'a, EmitFn>
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
    pub(crate) fn new(emit_delta: &'a mut EmitFn) -> Self {
        Self {
            text: String::new(),
            emitted_any: false,
            emit_delta,
        }
    }

    pub(crate) fn push(&mut self, delta: &str) -> Result<(), String> {
        if delta.is_empty() {
            return Ok(());
        }

        self.text.push_str(delta);
        self.emitted_any = true;
        (self.emit_delta)(&self.text, delta)
    }

    pub(crate) fn text(&self) -> String {
        self.text.clone()
    }
}

/// Reassembles transport chunks into complete lines before higher-level
/// streaming parsers inspect them. HTTP/SSE providers can split a single line
/// across arbitrary network chunks, so chunk boundaries are not message
/// boundaries.
#[derive(Default)]
pub(crate) struct StreamingLineBuffer {
    buffer: String,
}

impl StreamingLineBuffer {
    pub(crate) fn process(&mut self, chunk: &str) -> Vec<String> {
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

    pub(crate) fn flush(&mut self) -> Vec<String> {
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

pub(crate) fn normalize_incremental_json_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed == "```" || trimmed == "```json" {
        return None;
    }

    let trimmed = trimmed.trim_end_matches(',').trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return Some(trimmed.to_string());
    }

    None
}

pub(crate) fn parse_json_array_or_ndjson<T: DeserializeOwned>(
    response_text: &str,
    task_type: LlmTaskType,
    chunk_number: usize,
) -> Result<Vec<T>, String> {
    let cleaned = clean_json_response(response_text);
    if cleaned.starts_with('[') {
        return serde_json::from_str::<Vec<T>>(&cleaned).map_err(|error| {
            chunk_error(
                task_type,
                chunk_number,
                format!("invalid JSON response: {error}"),
            )
        });
    }

    let mut items = Vec::new();
    for line in cleaned.lines() {
        if let Some(normalized) = normalize_incremental_json_line(line) {
            let parsed = serde_json::from_str::<T>(&normalized).map_err(|error| {
                chunk_error(
                    task_type,
                    chunk_number,
                    format!("invalid JSON response: {error}"),
                )
            })?;
            items.push(parsed);
        }
    }

    if items.is_empty() {
        return Err(chunk_error(
            task_type,
            chunk_number,
            "invalid JSON response: expected NDJSON lines or a JSON array",
        ));
    }

    Ok(items)
}

async fn stream_rig_completion_model<M, EmitFn>(
    model: M,
    input: &str,
    temperature: f32,
    accumulator: &mut StreamTextAccumulator<'_, EmitFn>,
) -> Result<StandardLlmResponse, String>
where
    M: CompletionModel,
    M::StreamingResponse: Clone + Unpin + GetTokenUsage + 'static,
    EmitFn: FnMut(&str, &str) -> Result<(), String>,
{
    let mut stream = model
        .completion_request(input)
        .temperature_opt(Some(temperature as f64))
        .stream()
        .await
        .map_err(|error| error.to_string())?;

    while let Some(item) = stream.next().await {
        match item.map_err(|error| error.to_string())? {
            StreamedAssistantContent::Text(text) => accumulator.push(&text.text)?,
            StreamedAssistantContent::Final(_) => {}
            _ => {}
        }
    }

    let text = if accumulator.text.is_empty() {
        extract_text_response(&stream.choice)?
    } else {
        accumulator.text()
    };

    Ok(StandardLlmResponse {
        text,
        usage: stream
            .response
            .as_ref()
            .and_then(|response| token_usage_from_rig_usage(response.token_usage())),
    })
}

fn build_openai_stream_url(config: &LlmConfig) -> String {
    match config.provider {
        LlmProvider::AzureOpenAi => {
            let version = config.api_version.as_deref().unwrap_or("2024-10-21");
            format!(
                "{}/openai/deployments/{}/chat/completions?api-version={}",
                config.base_url.trim_end_matches('/'),
                config.model.trim(),
                version
            )
        }
        LlmProvider::Perplexity => join_url(
            &config.base_url,
            config.api_path.as_deref().unwrap_or("/chat/completions"),
        ),
        _ => join_url(
            &config.base_url,
            config.api_path.as_deref().unwrap_or("/v1/chat/completions"),
        ),
    }
}

async fn stream_openai_chat_completion<EmitFn>(
    config: &LlmConfig,
    input: &str,
    accumulator: &mut StreamTextAccumulator<'_, EmitFn>,
) -> Result<StandardLlmResponse, String>
where
    EmitFn: FnMut(&str, &str) -> Result<(), String>,
{
    let url = LlmApiUrl::parse(&build_openai_stream_url(config))?;
    let client = url.client()?;
    let payload = if config.provider == LlmProvider::AzureOpenAi {
        json!({
            "messages": [
                {
                    "role": "user",
                    "content": input,
                }
            ],
            "temperature": config.temperature.unwrap_or(0.7),
            "stream": true,
        })
    } else {
        json!({
            "model": config.model,
            "messages": [
                {
                    "role": "user",
                    "content": input,
                }
            ],
            "temperature": config.temperature.unwrap_or(0.7),
            "stream": true,
        })
    };

    let mut request = client
        .post(url.reqwest_url())
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream");

    if config.provider == LlmProvider::AzureOpenAi {
        request = request.header("api-key", config.api_key.clone());
    } else if !config.api_key.is_empty() {
        request = request.header("Authorization", format!("Bearer {}", config.api_key));
    }

    let response = request
        .json(&payload)
        .send()
        .await
        .map_err(|error| error.to_string())?;

    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(format!("LLM API Error: {} {}", status, text));
    }

    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();

    if !content_type.contains("text/event-stream") {
        let body = response.text().await.map_err(|error| error.to_string())?;
        let json = serde_json::from_str::<Value>(&body).map_err(|error| error.to_string())?;
        return Ok(StandardLlmResponse {
            text: extract_text_from_json_response(&json)?,
            usage: extract_usage_from_json_response(&json),
        });
    }

    let mut sse = SseEventBuffer::default();
    let mut byte_stream = response.bytes_stream();

    while let Some(item) = byte_stream.next().await {
        let bytes = item.map_err(|error| error.to_string())?;
        let chunk = String::from_utf8_lossy(&bytes);
        for data in sse.process(&chunk) {
            if data.trim() == "[DONE]" {
                continue;
            }

            let event = serde_json::from_str::<Value>(&data).map_err(|error| error.to_string())?;
            if let Some(delta) = event
                .pointer("/choices/0/delta/content")
                .and_then(Value::as_str)
            {
                accumulator.push(delta)?;
            }
        }
    }

    for data in sse.flush() {
        if data.trim() == "[DONE]" {
            continue;
        }

        let event = serde_json::from_str::<Value>(&data).map_err(|error| error.to_string())?;
        if let Some(delta) = event
            .pointer("/choices/0/delta/content")
            .and_then(Value::as_str)
        {
            accumulator.push(delta)?;
        }
    }

    if accumulator.text.is_empty() {
        return Err("LLM response did not contain text output".to_string());
    }

    Ok(StandardLlmResponse {
        text: accumulator.text(),
        usage: None,
    })
}

async fn stream_openai_responses_completion<EmitFn>(
    config: &LlmConfig,
    input: &str,
    accumulator: &mut StreamTextAccumulator<'_, EmitFn>,
) -> Result<StandardLlmResponse, String>
where
    EmitFn: FnMut(&str, &str) -> Result<(), String>,
{
    let base_url = LlmApiUrl::parse(&config.base_url)?;
    let url = base_url.join(config.api_path.as_deref().unwrap_or("/v1/responses"))?;
    let client = url.client()?;
    let response = client
        .post(url.reqwest_url())
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream")
        .header("Authorization", format!("Bearer {}", config.api_key))
        .json(&json!({
            "model": config.model,
            "input": input,
            "temperature": config.temperature.unwrap_or(0.7),
            "stream": true,
        }))
        .send()
        .await
        .map_err(|error| error.to_string())?;

    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(format!("LLM API Error: {} {}", status, text));
    }

    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();

    if !content_type.contains("text/event-stream") {
        let body = response.text().await.map_err(|error| error.to_string())?;
        let json = serde_json::from_str::<Value>(&body).map_err(|error| error.to_string())?;
        return Ok(StandardLlmResponse {
            text: extract_text_from_json_response(&json)?,
            usage: extract_usage_from_json_response(&json),
        });
    }

    let mut sse = SseEventBuffer::default();
    let mut byte_stream = response.bytes_stream();

    while let Some(item) = byte_stream.next().await {
        let bytes = item.map_err(|error| error.to_string())?;
        let chunk = String::from_utf8_lossy(&bytes);
        for data in sse.process(&chunk) {
            if data.trim() == "[DONE]" {
                continue;
            }

            let event = serde_json::from_str::<Value>(&data).map_err(|error| error.to_string())?;
            if let Some(event_type) = event.get("type").and_then(Value::as_str) {
                if event_type.contains("output_text.delta") || event_type.contains("refusal.delta")
                {
                    if let Some(delta) = event.get("delta").and_then(Value::as_str) {
                        accumulator.push(delta)?;
                    }
                }
            }
        }
    }

    for data in sse.flush() {
        if data.trim() == "[DONE]" {
            continue;
        }

        let event = serde_json::from_str::<Value>(&data).map_err(|error| error.to_string())?;
        if let Some(event_type) = event.get("type").and_then(Value::as_str) {
            if event_type.contains("output_text.delta") || event_type.contains("refusal.delta") {
                if let Some(delta) = event.get("delta").and_then(Value::as_str) {
                    accumulator.push(delta)?;
                }
            }
        }
    }

    if accumulator.text.is_empty() {
        return Err("LLM response did not contain text output".to_string());
    }

    Ok(StandardLlmResponse {
        text: accumulator.text(),
        usage: None,
    })
}

pub(crate) async fn try_stream_text<EmitFn>(
    request: &LlmGenerateRequest,
    accumulator: &mut StreamTextAccumulator<'_, EmitFn>,
) -> Result<Option<StandardLlmResponse>, String>
where
    EmitFn: FnMut(&str, &str) -> Result<(), String>,
{
    let input = build_standard_input(&StandardLlmRequest {
        messages: vec![StandardMessage {
            role: MessageRole::User,
            content: request.input.clone(),
        }],
        temperature: request.config.temperature.unwrap_or(0.7),
        max_tokens: None,
    });

    let response = match request.config.provider {
        LlmProvider::Anthropic => {
            let client = anthropic::Client::builder()
                .api_key(&request.config.api_key)
                .base_url(&request.config.base_url)
                .build()
                .map_err(|error| error.to_string())?;
            Some(
                stream_rig_completion_model(
                    client.completion_model(&request.config.model),
                    &input,
                    request.config.temperature.unwrap_or(0.7),
                    accumulator,
                )
                .await?,
            )
        }
        LlmProvider::Gemini => {
            let client = gemini::Client::builder()
                .api_key(&request.config.api_key)
                .base_url(clean_gemini_base_url(&request.config.base_url))
                .build()
                .map_err(|error| error.to_string())?;
            Some(
                stream_rig_completion_model(
                    client.completion_model(&request.config.model),
                    &input,
                    request.config.temperature.unwrap_or(0.7),
                    accumulator,
                )
                .await?,
            )
        }
        LlmProvider::Ollama => {
            let client = ollama::Client::builder()
                .api_key(Nothing)
                .base_url(request.config.base_url.trim_end_matches("/v1"))
                .build()
                .map_err(|error| error.to_string())?;
            Some(
                stream_rig_completion_model(
                    client.completion_model(&request.config.model),
                    &input,
                    request.config.temperature.unwrap_or(0.7),
                    accumulator,
                )
                .await?,
            )
        }
        LlmProvider::OpenAiResponses => {
            Some(stream_openai_responses_completion(&request.config, &input, accumulator).await?)
        }
        LlmProvider::GoogleTranslate | LlmProvider::GoogleTranslateFree => None,
        _ => Some(stream_openai_chat_completion(&request.config, &input, accumulator).await?),
    };

    Ok(response)
}

pub(crate) async fn generate_with_optional_streaming<EmitFn>(
    request: LlmGenerateRequest,
    emit_delta: &mut EmitFn,
) -> Result<StandardLlmResponse, String>
where
    EmitFn: FnMut(&str, &str) -> Result<(), String>,
{
    let mut accumulator = StreamTextAccumulator::new(emit_delta);
    let stream_result = try_stream_text(&request, &mut accumulator).await;
    let emitted_any = accumulator.emitted_any;
    drop(accumulator);

    match stream_result {
        Ok(Some(response)) => Ok(response),
        Ok(None) => generate_with_rig(request).await,
        Err(error) if !emitted_any => {
            warn!(
                "[LLM] streaming unavailable or failed before first token, falling back to buffered generate: provider={:?} error={}",
                request.config.provider, error
            );
            generate_with_rig(request).await
        }
        Err(error) => Err(error),
    }
}

pub(crate) async fn generate_with_openai_chat_api(
    url: &LlmApiUrl,
    api_key: &str,
    model: &str,
    input: &str,
    temperature: Option<f32>,
    extra_headers: Vec<(&str, String)>,
) -> Result<StandardLlmResponse, String> {
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
    Ok(StandardLlmResponse {
        text: extract_text_from_json_response(&response)?,
        usage: extract_usage_from_json_response(&response),
    })
}

pub(crate) async fn generate_with_openai_responses_api(
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
    )
    .await?;

    Ok(StandardLlmResponse {
        text: extract_text_from_json_response(&response)?,
        usage: extract_usage_from_json_response(&response),
    })
}

pub(crate) async fn generate_with_azure_openai(
    base_url: &str,
    api_key: &str,
    deployment: &str,
    input: &str,
    temperature: Option<f32>,
    api_version: Option<&str>,
) -> Result<StandardLlmResponse, String> {
    let version = api_version.unwrap_or("2024-10-21");
    let deployment = deployment.trim();
    let base_url = LlmApiUrl::parse(base_url)?;
    let endpoint = base_url
        .join(&format!(
            "/openai/deployments/{}/chat/completions",
            deployment
        ))?
        .with_query(&format!("api-version={}", version))?;

    let payload = json!({
        "messages": [
            {
                "role": "user",
                "content": input,
            }
        ],
        "temperature": temperature.unwrap_or(0.7),
    });

    let response =
        post_json_request(&endpoint, vec![("api-key", api_key.to_string())], payload).await?;

    Ok(StandardLlmResponse {
        text: extract_text_from_json_response(&response)?,
        usage: extract_usage_from_json_response(&response),
    })
}

pub(crate) async fn generate_with_openai_custom_path(
    base_url: &str,
    api_key: &str,
    model: &str,
    input: &str,
    temperature: Option<f32>,
    api_path: Option<&str>,
) -> Result<StandardLlmResponse, String> {
    let base_url = LlmApiUrl::parse(base_url)?;
    let url = base_url.join(api_path.unwrap_or("/v1/chat/completions"))?;
    generate_with_openai_chat_api(&url, api_key, model, input, temperature, vec![]).await
}

pub(crate) async fn generate_with_perplexity(
    api_key: &str,
    model: &str,
    input: &str,
    temperature: Option<f32>,
) -> Result<StandardLlmResponse, String> {
    let url = LlmApiUrl::parse("https://api.perplexity.ai/chat/completions")?;
    generate_with_openai_chat_api(&url, api_key, model, input, temperature, vec![]).await
}

pub(crate) async fn generate_with_rig(
    request: LlmGenerateRequest,
) -> Result<StandardLlmResponse, String> {
    let adapter = AdapterFactory::create(request.config.provider);
    let std_req = StandardLlmRequest {
        messages: vec![StandardMessage {
            role: MessageRole::User,
            content: request.input,
        }],
        temperature: request.config.temperature.unwrap_or(0.7),
        max_tokens: None,
    };

    let client = Client::new();
    let response = adapter.generate(&client, &std_req, &request.config).await?;

    Ok(response)
}
