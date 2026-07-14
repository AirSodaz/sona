use futures_util::StreamExt;
use reqwest::header::CONTENT_TYPE;
use rig_core::client::{CompletionClient, Nothing};
use rig_core::completion::{CompletionModel, GetTokenUsage};
use rig_core::providers::{anthropic, copilot, gemini, ollama};
use rig_core::streaming::StreamedAssistantContent;
use serde_json::Value;
use sona_core::llm::provider_protocol::{
    StandardLlmResponse, clean_gemini_base_url, extract_text_from_json_response,
    extract_usage_from_json_response, join_url,
};
use sona_core::llm::requests::{LlmConfig, LlmGenerateRequest};
use sona_core::llm::runtime::{LlmCompletionRequest, LlmPromptCachePolicy};
use sona_core::llm::streaming_protocol::{
    OpenAiStreamUrlConfig, SseEventBuffer, StreamTextAccumulator, build_openai_stream_url,
};
use sona_core::llm::tasks::LlmProviderStrategy;
use sona_core::llm::usage::TokenUsage;
use sona_core::ports::llm::LlmPortError;

use crate::anthropic::build_anthropic_payload_for_request;
use crate::completion::{
    build_rig_completion_request, extract_text_response, token_usage_from_rig_usage,
};
use crate::gemini::{
    build_gemini_generate_content_request_parts_for_reqwest, build_gemini_payload_for_request,
    extract_gemini_usage, extract_gemini_visible_text,
};
use crate::openai_compatible::build_openai_chat_payload_for_request;
use crate::responses::build_openai_responses_payload;
use crate::transport::{
    LlmApiUrl, classify_llm_port_error, http_status_port_error, reqwest_port_error,
};

#[derive(Debug)]
struct StreamingError(LlmPortError);

impl From<String> for StreamingError {
    fn from(error: String) -> Self {
        Self(classify_llm_port_error(error))
    }
}

impl From<LlmPortError> for StreamingError {
    fn from(error: LlmPortError) -> Self {
        Self(error)
    }
}

impl From<reqwest::Error> for StreamingError {
    fn from(error: reqwest::Error) -> Self {
        Self(reqwest_port_error(error))
    }
}

impl From<StreamingError> for LlmPortError {
    fn from(error: StreamingError) -> Self {
        error.0
    }
}

async fn stream_rig_completion_model<M, EmitFn>(
    model: M,
    request: &LlmCompletionRequest,
    accumulator: &mut StreamTextAccumulator<'_, EmitFn>,
) -> Result<StandardLlmResponse, StreamingError>
where
    M: CompletionModel,
    M::StreamingResponse: Clone + Unpin + GetTokenUsage + 'static,
    EmitFn: FnMut(&str, &str) -> Result<(), String> + Send + ?Sized,
{
    let mut stream = build_rig_completion_request(model, request)?
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

    let text = if accumulator.is_empty() {
        extract_text_response(&stream.choice)?
    } else {
        accumulator.text()
    };

    Ok(StandardLlmResponse {
        text,
        usage: stream
            .response
            .as_ref()
            .and_then(|response| token_usage_from_rig_usage(Some(response.token_usage()))),
    })
}

fn openai_stream_url_config(config: &LlmConfig) -> OpenAiStreamUrlConfig<'_> {
    OpenAiStreamUrlConfig {
        strategy: config.strategy,
        base_url: &config.base_url,
        model: &config.model,
        api_path: config.api_path.as_deref(),
        api_version: config.api_version.as_deref(),
    }
}

async fn stream_openai_chat_completion<EmitFn>(
    request: &LlmCompletionRequest,
    accumulator: &mut StreamTextAccumulator<'_, EmitFn>,
) -> Result<StandardLlmResponse, StreamingError>
where
    EmitFn: FnMut(&str, &str) -> Result<(), String> + Send + ?Sized,
{
    let config = &request.config;
    let mut last_usage: Option<TokenUsage> = None;
    let url = LlmApiUrl::parse(&build_openai_stream_url(openai_stream_url_config(config)))?;
    let client = url.client(config.timeout_seconds)?;
    let payload = build_openai_chat_payload_for_request(
        request,
        true,
        config.strategy == LlmProviderStrategy::AzureOpenAi,
    )?;

    let mut request = client
        .post(url.reqwest_url())
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream");

    if config.strategy == LlmProviderStrategy::AzureOpenAi {
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
        let headers = response.headers().clone();
        let text = response.text().await.unwrap_or_default();
        return Err(http_status_port_error(status, &headers, text).into());
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
            if let Some(usage) = extract_usage_from_json_response(&event) {
                last_usage = Some(usage);
            }
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
        if let Some(usage) = extract_usage_from_json_response(&event) {
            last_usage = Some(usage);
        }
        if let Some(delta) = event
            .pointer("/choices/0/delta/content")
            .and_then(Value::as_str)
        {
            accumulator.push(delta)?;
        }
    }

    if accumulator.is_empty() {
        return Err("LLM response did not contain text output"
            .to_string()
            .into());
    }

    Ok(StandardLlmResponse {
        text: accumulator.text(),
        usage: last_usage,
    })
}

async fn stream_anthropic_custom_completion<EmitFn>(
    request: &LlmCompletionRequest,
    accumulator: &mut StreamTextAccumulator<'_, EmitFn>,
) -> Result<StandardLlmResponse, StreamingError>
where
    EmitFn: FnMut(&str, &str) -> Result<(), String> + Send + ?Sized,
{
    let config = &request.config;
    let mut usage = TokenUsage::default();
    let url = LlmApiUrl::parse(&join_url(&config.base_url, "/v1/messages"))?;
    let client = url.client(config.timeout_seconds)?;
    let payload = build_anthropic_payload_for_request(request, true)?;

    let response = client
        .post(url.reqwest_url())
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream")
        .header("x-api-key", config.api_key.clone())
        .header("anthropic-version", "2023-06-01")
        .json(&payload)
        .send()
        .await
        .map_err(|error| error.to_string())?;

    let status = response.status();
    if !status.is_success() {
        let headers = response.headers().clone();
        let text = response.text().await.unwrap_or_default();
        return Err(http_status_port_error(status, &headers, text).into());
    }

    let mut sse = SseEventBuffer::default();
    let mut byte_stream = response.bytes_stream();

    while let Some(item) = byte_stream.next().await {
        let bytes = item.map_err(|error| error.to_string())?;
        let chunk = String::from_utf8_lossy(&bytes);
        for data in sse.process(&chunk) {
            if let Ok(event) = serde_json::from_str::<Value>(&data) {
                update_anthropic_stream_usage(&mut usage, &event);
                if let Some(delta) = event.pointer("/delta/text").and_then(Value::as_str) {
                    accumulator.push(delta)?;
                }
            }
        }
    }

    for data in sse.flush() {
        if let Ok(event) = serde_json::from_str::<Value>(&data) {
            update_anthropic_stream_usage(&mut usage, &event);
            if let Some(delta) = event.pointer("/delta/text").and_then(Value::as_str) {
                accumulator.push(delta)?;
            }
        }
    }

    Ok(StandardLlmResponse {
        text: accumulator.text(),
        usage: finish_anthropic_stream_usage(usage),
    })
}

async fn stream_gemini_custom_completion<EmitFn>(
    request: &LlmCompletionRequest,
    accumulator: &mut StreamTextAccumulator<'_, EmitFn>,
) -> Result<StandardLlmResponse, StreamingError>
where
    EmitFn: FnMut(&str, &str) -> Result<(), String> + Send + ?Sized,
{
    let config = &request.config;
    let mut last_usage: Option<TokenUsage> = None;
    let request_parts = build_gemini_generate_content_request_parts_for_reqwest(
        &config.base_url,
        &config.model,
        &config.api_key,
        true,
    )?;
    let client = request_parts.url.client(config.timeout_seconds)?;

    let payload = build_gemini_payload_for_request(request)?;

    let mut request = client
        .post(request_parts.url.reqwest_url())
        .header("Content-Type", "application/json");

    for (key, value) in request_parts.headers {
        request = request.header(key, value);
    }

    let response = request
        .json(&payload)
        .send()
        .await
        .map_err(|error| error.to_string())?;

    let status = response.status();
    if !status.is_success() {
        let headers = response.headers().clone();
        let text = response.text().await.unwrap_or_default();
        return Err(http_status_port_error(status, &headers, text).into());
    }

    let mut sse = SseEventBuffer::default();
    let mut byte_stream = response.bytes_stream();

    while let Some(item) = byte_stream.next().await {
        let bytes = item.map_err(|error| error.to_string())?;
        let chunk = String::from_utf8_lossy(&bytes);
        for data in sse.process(&chunk) {
            if let Ok(event) = serde_json::from_str::<Value>(&data) {
                if let Some(usage) = event.get("usageMetadata").and_then(extract_gemini_usage) {
                    last_usage = Some(usage);
                }
                if let Some(text) = extract_gemini_visible_text(&event) {
                    accumulator.push(&text)?;
                }
            }
        }
    }

    for data in sse.flush() {
        if let Ok(event) = serde_json::from_str::<Value>(&data) {
            if let Some(usage) = event.get("usageMetadata").and_then(extract_gemini_usage) {
                last_usage = Some(usage);
            }
            if let Some(text) = extract_gemini_visible_text(&event) {
                accumulator.push(&text)?;
            }
        }
    }

    Ok(StandardLlmResponse {
        text: accumulator.text(),
        usage: last_usage,
    })
}

async fn stream_openai_responses_completion<EmitFn>(
    request: &LlmCompletionRequest,
    accumulator: &mut StreamTextAccumulator<'_, EmitFn>,
) -> Result<StandardLlmResponse, StreamingError>
where
    EmitFn: FnMut(&str, &str) -> Result<(), String> + Send + ?Sized,
{
    let config = &request.config;
    let base_url = LlmApiUrl::parse(&config.base_url)?;
    let url = base_url.join(config.api_path.as_deref().unwrap_or("/v1/responses"))?;
    let client = url.client(config.timeout_seconds)?;
    let response = client
        .post(url.reqwest_url())
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream")
        .header("Authorization", format!("Bearer {}", config.api_key))
        .json(&build_openai_responses_payload(request, true))
        .send()
        .await
        .map_err(|error| error.to_string())?;

    let status = response.status();
    if !status.is_success() {
        let headers = response.headers().clone();
        let text = response.text().await.unwrap_or_default();
        return Err(http_status_port_error(status, &headers, text).into());
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
    let mut last_usage = None;
    let mut byte_stream = response.bytes_stream();

    while let Some(item) = byte_stream.next().await {
        let bytes = item.map_err(|error| error.to_string())?;
        let chunk = String::from_utf8_lossy(&bytes);
        for data in sse.process(&chunk) {
            if data.trim() == "[DONE]" {
                continue;
            }

            let event = serde_json::from_str::<Value>(&data).map_err(|error| error.to_string())?;
            if let Some(usage) = extract_openai_responses_stream_usage(&event) {
                last_usage = Some(usage);
            }
            if let Some(event_type) = event.get("type").and_then(Value::as_str)
                && (event_type.contains("output_text.delta")
                    || event_type.contains("refusal.delta"))
                && let Some(delta) = event.get("delta").and_then(Value::as_str)
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
        if let Some(usage) = extract_openai_responses_stream_usage(&event) {
            last_usage = Some(usage);
        }
        if let Some(event_type) = event.get("type").and_then(Value::as_str)
            && (event_type.contains("output_text.delta") || event_type.contains("refusal.delta"))
            && let Some(delta) = event.get("delta").and_then(Value::as_str)
        {
            accumulator.push(delta)?;
        }
    }

    if accumulator.is_empty() {
        return Err("LLM response did not contain text output"
            .to_string()
            .into());
    }

    Ok(StandardLlmResponse {
        text: accumulator.text(),
        usage: last_usage,
    })
}

fn update_anthropic_stream_usage(usage: &mut TokenUsage, event: &Value) {
    let source = event
        .pointer("/message/usage")
        .or_else(|| event.get("usage"));
    let Some(source) = source else {
        return;
    };

    if let Some(value) = source.get("input_tokens").and_then(Value::as_u64) {
        usage.prompt_tokens = value;
    }
    if let Some(value) = source.get("output_tokens").and_then(Value::as_u64) {
        usage.completion_tokens = value;
    }
    if let Some(value) = source
        .get("cache_read_input_tokens")
        .and_then(Value::as_u64)
    {
        usage.cached_input_tokens = value;
    }
    if let Some(value) = source
        .get("cache_creation_input_tokens")
        .and_then(Value::as_u64)
    {
        usage.cache_creation_input_tokens = value;
    }
}

fn finish_anthropic_stream_usage(mut usage: TokenUsage) -> Option<TokenUsage> {
    usage.total_tokens = usage
        .prompt_tokens
        .saturating_add(usage.completion_tokens)
        .saturating_add(usage.cached_input_tokens)
        .saturating_add(usage.cache_creation_input_tokens);
    (usage.total_tokens > 0).then_some(usage)
}

pub fn extract_anthropic_stream_usage(events: &[Value]) -> Option<TokenUsage> {
    let mut usage = TokenUsage::default();
    for event in events {
        update_anthropic_stream_usage(&mut usage, event);
    }
    finish_anthropic_stream_usage(usage)
}

pub fn extract_openai_responses_stream_usage(event: &Value) -> Option<TokenUsage> {
    event
        .get("response")
        .and_then(extract_usage_from_json_response)
        .or_else(|| extract_usage_from_json_response(event))
}

async fn try_stream_completion_with_provider_inner<EmitFn>(
    request: &LlmCompletionRequest,
    accumulator: &mut StreamTextAccumulator<'_, EmitFn>,
) -> Result<Option<StandardLlmResponse>, StreamingError>
where
    EmitFn: FnMut(&str, &str) -> Result<(), String> + Send + ?Sized,
{
    let response = match request.config.strategy {
        LlmProviderStrategy::Anthropic => {
            if request.effective_reasoning_enabled() {
                Some(stream_anthropic_custom_completion(request, accumulator).await?)
            } else {
                let reqwest_client = LlmApiUrl::parse(&request.config.base_url)?
                    .client(request.config.timeout_seconds)?;
                let client = anthropic::Client::builder()
                    .api_key(&request.config.api_key)
                    .base_url(&request.config.base_url)
                    .http_client(reqwest_client)
                    .build()
                    .map_err(|error| error.to_string())?;
                let mut model = client.completion_model(&request.config.model);
                if request.options.prompt_cache == LlmPromptCachePolicy::Automatic {
                    model = model.with_automatic_caching();
                }
                Some(stream_rig_completion_model(model, request, accumulator).await?)
            }
        }
        LlmProviderStrategy::Gemini => {
            if request.effective_reasoning_enabled() {
                Some(stream_gemini_custom_completion(request, accumulator).await?)
            } else {
                let reqwest_client = LlmApiUrl::parse(&request.config.base_url)?
                    .client(request.config.timeout_seconds)?;
                let client = gemini::Client::builder()
                    .api_key(&request.config.api_key)
                    .base_url(clean_gemini_base_url(&request.config.base_url))
                    .http_client(reqwest_client)
                    .build()
                    .map_err(|error| error.to_string())?;
                Some(
                    stream_rig_completion_model(
                        client.completion_model(&request.config.model),
                        request,
                        accumulator,
                    )
                    .await?,
                )
            }
        }
        LlmProviderStrategy::Ollama => {
            let reqwest_client = LlmApiUrl::parse(&request.config.base_url)?
                .client(request.config.timeout_seconds)?;
            let client = ollama::Client::builder()
                .api_key(Nothing)
                .base_url(request.config.base_url.trim_end_matches("/v1"))
                .http_client(reqwest_client)
                .build()
                .map_err(|error| error.to_string())?;
            Some(
                stream_rig_completion_model(
                    client.completion_model(&request.config.model),
                    request,
                    accumulator,
                )
                .await?,
            )
        }
        LlmProviderStrategy::OpenAiResponses => {
            Some(stream_openai_responses_completion(request, accumulator).await?)
        }
        LlmProviderStrategy::Copilot => {
            let reqwest_client = LlmApiUrl::parse(&request.config.base_url)?
                .client(request.config.timeout_seconds)?;
            let client = copilot::Client::builder()
                .api_key(&request.config.api_key)
                .base_url(&request.config.base_url)
                .http_client(reqwest_client)
                .build()
                .map_err(|error| error.to_string())?;
            Some(
                stream_rig_completion_model(
                    client.completion_model(&request.config.model),
                    request,
                    accumulator,
                )
                .await?,
            )
        }
        LlmProviderStrategy::GoogleTranslate | LlmProviderStrategy::GoogleTranslateFree => None,
        _ => Some(stream_openai_chat_completion(request, accumulator).await?),
    };

    Ok(response)
}

pub async fn try_stream_completion_with_provider<EmitFn>(
    request: &LlmCompletionRequest,
    accumulator: &mut StreamTextAccumulator<'_, EmitFn>,
) -> Result<Option<StandardLlmResponse>, LlmPortError>
where
    EmitFn: FnMut(&str, &str) -> Result<(), String> + Send + ?Sized,
{
    try_stream_completion_with_provider_inner(request, accumulator)
        .await
        .map_err(Into::into)
}

pub async fn try_stream_text_with_provider<EmitFn>(
    request: &LlmGenerateRequest,
    accumulator: &mut StreamTextAccumulator<'_, EmitFn>,
) -> Result<Option<StandardLlmResponse>, LlmPortError>
where
    EmitFn: FnMut(&str, &str) -> Result<(), String> + Send + ?Sized,
{
    let completion_request = request.clone().into();
    try_stream_completion_with_provider(&completion_request, accumulator).await
}
