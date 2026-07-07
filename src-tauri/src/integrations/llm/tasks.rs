use super::network::{LlmApiUrl, validate_llm_api_host};
use super::*;
use futures_util::{StreamExt, future::BoxFuture, stream};
use log::{info, warn};
use reqwest::{Client, StatusCode, header::RETRY_AFTER};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use sona_core::llm_tasks::{PlannedSegmentChunk, chunk_error, normalize_incremental_json_line};
pub(crate) use sona_core::llm_tasks::{
    build_polish_prompt, build_translate_prompt, parse_polish_chunk, parse_translate_chunk,
    plan_segment_task_chunks, run_summary_task, validate_summary_strategy,
};
use std::{future::Future, time::Duration};

pub(crate) struct SegmentTaskContext<'a> {
    pub(crate) task_id: &'a str,
    pub(crate) task_type: LlmTaskType,
    pub(crate) segments: &'a [LlmSegmentInput],
    pub(crate) chunk_size: Option<usize>,
    pub(crate) prompt_char_budget: usize,
}

impl<'a> SegmentTaskContext<'a> {
    fn plan_chunks<BuildPrompt>(&self, build_prompt: &mut BuildPrompt) -> Vec<PlannedSegmentChunk>
    where
        BuildPrompt: FnMut(&[LlmSegmentInput]) -> String,
    {
        plan_segment_task_chunks(
            self.task_id,
            self.task_type,
            self.segments,
            self.chunk_size,
            self.prompt_char_budget,
            build_prompt,
        )
    }

    fn chunk_payload<Output>(
        &self,
        chunk_number: usize,
        total_chunks: usize,
        items: Vec<Output>,
    ) -> LlmTaskChunkPayload<Output> {
        LlmTaskChunkPayload {
            task_id: self.task_id.to_string(),
            task_type: self.task_type,
            chunk_index: chunk_number,
            total_chunks,
            items,
        }
    }

    fn progress_payload(
        &self,
        completed_chunks: usize,
        total_chunks: usize,
    ) -> LlmTaskProgressPayload {
        LlmTaskProgressPayload {
            task_id: self.task_id.to_string(),
            task_type: self.task_type,
            completed_chunks,
            total_chunks,
        }
    }
}

pub(crate) struct BufferedSegmentTaskConfig<
    BuildPrompt,
    ParseChunk,
    GenerateFn,
    OnSuccessFn,
    EmitChunkFn,
    EmitProgressFn,
> {
    pub(crate) build_prompt: BuildPrompt,
    pub(crate) parse_chunk: ParseChunk,
    pub(crate) generate_text: GenerateFn,
    pub(crate) on_success: OnSuccessFn,
    pub(crate) emit_chunk: EmitChunkFn,
    pub(crate) emit_progress: EmitProgressFn,
}

pub(crate) struct StreamingSegmentTaskConfig<
    BuildPrompt,
    ParseChunk,
    BuildRequest,
    GetId,
    OnSuccessFn,
    EmitChunkFn,
    EmitProgressFn,
> {
    pub(crate) build_prompt: BuildPrompt,
    pub(crate) parse_chunk: ParseChunk,
    pub(crate) build_request: BuildRequest,
    pub(crate) get_output_id: GetId,
    pub(crate) on_success: OnSuccessFn,
    pub(crate) emit_chunk: EmitChunkFn,
    pub(crate) emit_progress: EmitProgressFn,
}

#[derive(Debug, Clone)]
pub(crate) enum GoogleTranslateFreeAttemptError {
    HttpStatus {
        status: StatusCode,
        retry_after: Option<Duration>,
    },
    Message(String),
}

fn format_attempt_label(attempts: usize) -> &'static str {
    if attempts == 1 { "attempt" } else { "attempts" }
}

pub(crate) fn parse_google_translate_free_retry_after(
    headers: &reqwest::header::HeaderMap,
) -> Option<Duration> {
    headers
        .get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(|seconds| clamp_google_translate_free_retry_after(Duration::from_secs(seconds)))
}

fn clamp_google_translate_free_retry_after(duration: Duration) -> Duration {
    // Treat server-provided Retry-After as a hint, but cap it so one response
    // cannot stall the whole translation task for an excessive amount of time.
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
        // Only 429 responses are retried. Other free-endpoint failures usually
        // indicate bad input or a non-transient upstream error, so retrying
        // would just add latency without improving success rate.
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

pub(crate) async fn fetch_google_translate_free_translation(
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

pub(crate) async fn execute_google_translate_free_request<
    FetchFn,
    FetchFuture,
    SleepFn,
    SleepFuture,
>(
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

pub(crate) async fn run_google_translate_free_requests_in_order<RunFn, RunFuture>(
    texts: Vec<String>,
    max_concurrency: usize,
    mut run_request: RunFn,
) -> Result<Vec<String>, String>
where
    RunFn: FnMut(usize, String) -> RunFuture,
    RunFuture: Future<Output = Result<(usize, String), String>>,
{
    let mut indexed_translations = Vec::with_capacity(texts.len());
    let results = stream::iter(texts.into_iter().enumerate())
        .map(move |(index, text)| run_request(index, text))
        .buffer_unordered(max_concurrency.max(1))
        .collect::<Vec<_>>()
        .await;

    for result in results {
        indexed_translations.push(result?);
    }

    indexed_translations.sort_by_key(|(index, _)| *index);
    Ok(indexed_translations
        .into_iter()
        .map(|(_, translation)| translation)
        .collect())
}

pub(crate) fn validate_llm_config(config: &LlmConfig) -> Result<(), String> {
    if config.model.trim().is_empty() {
        return Err("Model name cannot be empty".to_string());
    }

    validate_llm_api_host(&config.base_url)?;

    Ok(())
}

pub(crate) fn validate_task_request(task_id: &str, config: &LlmConfig) -> Result<(), String> {
    if task_id.trim().is_empty() {
        return Err("Task ID cannot be empty".to_string());
    }

    validate_llm_config(config)
}

pub(crate) async fn run_segment_task<
    Output,
    BuildPrompt,
    ParseChunk,
    GenerateFn,
    OnSuccessFn,
    EmitChunkFn,
    EmitProgressFn,
>(
    context: SegmentTaskContext<'_>,
    config: BufferedSegmentTaskConfig<
        BuildPrompt,
        ParseChunk,
        GenerateFn,
        OnSuccessFn,
        EmitChunkFn,
        EmitProgressFn,
    >,
) -> Result<Vec<Output>, String>
where
    Output: Serialize + Clone,
    BuildPrompt: FnMut(&[LlmSegmentInput]) -> String,
    ParseChunk: FnMut(&str, &[LlmSegmentInput], usize) -> Result<Vec<Output>, String>,
    GenerateFn: FnMut(String) -> BoxFuture<'static, Result<StandardLlmResponse, String>>,
    OnSuccessFn: FnMut(&StandardLlmResponse),
    EmitChunkFn: FnMut(LlmTaskChunkPayload<Output>) -> Result<(), String>,
    EmitProgressFn: FnMut(LlmTaskProgressPayload) -> Result<(), String>,
{
    if context.segments.is_empty() {
        return Ok(Vec::new());
    }

    let BufferedSegmentTaskConfig {
        mut build_prompt,
        mut parse_chunk,
        mut generate_text,
        mut on_success,
        mut emit_chunk,
        mut emit_progress,
    } = config;

    let planned_chunks = context.plan_chunks(&mut build_prompt);
    let total_chunks = planned_chunks.len();
    let mut results = Vec::with_capacity(context.segments.len());

    for (chunk_index, planned_chunk) in planned_chunks.into_iter().enumerate() {
        let chunk_number = chunk_index + 1;
        let chunk = &context.segments[planned_chunk.start..planned_chunk.end];
        let response = generate_text(planned_chunk.prompt)
            .await
            .map_err(|error| chunk_error(context.task_type, chunk_number, error))?;
        on_success(&response);
        let response_text = response.text;
        let parsed = parse_chunk(&response_text, chunk, chunk_number)?;

        emit_chunk(context.chunk_payload(chunk_number, total_chunks, parsed.clone()))?;

        emit_progress(context.progress_payload(chunk_number, total_chunks))?;

        results.extend(parsed);
    }

    Ok(results)
}

pub(crate) async fn run_streaming_segment_task<
    Output,
    BuildPrompt,
    ParseChunk,
    BuildRequest,
    GetId,
    OnSuccessFn,
    EmitChunkFn,
    EmitProgressFn,
>(
    context: SegmentTaskContext<'_>,
    config: StreamingSegmentTaskConfig<
        BuildPrompt,
        ParseChunk,
        BuildRequest,
        GetId,
        OnSuccessFn,
        EmitChunkFn,
        EmitProgressFn,
    >,
) -> Result<Vec<Output>, String>
where
    Output: Serialize + Clone + DeserializeOwned,
    BuildPrompt: FnMut(&[LlmSegmentInput]) -> String,
    ParseChunk: FnMut(&str, &[LlmSegmentInput], usize) -> Result<Vec<Output>, String>,
    BuildRequest: FnMut(String) -> LlmGenerateRequest,
    GetId: for<'item> FnMut(&'item Output) -> &'item str,
    OnSuccessFn: FnMut(&StandardLlmResponse),
    EmitChunkFn: FnMut(LlmTaskChunkPayload<Output>) -> Result<(), String>,
    EmitProgressFn: FnMut(LlmTaskProgressPayload) -> Result<(), String>,
{
    if context.segments.is_empty() {
        return Ok(Vec::new());
    }

    let StreamingSegmentTaskConfig {
        mut build_prompt,
        mut parse_chunk,
        mut build_request,
        mut get_output_id,
        mut on_success,
        mut emit_chunk,
        mut emit_progress,
    } = config;

    let planned_chunks = context.plan_chunks(&mut build_prompt);
    let total_chunks = planned_chunks.len();
    let mut results = Vec::with_capacity(context.segments.len());

    for (chunk_index, planned_chunk) in planned_chunks.into_iter().enumerate() {
        let chunk_number = chunk_index + 1;
        let chunk = &context.segments[planned_chunk.start..planned_chunk.end];
        let mut line_buffer = StreamingLineBuffer::default();
        let mut streamed_items = Vec::new();
        let response_text = {
            let mut emit_streamed_line = |line: &str| -> Result<(), String> {
                let Some(normalized) = normalize_incremental_json_line(line) else {
                    return Ok(());
                };

                let parsed = serde_json::from_str::<Output>(&normalized).map_err(|error| {
                    chunk_error(
                        context.task_type,
                        chunk_number,
                        format!("invalid JSON response: {error}"),
                    )
                })?;
                let expected_segment = chunk.get(streamed_items.len()).ok_or_else(|| {
                    chunk_error(
                        context.task_type,
                        chunk_number,
                        "received more objects than expected",
                    )
                })?;
                let actual_id = get_output_id(&parsed);
                if actual_id != expected_segment.id {
                    return Err(chunk_error(
                        context.task_type,
                        chunk_number,
                        format!(
                            "segment {} expected id '{}' but received '{}'",
                            streamed_items.len() + 1,
                            expected_segment.id,
                            actual_id
                        ),
                    ));
                }

                streamed_items.push(parsed.clone());
                emit_chunk(context.chunk_payload(chunk_number, total_chunks, vec![parsed]))?;
                Ok(())
            };

            let response = generate_with_optional_streaming(
                build_request(planned_chunk.prompt),
                &mut |_, delta| {
                    for line in line_buffer.process(delta) {
                        emit_streamed_line(&line)?;
                    }
                    Ok(())
                },
            )
            .await
            .map_err(|error| chunk_error(context.task_type, chunk_number, error))?;
            on_success(&response);
            let response_text = response.text;
            for line in line_buffer.flush() {
                emit_streamed_line(&line)?;
            }
            response_text
        };

        let parsed = if streamed_items.is_empty() {
            let parsed = parse_chunk(&response_text, chunk, chunk_number)?;
            emit_chunk(context.chunk_payload(chunk_number, total_chunks, parsed.clone()))?;
            parsed
        } else {
            if streamed_items.len() != chunk.len() {
                return Err(chunk_error(
                    context.task_type,
                    chunk_number,
                    format!(
                        "expected {} objects but received {}",
                        chunk.len(),
                        streamed_items.len()
                    ),
                ));
            }
            streamed_items
        };

        emit_progress(context.progress_payload(chunk_number, total_chunks))?;

        results.extend(parsed);
    }

    Ok(results)
}
