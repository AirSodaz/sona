use super::network::validate_llm_api_host;
use super::*;
use futures_util::{StreamExt, future::BoxFuture, stream};
use serde::{Serialize, de::DeserializeOwned};
use sona_core::llm_tasks::{PlannedSegmentChunk, chunk_error, normalize_incremental_json_line};
pub(crate) use sona_core::llm_tasks::{
    build_polish_prompt, build_translate_prompt, parse_polish_chunk, parse_translate_chunk,
    plan_segment_task_chunks, run_summary_task, validate_summary_strategy,
};
#[cfg(test)]
pub(crate) use sona_online_llm::{
    GoogleTranslateFreeAttemptError, parse_google_translate_free_retry_after,
};
pub(crate) use sona_online_llm::{
    execute_google_translate_free_request, fetch_google_translate_free_translation,
};
use std::future::Future;

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
