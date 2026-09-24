use futures_util::StreamExt;
use serde_json::Value;
use sona_core::llm::provider_protocol::{StandardLlmResponse, extract_usage_from_json_response};
use sona_core::llm::runtime::{LlmCompletionRequest, LlmStreamDelta, LlmStreamDeltaKind};
use sona_core::llm::streaming_protocol::{DualStreamAccumulator, SseEventBuffer};
use sona_core::llm::tasks::LlmProviderStrategy;
use sona_core::llm::usage::TokenUsage;
use sona_core::ports::llm::{LlmPortError, LlmPortErrorKind};

use crate::demuxer::ThoughtStreamDemuxer;
use crate::gemini::extract_gemini_usage;
use crate::native_completion::resolve_strategy_url_and_headers;
use crate::streaming::{finish_anthropic_stream_usage, update_anthropic_stream_usage};
use crate::transport::{http_status_port_error, reqwest_port_error};

pub async fn execute_native_stream<EmitFn>(
    request: &LlmCompletionRequest,
    accumulator: &mut DualStreamAccumulator<'_, EmitFn, LlmPortError>,
) -> Result<StandardLlmResponse, LlmPortError>
where
    EmitFn: FnMut(LlmStreamDelta) -> Result<(), LlmPortError> + Send + ?Sized,
{
    let strategy = request.config.strategy;
    if matches!(
        strategy,
        LlmProviderStrategy::GoogleTranslate | LlmProviderStrategy::GoogleTranslateFree
    ) {
        return Err(LlmPortError::new(
            LlmPortErrorKind::Unsupported,
            "Google Translate does not support streaming completion",
        ));
    }

    let (url, headers) = resolve_strategy_url_and_headers(request, true)?;
    let is_azure = strategy == LlmProviderStrategy::AzureOpenAi;

    let payload = match strategy {
        LlmProviderStrategy::Anthropic => {
            crate::anthropic::build_anthropic_payload_for_request(request, true)?
        }
        LlmProviderStrategy::Gemini => crate::gemini::build_gemini_payload_for_request(request)?,
        LlmProviderStrategy::OpenAiResponses => {
            crate::responses::build_openai_responses_payload(request, true)
        }
        _ => crate::openai_compatible::build_openai_chat_payload_for_request(
            request, true, is_azure,
        )?,
    };

    let client = url.client(request.config.timeout_seconds)?;
    let mut req_builder = client
        .post(url.reqwest_url())
        .header("Content-Type", "application/json");

    for (k, v) in &headers {
        req_builder = req_builder.header(*k, v);
    }

    let response = req_builder
        .json(&payload)
        .send()
        .await
        .map_err(reqwest_port_error)?;

    let status = response.status();
    let response = if status == reqwest::StatusCode::BAD_REQUEST {
        let headers_clone = response.headers().clone();
        let text = response.text().await.map_err(reqwest_port_error)?;
        let lower_err = text.to_ascii_lowercase();
        let mut retry_needed = false;
        let mut cleaned_payload = payload.clone();

        if lower_err.contains("stream_options") && cleaned_payload.get("stream_options").is_some() {
            cleaned_payload
                .as_object_mut()
                .and_then(|p| p.remove("stream_options"));
            retry_needed = true;
        }
        if lower_err.contains("reasoning_effort")
            && cleaned_payload.get("reasoning_effort").is_some()
        {
            cleaned_payload
                .as_object_mut()
                .and_then(|p| p.remove("reasoning_effort"));
            retry_needed = true;
        }
        if lower_err.contains("temperature") && cleaned_payload.get("temperature").is_some() {
            cleaned_payload
                .as_object_mut()
                .and_then(|p| p.remove("temperature"));
            retry_needed = true;
        }

        if retry_needed {
            let mut retry_builder = client
                .post(url.reqwest_url())
                .header("Content-Type", "application/json");
            for (k, v) in &headers {
                retry_builder = retry_builder.header(*k, v);
            }
            let retry_resp = retry_builder
                .json(&cleaned_payload)
                .send()
                .await
                .map_err(reqwest_port_error)?;
            let retry_status = retry_resp.status();
            if !retry_status.is_success() {
                let retry_headers = retry_resp.headers().clone();
                let retry_text = retry_resp.text().await.map_err(reqwest_port_error)?;
                return Err(http_status_port_error(
                    retry_status,
                    &retry_headers,
                    retry_text,
                ));
            }
            retry_resp
        } else {
            return Err(http_status_port_error(status, &headers_clone, text));
        }
    } else if !status.is_success() {
        let headers = response.headers().clone();
        let text = response.text().await.map_err(reqwest_port_error)?;
        return Err(http_status_port_error(status, &headers, text));
    } else {
        response
    };
    let mut byte_stream = response.bytes_stream();
    let mut sse_buffer = SseEventBuffer::default();
    let mut demuxer = ThoughtStreamDemuxer::new();
    let mut total_usage: Option<TokenUsage> = None;
    let mut anthropic_usage = TokenUsage::default();
    let mut byte_buffer = Vec::new();

    while let Some(chunk_res) = byte_stream.next().await {
        let chunk = chunk_res.map_err(reqwest_port_error)?;
        byte_buffer.extend_from_slice(&chunk);

        let (valid_up_to, error_len) = match std::str::from_utf8(&byte_buffer) {
            Ok(_) => (byte_buffer.len(), None),
            Err(e) => (e.valid_up_to(), e.error_len()),
        };

        if valid_up_to > 0 {
            if let Ok(valid_str) = std::str::from_utf8(&byte_buffer[..valid_up_to]) {
                let events = sse_buffer.process(valid_str);
                for event in events {
                    process_sse_event(
                        &event,
                        strategy,
                        accumulator,
                        &mut demuxer,
                        &mut total_usage,
                        &mut anthropic_usage,
                    )?;
                }
            }
            byte_buffer.drain(..valid_up_to);
        } else if let Some(bad_len) = error_len {
            // Skip invalid non-UTF8 bytes to prevent buffer lockup
            byte_buffer.drain(..bad_len);
        }
    }

    if !byte_buffer.is_empty() {
        let text = String::from_utf8_lossy(&byte_buffer);
        let events = sse_buffer.process(&text);
        for event in events {
            process_sse_event(
                &event,
                strategy,
                accumulator,
                &mut demuxer,
                &mut total_usage,
                &mut anthropic_usage,
            )?;
        }
    }

    let remaining_events = sse_buffer.flush();
    for event in remaining_events {
        process_sse_event(
            &event,
            strategy,
            accumulator,
            &mut demuxer,
            &mut total_usage,
            &mut anthropic_usage,
        )?;
    }

    for chunk in demuxer.flush() {
        match chunk.kind {
            LlmStreamDeltaKind::Thought => accumulator.push_thought(&chunk.text)?,
            LlmStreamDeltaKind::Content => accumulator.push_content(&chunk.text)?,
        }
    }

    if strategy == LlmProviderStrategy::Anthropic
        && let Some(usage) = finish_anthropic_stream_usage(anthropic_usage)
    {
        total_usage = Some(usage);
    }

    Ok(StandardLlmResponse {
        text: accumulator.content_text().to_string(),
        thought: if accumulator.thought_text().is_empty() {
            None
        } else {
            Some(accumulator.thought_text().to_string())
        },
        usage: total_usage,
    })
}

fn process_sse_event<EmitFn>(
    event_str: &str,
    strategy: LlmProviderStrategy,
    accumulator: &mut DualStreamAccumulator<'_, EmitFn, LlmPortError>,
    demuxer: &mut ThoughtStreamDemuxer,
    total_usage: &mut Option<TokenUsage>,
    anthropic_usage: &mut TokenUsage,
) -> Result<(), LlmPortError>
where
    EmitFn: FnMut(LlmStreamDelta) -> Result<(), LlmPortError> + Send + ?Sized,
{
    let trimmed = event_str.trim();
    if trimmed.is_empty() || trimmed == "[DONE]" {
        return Ok(());
    }

    let Ok(json) = serde_json::from_str::<Value>(trimmed) else {
        return Ok(());
    };

    // Propagate upstream errors reported in SSE payloads
    if let Some(error_val) = json.get("error") {
        let msg = if let Some(m) = error_val.get("message").and_then(Value::as_str) {
            m.to_string()
        } else if let Some(s) = error_val.as_str() {
            s.to_string()
        } else {
            error_val.to_string()
        };
        return Err(LlmPortError::new(
            LlmPortErrorKind::Protocol,
            format!("Stream received upstream error: {msg}"),
        ));
    }
    if json.get("type").and_then(Value::as_str) == Some("error") {
        let msg = json
            .pointer("/error/message")
            .and_then(Value::as_str)
            .unwrap_or("Anthropic streaming error");
        return Err(LlmPortError::new(
            LlmPortErrorKind::Protocol,
            format!("Stream received upstream error: {msg}"),
        ));
    }

    match strategy {
        LlmProviderStrategy::Anthropic => {
            update_anthropic_stream_usage(anthropic_usage, &json);
            if let Some(event_type) = json.get("type").and_then(Value::as_str)
                && event_type == "content_block_delta"
                && let Some(delta) = json.get("delta")
            {
                if delta.get("type").and_then(Value::as_str) == Some("thinking_delta")
                    && let Some(thinking) = delta.get("thinking").and_then(Value::as_str)
                {
                    accumulator.push_thought(thinking)?;
                } else if delta.get("type").and_then(Value::as_str) == Some("text_delta")
                    && let Some(text) = delta.get("text").and_then(Value::as_str)
                {
                    for chunk in demuxer.process(text) {
                        match chunk.kind {
                            LlmStreamDeltaKind::Thought => {
                                accumulator.push_thought(&chunk.text)?;
                            }
                            LlmStreamDeltaKind::Content => {
                                accumulator.push_content(&chunk.text)?;
                            }
                        }
                    }
                }
            }
        }
        LlmProviderStrategy::Gemini => {
            if let Some(candidates) = json.get("candidates").and_then(Value::as_array) {
                for candidate in candidates {
                    if let Some(parts) = candidate
                        .pointer("/content/parts")
                        .and_then(Value::as_array)
                    {
                        for part in parts {
                            let is_thought = part
                                .get("thought")
                                .and_then(Value::as_bool)
                                .unwrap_or(false);
                            if let Some(text) = part.get("text").and_then(Value::as_str) {
                                if is_thought {
                                    accumulator.push_thought(text)?;
                                } else {
                                    for chunk in demuxer.process(text) {
                                        match chunk.kind {
                                            LlmStreamDeltaKind::Thought => {
                                                accumulator.push_thought(&chunk.text)?;
                                            }
                                            LlmStreamDeltaKind::Content => {
                                                accumulator.push_content(&chunk.text)?;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if let Some(usage) = json.get("usageMetadata").and_then(extract_gemini_usage) {
                *total_usage = Some(usage);
            }
        }
        LlmProviderStrategy::OpenAiResponses => {
            if let Some(usage) = json
                .get("response")
                .and_then(extract_usage_from_json_response)
                .or_else(|| extract_usage_from_json_response(&json))
            {
                *total_usage = Some(usage);
            }
            let event_type = json.get("type").and_then(Value::as_str);
            let is_reasoning_event = event_type == Some("response.reasoning.delta")
                || event_type == Some("response.reasoning_text.delta")
                || json.get("reasoning_delta").is_some()
                || json.get("reasoning_content").is_some();

            if is_reasoning_event {
                let thought = json
                    .get("delta")
                    .or_else(|| json.get("reasoning_delta"))
                    .or_else(|| json.get("reasoning_content"))
                    .or_else(|| json.get("output_text_delta"))
                    .and_then(Value::as_str);
                if let Some(t) = thought {
                    accumulator.push_thought(t)?;
                }
            } else if let Some(delta) = json
                .get("delta")
                .and_then(Value::as_str)
                .or_else(|| json.get("output_text_delta").and_then(Value::as_str))
            {
                for chunk in demuxer.process(delta) {
                    match chunk.kind {
                        LlmStreamDeltaKind::Thought => {
                            accumulator.push_thought(&chunk.text)?;
                        }
                        LlmStreamDeltaKind::Content => {
                            accumulator.push_content(&chunk.text)?;
                        }
                    }
                }
            }
        }
        LlmProviderStrategy::Cohere => {
            if let Some(usage) = extract_usage_from_json_response(&json) {
                *total_usage = Some(usage);
            }
            if let Some(delta_text) = json
                .pointer("/delta/message/content/text")
                .or_else(|| json.pointer("/delta/text"))
                .and_then(Value::as_str)
            {
                for chunk in demuxer.process(delta_text) {
                    match chunk.kind {
                        LlmStreamDeltaKind::Thought => {
                            accumulator.push_thought(&chunk.text)?;
                        }
                        LlmStreamDeltaKind::Content => {
                            accumulator.push_content(&chunk.text)?;
                        }
                    }
                }
            }
        }
        _ => {
            if let Some(usage) = extract_usage_from_json_response(&json) {
                *total_usage = Some(usage);
            }
            if let Some(choice) = json.pointer("/choices/0") {
                let delta = choice.get("delta").or_else(|| choice.get("message"));
                if let Some(delta) = delta {
                    let thought_field = delta
                        .get("reasoning_content")
                        .or_else(|| delta.get("reasoning"))
                        .or_else(|| delta.get("thought"))
                        .or_else(|| delta.get("thinking"))
                        .or_else(|| delta.get("reasoning_text"))
                        .and_then(Value::as_str);

                    if let Some(thought) = thought_field {
                        accumulator.push_thought(thought)?;
                    }

                    if let Some(content) = delta.get("content").and_then(Value::as_str) {
                        for chunk in demuxer.process(content) {
                            match chunk.kind {
                                LlmStreamDeltaKind::Thought => {
                                    accumulator.push_thought(&chunk.text)?;
                                }
                                LlmStreamDeltaKind::Content => {
                                    accumulator.push_content(&chunk.text)?;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
