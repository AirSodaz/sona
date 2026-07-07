use super::network::LlmApiUrl;
use super::*;
use futures_util::StreamExt;
use log::warn;
use reqwest::header::CONTENT_TYPE;
use rig_core::client::{CompletionClient, Nothing};
use rig_core::completion::{CompletionModel, GetTokenUsage};
use rig_core::providers::{anthropic, copilot, gemini, ollama};
use rig_core::streaming::StreamedAssistantContent;
use serde_json::{Value, json};

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

fn openai_chat_payload_config(config: &LlmConfig) -> OpenAiChatPayloadConfig<'_> {
    OpenAiChatPayloadConfig {
        strategy: config.strategy,
        model: &config.model,
        temperature: config.temperature,
        reasoning_enabled: config.reasoning_enabled.unwrap_or(false),
        reasoning_level: config.reasoning_level.as_deref(),
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
    let mut last_usage: Option<TokenUsage> = None;
    let url = LlmApiUrl::parse(&build_openai_stream_url(openai_stream_url_config(config)))?;
    let client = url.client(config.timeout_seconds)?;
    let payload = build_openai_chat_payload(openai_chat_payload_config(config), input, true);

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
        return Err("LLM response did not contain text output".to_string());
    }

    Ok(StandardLlmResponse {
        text: accumulator.text(),
        usage: last_usage,
    })
}

async fn stream_anthropic_custom_completion<EmitFn>(
    config: &LlmConfig,
    input: &str,
    accumulator: &mut StreamTextAccumulator<'_, EmitFn>,
) -> Result<StandardLlmResponse, String>
where
    EmitFn: FnMut(&str, &str) -> Result<(), String>,
{
    let mut input_tokens = 0;
    let mut output_tokens = 0;
    let url = LlmApiUrl::parse(&join_url(&config.base_url, "/v1/messages"))?;
    let client = url.client(config.timeout_seconds)?;

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
        "stream": true,
    });

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
        let text = response.text().await.unwrap_or_default();
        return Err(format!("Anthropic API Error: {} {}", status, text));
    }

    let mut sse = SseEventBuffer::default();
    let mut byte_stream = response.bytes_stream();

    while let Some(item) = byte_stream.next().await {
        let bytes = item.map_err(|error| error.to_string())?;
        let chunk = String::from_utf8_lossy(&bytes);
        for data in sse.process(&chunk) {
            if let Ok(event) = serde_json::from_str::<Value>(&data) {
                if let Some(t) = event.get("type").and_then(Value::as_str) {
                    if t == "message_start" {
                        if let Some(tokens) = event
                            .pointer("/message/usage/input_tokens")
                            .and_then(Value::as_u64)
                        {
                            input_tokens = tokens;
                        }
                    } else if t == "message_delta"
                        && let Some(tokens) = event
                            .pointer("/usage/output_tokens")
                            .and_then(Value::as_u64)
                    {
                        output_tokens = tokens;
                    }
                }
                if let Some(delta) = event.pointer("/delta/text").and_then(Value::as_str) {
                    accumulator.push(delta)?;
                }
            }
        }
    }

    for data in sse.flush() {
        if let Ok(event) = serde_json::from_str::<Value>(&data) {
            if let Some(t) = event.get("type").and_then(Value::as_str) {
                if t == "message_start" {
                    if let Some(tokens) = event
                        .pointer("/message/usage/input_tokens")
                        .and_then(Value::as_u64)
                    {
                        input_tokens = tokens;
                    }
                } else if t == "message_delta"
                    && let Some(tokens) = event
                        .pointer("/usage/output_tokens")
                        .and_then(Value::as_u64)
                {
                    output_tokens = tokens;
                }
            }
            if let Some(delta) = event.pointer("/delta/text").and_then(Value::as_str) {
                accumulator.push(delta)?;
            }
        }
    }

    let last_usage = if input_tokens > 0 || output_tokens > 0 {
        Some(TokenUsage {
            prompt_tokens: input_tokens as u32,
            completion_tokens: output_tokens as u32,
            total_tokens: (input_tokens + output_tokens) as u32,
        })
    } else {
        None
    };

    Ok(StandardLlmResponse {
        text: accumulator.text(),
        usage: last_usage,
    })
}

async fn stream_gemini_custom_completion<EmitFn>(
    config: &LlmConfig,
    input: &str,
    accumulator: &mut StreamTextAccumulator<'_, EmitFn>,
) -> Result<StandardLlmResponse, String>
where
    EmitFn: FnMut(&str, &str) -> Result<(), String>,
{
    let mut last_usage: Option<TokenUsage> = None;
    let is_gemini_2_5 = config.model.contains("gemini-2.5");
    let request_parts = build_gemini_generate_content_request_parts(
        &config.base_url,
        &config.model,
        &config.api_key,
        true,
    )?;
    let client = request_parts.url.client(config.timeout_seconds)?;

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
            "temperature": config.temperature.unwrap_or(0.7),
            "thinkingConfig": thinking_config
        }
    });

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
        let text = response.text().await.unwrap_or_default();
        return Err(format!("Gemini API Error: {} {}", status, text));
    }

    let mut sse = SseEventBuffer::default();
    let mut byte_stream = response.bytes_stream();

    while let Some(item) = byte_stream.next().await {
        let bytes = item.map_err(|error| error.to_string())?;
        let chunk = String::from_utf8_lossy(&bytes);
        for data in sse.process(&chunk) {
            if let Ok(event) = serde_json::from_str::<Value>(&data) {
                if let Some(usage) = event.get("usageMetadata")
                    && let (Some(prompt), Some(candidates), Some(total)) = (
                        usage.get("promptTokenCount").and_then(Value::as_u64),
                        usage.get("candidatesTokenCount").and_then(Value::as_u64),
                        usage.get("totalTokenCount").and_then(Value::as_u64),
                    )
                {
                    last_usage = Some(TokenUsage {
                        prompt_tokens: prompt as u32,
                        completion_tokens: candidates as u32,
                        total_tokens: total as u32,
                    });
                }
                if let Some(text) = event
                    .pointer("/candidates/0/content/parts/0/text")
                    .and_then(Value::as_str)
                {
                    accumulator.push(text)?;
                }
            }
        }
    }

    for data in sse.flush() {
        if let Ok(event) = serde_json::from_str::<Value>(&data) {
            if let Some(usage) = event.get("usageMetadata")
                && let (Some(prompt), Some(candidates), Some(total)) = (
                    usage.get("promptTokenCount").and_then(Value::as_u64),
                    usage.get("candidatesTokenCount").and_then(Value::as_u64),
                    usage.get("totalTokenCount").and_then(Value::as_u64),
                )
            {
                last_usage = Some(TokenUsage {
                    prompt_tokens: prompt as u32,
                    completion_tokens: candidates as u32,
                    total_tokens: total as u32,
                });
            }
            if let Some(text) = event
                .pointer("/candidates/0/content/parts/0/text")
                .and_then(Value::as_str)
            {
                accumulator.push(text)?;
            }
        }
    }

    Ok(StandardLlmResponse {
        text: accumulator.text(),
        usage: last_usage,
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
    let client = url.client(config.timeout_seconds)?;
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
        if let Some(event_type) = event.get("type").and_then(Value::as_str)
            && (event_type.contains("output_text.delta") || event_type.contains("refusal.delta"))
            && let Some(delta) = event.get("delta").and_then(Value::as_str)
        {
            accumulator.push(delta)?;
        }
    }

    if accumulator.is_empty() {
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
    });

    let response = match request.config.strategy {
        LlmProviderStrategy::Anthropic => {
            if request.config.reasoning_enabled.unwrap_or(false) {
                Some(
                    stream_anthropic_custom_completion(&request.config, &input, accumulator)
                        .await?,
                )
            } else {
                let reqwest_client = LlmApiUrl::parse(&request.config.base_url)?
                    .client(request.config.timeout_seconds)?;
                let client = anthropic::Client::builder()
                    .api_key(&request.config.api_key)
                    .base_url(&request.config.base_url)
                    .http_client(reqwest_client)
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
        }
        LlmProviderStrategy::Gemini => {
            if request.config.reasoning_enabled.unwrap_or(false) {
                Some(stream_gemini_custom_completion(&request.config, &input, accumulator).await?)
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
                        &input,
                        request.config.temperature.unwrap_or(0.7),
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
                    &input,
                    request.config.temperature.unwrap_or(0.7),
                    accumulator,
                )
                .await?,
            )
        }
        LlmProviderStrategy::OpenAiResponses => {
            Some(stream_openai_responses_completion(&request.config, &input, accumulator).await?)
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
                    &input,
                    request.config.temperature.unwrap_or(0.7),
                    accumulator,
                )
                .await?,
            )
        }
        LlmProviderStrategy::GoogleTranslate | LlmProviderStrategy::GoogleTranslateFree => None,
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
    let emitted_any = accumulator.emitted_any();
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

pub(crate) async fn generate_with_rig(
    request: LlmGenerateRequest,
) -> Result<StandardLlmResponse, String> {
    generate_text_with_provider(request).await
}
