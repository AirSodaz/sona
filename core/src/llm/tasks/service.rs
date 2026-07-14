use std::collections::HashSet;
use std::time::Duration;

use futures_util::{StreamExt, stream};
use serde::Serialize;

use crate::llm::provider_protocol::LlmModelSummary;
use crate::llm::requests::{
    LlmConfig, PolishSegmentsRequest, SummarizeTranscriptRequest, TranslateSegmentsRequest,
    validate_polish_segments_request, validate_summarize_transcript_request,
    validate_translate_segments_request,
};
use crate::llm::runtime::{
    LlmCapabilityPolicy, LlmCompletionOptions, LlmCompletionRequest, LlmCompletionResponse,
    LlmPromptCachePolicy, LlmResponseFormat, LlmRuntimeError, LlmRuntimeService, LlmStreamDelta,
};
use crate::ports::llm::{
    LlmPortError, LlmPortErrorKind, LlmTaskRuntimePort, LlmTranslationRequest,
};

const DEFAULT_TASK_CONCURRENCY: usize = 2;
const MAX_NETWORK_RETRIES: u32 = 2;
const DEFAULT_RETRY_DELAYS_MS: [u64; 2] = [250, 1000];
const MAX_RETRY_AFTER_MS: u64 = 5000;
const DEFAULT_SUMMARY_MAP_OUTPUT_TOKENS: u64 = 2048;
const DEFAULT_SUMMARY_FINAL_OUTPUT_TOKENS: u64 = 4096;
const MAX_SUMMARY_REDUCE_ROUNDS: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmTaskSummaryChunkPayload {
    pub task_id: String,
    pub chunk_index: usize,
    pub total_chunks: usize,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum LlmTaskResult {
    Polish(Vec<super::PolishedSegment>),
    Translate(Vec<super::TranslatedSegment>),
    Summary(super::TranscriptSummaryResult),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum LlmTaskEvent {
    Progress(super::LlmTaskProgressPayload),
    PolishChunk(super::LlmTaskChunkPayload<super::PolishedSegment>),
    TranslateChunk(super::LlmTaskChunkPayload<super::TranslatedSegment>),
    SummaryChunk(LlmTaskSummaryChunkPayload),
    Text(super::LlmTaskTextPayload),
    Completed(LlmTaskResult),
}

pub trait LlmTaskObserver: Send + Sync {
    fn on_event(&self, event: LlmTaskEvent) -> Result<(), String>;
}

impl LlmTaskObserver for () {
    fn on_event(&self, _event: LlmTaskEvent) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LlmTaskError {
    #[error("{reason}")]
    InvalidRequest { reason: String },
    #[error("{stage} failed: {source}")]
    Runtime {
        stage: String,
        #[source]
        source: LlmRuntimeError,
    },
    #[error("{reason}")]
    InvalidResponse { reason: String },
    #[error("LLM task observer failed: {reason}")]
    Observer { reason: String },
}

pub struct LlmTaskService<Runtime> {
    runtime: Runtime,
    max_concurrency: usize,
}

impl<Runtime> LlmTaskService<Runtime>
where
    Runtime: LlmTaskRuntimePort,
{
    pub fn new(runtime: Runtime) -> Self {
        Self {
            runtime,
            max_concurrency: DEFAULT_TASK_CONCURRENCY,
        }
    }

    pub fn with_max_concurrency(runtime: Runtime, max_concurrency: usize) -> Self {
        Self {
            runtime,
            max_concurrency: max_concurrency.max(1),
        }
    }

    pub async fn polish(
        &self,
        request: PolishSegmentsRequest,
        observer: &dyn LlmTaskObserver,
    ) -> Result<Vec<super::PolishedSegment>, LlmTaskError> {
        validate_polish_segments_request(&request)
            .map_err(|reason| LlmTaskError::InvalidRequest { reason })?;
        validate_segment_inputs(&request.segments)?;
        if request.segments.is_empty() {
            let result = Vec::new();
            emit(
                observer,
                LlmTaskEvent::Completed(LlmTaskResult::Polish(result.clone())),
            )?;
            return Ok(result);
        }

        let model = self.describe_model(&request.config).await;
        let budget = super::resolve_task_budget(
            model.as_ref(),
            super::DEFAULT_SEGMENT_PROMPT_CHAR_BUDGET,
            super::estimate_structured_output_tokens(&request.segments),
        );
        let chunks = super::plan_segment_chunks_with_budget(
            &request.segments,
            request.chunk_size,
            budget,
            |segments| {
                super::build_polish_task_input(
                    segments,
                    request.context.as_deref(),
                    request.keywords.as_deref(),
                )
            },
        )
        .map_err(|reason| LlmTaskError::InvalidRequest { reason })?;
        let total_chunks = chunks.len();
        let cache = task_cache_policy(total_chunks);
        let config = request.config.clone();
        let segments = &request.segments;
        let mut pending = stream::iter(chunks.into_iter().enumerate())
            .map(|(chunk_index, planned)| {
                let chunk = segments[planned.start..planned.end].to_vec();
                let config = config.clone();
                async move {
                    self.complete_polish_chunk(
                        config,
                        planned.prompt,
                        chunk,
                        chunk_index + 1,
                        cache,
                        budget.max_output_tokens,
                    )
                    .await
                    .map(|items| (chunk_index, items))
                }
            })
            .buffer_unordered(self.max_concurrency);

        let mut ordered = vec![None; total_chunks];
        let mut completed = 0usize;
        while let Some(result) = pending.next().await {
            let (chunk_index, items) = result?;
            completed += 1;
            emit(
                observer,
                LlmTaskEvent::PolishChunk(super::LlmTaskChunkPayload {
                    task_id: request.task_id.clone(),
                    task_type: super::LlmTaskType::Polish,
                    chunk_index: chunk_index + 1,
                    total_chunks,
                    items: items.clone(),
                }),
            )?;
            emit_progress(
                observer,
                &request.task_id,
                super::LlmTaskType::Polish,
                completed,
                total_chunks,
            )?;
            ordered[chunk_index] = Some(items);
        }

        let result = ordered.into_iter().flatten().flatten().collect::<Vec<_>>();
        emit(
            observer,
            LlmTaskEvent::Completed(LlmTaskResult::Polish(result.clone())),
        )?;
        Ok(result)
    }

    pub async fn translate(
        &self,
        request: TranslateSegmentsRequest,
        observer: &dyn LlmTaskObserver,
    ) -> Result<Vec<super::TranslatedSegment>, LlmTaskError> {
        validate_translate_segments_request(&request)
            .map_err(|reason| LlmTaskError::InvalidRequest { reason })?;
        validate_segment_inputs(&request.segments)?;
        if request.segments.is_empty() {
            let result = Vec::new();
            emit(
                observer,
                LlmTaskEvent::Completed(LlmTaskResult::Translate(result.clone())),
            )?;
            return Ok(result);
        }

        let model = self.describe_model(&request.config).await;
        let budget = super::resolve_task_budget(
            model.as_ref(),
            super::DEFAULT_SEGMENT_PROMPT_CHAR_BUDGET,
            super::estimate_structured_output_tokens(&request.segments),
        );
        let free_translation =
            request.config.strategy == super::LlmProviderStrategy::GoogleTranslateFree;
        let chunks = super::plan_segment_chunks_with_budget(
            &request.segments,
            if free_translation {
                Some(1)
            } else {
                request.chunk_size
            },
            budget,
            |segments| {
                super::build_translate_task_input(
                    segments,
                    &request.target_language,
                    request.target_language_name.as_deref(),
                )
            },
        )
        .map_err(|reason| LlmTaskError::InvalidRequest { reason })?;
        let total_chunks = chunks.len();
        let cache = task_cache_policy(total_chunks);
        let config = request.config.clone();
        let target_language = request.target_language.clone();
        let segments = &request.segments;
        let direct_translation = matches!(
            request.config.strategy,
            super::LlmProviderStrategy::GoogleTranslate
                | super::LlmProviderStrategy::GoogleTranslateFree
        );
        let mut pending = stream::iter(chunks.into_iter().enumerate())
            .map(|(chunk_index, planned)| {
                let chunk = segments[planned.start..planned.end].to_vec();
                let config = config.clone();
                let target_language = target_language.clone();
                async move {
                    let result = if direct_translation {
                        self.translate_direct_chunk(config, chunk, target_language, chunk_index + 1)
                            .await
                    } else {
                        self.complete_translate_chunk(
                            config,
                            planned.prompt,
                            chunk,
                            chunk_index + 1,
                            cache,
                            budget.max_output_tokens,
                        )
                        .await
                    };
                    result.map(|items| (chunk_index, items))
                }
            })
            .buffer_unordered(self.max_concurrency);

        let mut ordered = vec![None; total_chunks];
        let mut completed = 0usize;
        while let Some(result) = pending.next().await {
            let (chunk_index, items) = result?;
            completed += 1;
            emit(
                observer,
                LlmTaskEvent::TranslateChunk(super::LlmTaskChunkPayload {
                    task_id: request.task_id.clone(),
                    task_type: super::LlmTaskType::Translate,
                    chunk_index: chunk_index + 1,
                    total_chunks,
                    items: items.clone(),
                }),
            )?;
            emit_progress(
                observer,
                &request.task_id,
                super::LlmTaskType::Translate,
                completed,
                total_chunks,
            )?;
            ordered[chunk_index] = Some(items);
        }

        let result = ordered.into_iter().flatten().flatten().collect::<Vec<_>>();
        emit(
            observer,
            LlmTaskEvent::Completed(LlmTaskResult::Translate(result.clone())),
        )?;
        Ok(result)
    }

    pub async fn summarize(
        &self,
        request: SummarizeTranscriptRequest,
        observer: &dyn LlmTaskObserver,
    ) -> Result<super::TranscriptSummaryResult, LlmTaskError> {
        validate_summarize_transcript_request(&request)
            .map_err(|reason| LlmTaskError::InvalidRequest { reason })?;
        if request.segments.is_empty() {
            return Err(LlmTaskError::InvalidRequest {
                reason: "Transcript cannot be empty".to_string(),
            });
        }

        let model = self.describe_model(&request.config).await;
        let explicit_budget = request
            .chunk_char_budget
            .filter(|value| *value > 0)
            .unwrap_or(super::DEFAULT_SUMMARY_CHUNK_CHAR_BUDGET);
        let map_budget = super::resolve_task_budget(
            model.as_ref(),
            explicit_budget,
            DEFAULT_SUMMARY_MAP_OUTPUT_TOKENS,
        );
        let final_budget = super::resolve_task_budget(
            model.as_ref(),
            super::DEFAULT_SEGMENT_CONTEXT_CHAR_BUDGET,
            DEFAULT_SUMMARY_FINAL_OUTPUT_TOKENS,
        );
        let chunk_char_budget = super::resolve_summary_chunk_char_budget(
            &request.template,
            explicit_budget,
            final_budget,
            request.segments.len(),
        );
        let chunks = super::split_summary_segments(&request.segments, chunk_char_budget);

        if chunks.len() == 1 {
            let prompt = super::build_summary_direct_prompt(&request.template, &chunks[0]);
            if !super::summary_prompt_fits_budget(
                super::SUMMARY_FINAL_SYSTEM_PROMPT,
                &prompt,
                final_budget,
            ) {
                return Err(LlmTaskError::InvalidRequest {
                    reason: "Transcript exceeds the model context budget".to_string(),
                });
            }
            let response = self
                .stream_summary(
                    &request,
                    prompt,
                    LlmPromptCachePolicy::Disabled,
                    final_budget.max_output_tokens,
                    observer,
                )
                .await?;
            emit_progress(
                observer,
                &request.task_id,
                super::LlmTaskType::Summary,
                1,
                1,
            )?;
            let result = super::TranscriptSummaryResult {
                template_id: request.template.id.clone(),
                content: response.text.trim().to_string(),
            };
            emit(
                observer,
                LlmTaskEvent::Completed(LlmTaskResult::Summary(result.clone())),
            )?;
            return Ok(result);
        }

        let map_count = chunks.len();
        let total_chunks = map_count + 1;
        let config = request.config.clone();
        let template = &request.template;
        let mut pending = stream::iter(chunks.into_iter().enumerate())
            .map(|(chunk_index, chunk)| {
                let config = config.clone();
                let prompt =
                    super::build_summary_chunk_prompt(template, &chunk, chunk_index + 1, map_count);
                async move {
                    self.complete_text(
                        config,
                        super::SUMMARY_MAP_SYSTEM_PROMPT,
                        prompt,
                        LlmPromptCachePolicy::Automatic,
                        map_budget.max_output_tokens,
                        "summary chunk",
                    )
                    .await
                    .map(|response| (chunk_index, response.text.trim().to_string()))
                }
            })
            .buffer_unordered(self.max_concurrency);

        let mut partials = vec![None; map_count];
        let mut completed = 0usize;
        while let Some(result) = pending.next().await {
            let (chunk_index, text) = result?;
            completed += 1;
            emit(
                observer,
                LlmTaskEvent::SummaryChunk(LlmTaskSummaryChunkPayload {
                    task_id: request.task_id.clone(),
                    chunk_index: chunk_index + 1,
                    total_chunks,
                    text: text.clone(),
                }),
            )?;
            emit_progress(
                observer,
                &request.task_id,
                super::LlmTaskType::Summary,
                completed,
                total_chunks,
            )?;
            partials[chunk_index] = Some(text);
        }

        let partials = partials.into_iter().flatten().collect::<Vec<_>>();
        let partials = self
            .reduce_summary_partials(&request, partials, final_budget)
            .await?;
        let response = self
            .stream_summary(
                &request,
                super::build_summary_finalize_prompt(&request.template, &partials),
                LlmPromptCachePolicy::Automatic,
                final_budget.max_output_tokens,
                observer,
            )
            .await?;
        emit_progress(
            observer,
            &request.task_id,
            super::LlmTaskType::Summary,
            total_chunks,
            total_chunks,
        )?;
        let result = super::TranscriptSummaryResult {
            template_id: request.template.id.clone(),
            content: response.text.trim().to_string(),
        };
        emit(
            observer,
            LlmTaskEvent::Completed(LlmTaskResult::Summary(result.clone())),
        )?;
        Ok(result)
    }

    async fn complete_polish_chunk(
        &self,
        config: LlmConfig,
        input: String,
        expected: Vec<super::LlmSegmentInput>,
        chunk_number: usize,
        cache: LlmPromptCachePolicy,
        max_output_tokens: Option<u64>,
    ) -> Result<Vec<super::PolishedSegment>, LlmTaskError> {
        let format = LlmResponseFormat::JsonSchema {
            name: "polished_segments".to_string(),
            schema: super::polish_output_schema(expected.len()),
        };
        let request = structured_request(
            config.clone(),
            super::POLISH_SYSTEM_PROMPT,
            input.clone(),
            format.clone(),
            cache,
            max_output_tokens,
        );
        match self.complete_with_retry(request).await {
            Ok(response) => match parse_polish_response(&response, &expected, chunk_number) {
                Ok(items) => Ok(items),
                Err(reason) => {
                    let repair =
                        super::build_structured_repair_input(&input, &reason, Some(&response.text));
                    let response = self
                        .complete_with_retry(structured_request(
                            config,
                            super::POLISH_SYSTEM_PROMPT,
                            repair,
                            format,
                            cache,
                            max_output_tokens,
                        ))
                        .await
                        .map_err(|source| runtime_error("polish repair", source))?;
                    parse_polish_response(&response, &expected, chunk_number)
                        .map_err(|reason| LlmTaskError::InvalidResponse { reason })
                }
            },
            Err(LlmRuntimeError::InvalidResponse { reason }) => {
                let repair = super::build_structured_repair_input(&input, &reason, None);
                let response = self
                    .complete_with_retry(structured_request(
                        config,
                        super::POLISH_SYSTEM_PROMPT,
                        repair,
                        format,
                        cache,
                        max_output_tokens,
                    ))
                    .await
                    .map_err(|source| runtime_error("polish repair", source))?;
                parse_polish_response(&response, &expected, chunk_number)
                    .map_err(|reason| LlmTaskError::InvalidResponse { reason })
            }
            Err(source) => Err(runtime_error(
                &format!("polish chunk {chunk_number}"),
                source,
            )),
        }
    }

    async fn complete_translate_chunk(
        &self,
        config: LlmConfig,
        input: String,
        expected: Vec<super::LlmSegmentInput>,
        chunk_number: usize,
        cache: LlmPromptCachePolicy,
        max_output_tokens: Option<u64>,
    ) -> Result<Vec<super::TranslatedSegment>, LlmTaskError> {
        let format = LlmResponseFormat::JsonSchema {
            name: "translated_segments".to_string(),
            schema: super::translate_output_schema(expected.len()),
        };
        let request = structured_request(
            config.clone(),
            super::TRANSLATE_SYSTEM_PROMPT,
            input.clone(),
            format.clone(),
            cache,
            max_output_tokens,
        );
        match self.complete_with_retry(request).await {
            Ok(response) => match parse_translate_response(&response, &expected, chunk_number) {
                Ok(items) => Ok(items),
                Err(reason) => {
                    let repair =
                        super::build_structured_repair_input(&input, &reason, Some(&response.text));
                    let response = self
                        .complete_with_retry(structured_request(
                            config,
                            super::TRANSLATE_SYSTEM_PROMPT,
                            repair,
                            format,
                            cache,
                            max_output_tokens,
                        ))
                        .await
                        .map_err(|source| runtime_error("translate repair", source))?;
                    parse_translate_response(&response, &expected, chunk_number)
                        .map_err(|reason| LlmTaskError::InvalidResponse { reason })
                }
            },
            Err(LlmRuntimeError::InvalidResponse { reason }) => {
                let repair = super::build_structured_repair_input(&input, &reason, None);
                let response = self
                    .complete_with_retry(structured_request(
                        config,
                        super::TRANSLATE_SYSTEM_PROMPT,
                        repair,
                        format,
                        cache,
                        max_output_tokens,
                    ))
                    .await
                    .map_err(|source| runtime_error("translate repair", source))?;
                parse_translate_response(&response, &expected, chunk_number)
                    .map_err(|reason| LlmTaskError::InvalidResponse { reason })
            }
            Err(source) => Err(runtime_error(
                &format!("translate chunk {chunk_number}"),
                source,
            )),
        }
    }

    async fn translate_direct_chunk(
        &self,
        config: LlmConfig,
        expected: Vec<super::LlmSegmentInput>,
        target_language: String,
        chunk_number: usize,
    ) -> Result<Vec<super::TranslatedSegment>, LlmTaskError> {
        let texts = expected
            .iter()
            .map(|segment| segment.text.clone())
            .collect();
        let translations = self
            .translate_with_retry(LlmTranslationRequest {
                config,
                texts,
                target_language,
            })
            .await
            .map_err(|source| runtime_error(&format!("translate chunk {chunk_number}"), source))?;
        if translations.len() != expected.len() {
            return Err(LlmTaskError::InvalidResponse {
                reason: super::chunk_error(
                    super::LlmTaskType::Translate,
                    chunk_number,
                    format!(
                        "expected {} translations but received {}",
                        expected.len(),
                        translations.len()
                    ),
                ),
            });
        }
        Ok(expected
            .into_iter()
            .zip(translations)
            .map(|(segment, translation)| super::TranslatedSegment {
                id: segment.id,
                translation,
            })
            .collect())
    }

    async fn complete_text(
        &self,
        config: LlmConfig,
        system_prompt: &'static str,
        input: String,
        cache: LlmPromptCachePolicy,
        max_output_tokens: Option<u64>,
        stage: &'static str,
    ) -> Result<LlmCompletionResponse, LlmTaskError> {
        self.complete_with_retry(text_request(
            config,
            system_prompt,
            input,
            cache,
            max_output_tokens,
        ))
        .await
        .map_err(|source| runtime_error(stage, source))
    }

    async fn reduce_summary_partials(
        &self,
        request: &SummarizeTranscriptRequest,
        mut partials: Vec<String>,
        budget: super::LlmTaskBudget,
    ) -> Result<Vec<String>, LlmTaskError> {
        for _ in 0..MAX_SUMMARY_REDUCE_ROUNDS {
            let final_prompt = super::build_summary_finalize_prompt(&request.template, &partials);
            if super::summary_prompt_fits_budget(
                super::SUMMARY_FINAL_SYSTEM_PROMPT,
                &final_prompt,
                budget,
            ) {
                return Ok(partials);
            }

            let previous_count = partials.len();
            let previous_chars = partials
                .iter()
                .map(|value| value.chars().count())
                .sum::<usize>();
            let groups =
                super::split_summary_partials_for_reduce(&request.template, &partials, budget);
            let mut reduced = Vec::with_capacity(groups.len());
            for group in groups {
                let prompt = super::build_summary_reduce_prompt(&request.template, &group);
                if !super::summary_prompt_fits_budget(
                    super::SUMMARY_REDUCE_SYSTEM_PROMPT,
                    &prompt,
                    budget,
                ) {
                    return Err(LlmTaskError::InvalidResponse {
                        reason: "An intermediate summary exceeds the model context budget"
                            .to_string(),
                    });
                }
                let response = self
                    .complete_text(
                        request.config.clone(),
                        super::SUMMARY_REDUCE_SYSTEM_PROMPT,
                        prompt,
                        LlmPromptCachePolicy::Automatic,
                        summary_reduce_output_tokens(budget),
                        "summary reduce",
                    )
                    .await?;
                let text = response.text.trim();
                if text.is_empty() {
                    return Err(LlmTaskError::InvalidResponse {
                        reason: "Summary reduce returned empty text".to_string(),
                    });
                }
                reduced.push(text.to_string());
            }

            let reduced_chars = reduced
                .iter()
                .map(|value| value.chars().count())
                .sum::<usize>();
            if reduced.len() >= previous_count && reduced_chars >= previous_chars {
                return Err(LlmTaskError::InvalidResponse {
                    reason: "Summary reduce did not fit the model context budget".to_string(),
                });
            }
            partials = reduced;
        }

        Err(LlmTaskError::InvalidResponse {
            reason: "Summary reduce exceeded the maximum number of passes".to_string(),
        })
    }

    async fn stream_summary(
        &self,
        request: &SummarizeTranscriptRequest,
        input: String,
        cache: LlmPromptCachePolicy,
        max_output_tokens: Option<u64>,
        observer: &dyn LlmTaskObserver,
    ) -> Result<LlmCompletionResponse, LlmTaskError> {
        self.stream_with_retry(
            text_request(
                request.config.clone(),
                super::SUMMARY_FINAL_SYSTEM_PROMPT,
                input,
                cache,
                max_output_tokens,
            ),
            &request.task_id,
            observer,
        )
        .await
        .map_err(|source| runtime_error("summary final synthesis", source))
    }

    async fn describe_model(&self, config: &LlmConfig) -> Option<LlmModelSummary> {
        LlmRuntimeService::new(&self.runtime, self.runtime.clone())
            .describe_model(config)
            .await
            .ok()
            .flatten()
    }

    async fn complete_with_retry(
        &self,
        request: LlmCompletionRequest,
    ) -> Result<LlmCompletionResponse, LlmRuntimeError> {
        let mut failed_attempts = 0u32;
        loop {
            let result = LlmRuntimeService::new(&self.runtime, self.runtime.clone())
                .complete(request.clone())
                .await;
            match result {
                Ok(mut response) => {
                    response.execution.attempts = failed_attempts + 1;
                    return Ok(response);
                }
                Err(error) => {
                    failed_attempts += 1;
                    let Some(delay) = llm_task_retry_delay(&error, failed_attempts) else {
                        return Err(error);
                    };
                    self.runtime.delay(delay).await;
                }
            }
        }
    }

    async fn stream_with_retry(
        &self,
        request: LlmCompletionRequest,
        task_id: &str,
        observer: &dyn LlmTaskObserver,
    ) -> Result<LlmCompletionResponse, LlmRuntimeError> {
        let mut failed_attempts = 0u32;
        loop {
            let mut emitted_any = false;
            let mut emit_delta = |delta: LlmStreamDelta| {
                emitted_any = true;
                observer
                    .on_event(LlmTaskEvent::Text(super::LlmTaskTextPayload {
                        task_id: task_id.to_string(),
                        task_type: super::LlmTaskType::Summary,
                        text: delta.text,
                        delta: delta.delta,
                        reset: false,
                    }))
                    .map_err(|message| LlmPortError::new(LlmPortErrorKind::Protocol, message))
            };
            let result = LlmRuntimeService::new(&self.runtime, self.runtime.clone())
                .stream(request.clone(), &mut emit_delta)
                .await;
            match result {
                Ok(mut response) => {
                    response.execution.attempts = failed_attempts + 1;
                    return Ok(response);
                }
                Err(error) => {
                    failed_attempts += 1;
                    let Some(delay) = llm_task_retry_delay(&error, failed_attempts) else {
                        return Err(error);
                    };
                    if emitted_any {
                        observer
                            .on_event(LlmTaskEvent::Text(super::LlmTaskTextPayload {
                                task_id: task_id.to_string(),
                                task_type: super::LlmTaskType::Summary,
                                text: String::new(),
                                delta: String::new(),
                                reset: true,
                            }))
                            .map_err(|message| {
                                LlmRuntimeError::from(LlmPortError::new(
                                    LlmPortErrorKind::Protocol,
                                    message,
                                ))
                            })?;
                    }
                    self.runtime.delay(delay).await;
                }
            }
        }
    }

    async fn translate_with_retry(
        &self,
        request: LlmTranslationRequest,
    ) -> Result<Vec<String>, LlmRuntimeError> {
        let mut failed_attempts = 0u32;
        loop {
            match self.runtime.translate_batch(request.clone()).await {
                Ok(result) => return Ok(result),
                Err(error) => {
                    let runtime_error = LlmRuntimeError::from(error);
                    failed_attempts += 1;
                    let Some(delay) = llm_task_retry_delay(&runtime_error, failed_attempts) else {
                        return Err(runtime_error);
                    };
                    self.runtime.delay(delay).await;
                }
            }
        }
    }
}

pub fn llm_task_retry_delay(error: &LlmRuntimeError, failed_attempt: u32) -> Option<Duration> {
    if failed_attempt == 0 || failed_attempt > MAX_NETWORK_RETRIES {
        return None;
    }
    let LlmRuntimeError::Adapter {
        kind,
        retry_after_ms,
        ..
    } = error
    else {
        return None;
    };
    if !matches!(
        kind,
        LlmPortErrorKind::RateLimited
            | LlmPortErrorKind::Timeout
            | LlmPortErrorKind::Unavailable
            | LlmPortErrorKind::Network
    ) {
        return None;
    }

    let delay_ms = retry_after_ms
        .map(|value| value.min(MAX_RETRY_AFTER_MS))
        .unwrap_or(DEFAULT_RETRY_DELAYS_MS[(failed_attempt - 1) as usize]);
    Some(Duration::from_millis(delay_ms))
}

fn validate_segment_inputs(segments: &[super::LlmSegmentInput]) -> Result<(), LlmTaskError> {
    let mut ids = HashSet::with_capacity(segments.len());
    for segment in segments {
        if segment.id.trim().is_empty() {
            return Err(LlmTaskError::InvalidRequest {
                reason: "Segment ID cannot be empty".to_string(),
            });
        }
        if !ids.insert(segment.id.as_str()) {
            return Err(LlmTaskError::InvalidRequest {
                reason: format!("Duplicate segment ID '{}'", segment.id),
            });
        }
    }
    Ok(())
}

fn task_cache_policy(total_chunks: usize) -> LlmPromptCachePolicy {
    if total_chunks > 1 {
        LlmPromptCachePolicy::Automatic
    } else {
        LlmPromptCachePolicy::Disabled
    }
}

fn summary_reduce_output_tokens(budget: super::LlmTaskBudget) -> Option<u64> {
    let target = budget
        .prompt_token_budget
        .map(|tokens| (tokens / 4).max(1))
        .unwrap_or(1024)
        .min(DEFAULT_SUMMARY_MAP_OUTPUT_TOKENS);
    Some(
        budget
            .max_output_tokens
            .map(|limit| limit.min(target))
            .unwrap_or(target)
            .max(1),
    )
}

fn structured_request(
    config: LlmConfig,
    system_prompt: &'static str,
    input: String,
    response_format: LlmResponseFormat,
    prompt_cache: LlmPromptCachePolicy,
    max_output_tokens: Option<u64>,
) -> LlmCompletionRequest {
    LlmCompletionRequest {
        config,
        system_prompt: Some(system_prompt.to_string()),
        input,
        options: LlmCompletionOptions {
            max_output_tokens,
            response_format,
            prompt_cache,
            capability_policy: LlmCapabilityPolicy::Compatible,
            ..LlmCompletionOptions::default()
        },
        source: None,
    }
}

fn text_request(
    config: LlmConfig,
    system_prompt: &'static str,
    input: String,
    prompt_cache: LlmPromptCachePolicy,
    max_output_tokens: Option<u64>,
) -> LlmCompletionRequest {
    structured_request(
        config,
        system_prompt,
        input,
        LlmResponseFormat::Text,
        prompt_cache,
        max_output_tokens,
    )
}

fn parse_polish_response(
    response: &LlmCompletionResponse,
    expected: &[super::LlmSegmentInput],
    chunk_number: usize,
) -> Result<Vec<super::PolishedSegment>, String> {
    let value = response
        .json
        .as_ref()
        .ok_or_else(|| "structured response did not include parsed JSON".to_string())?;
    super::parse_polish_object(value, expected, chunk_number)
}

fn parse_translate_response(
    response: &LlmCompletionResponse,
    expected: &[super::LlmSegmentInput],
    chunk_number: usize,
) -> Result<Vec<super::TranslatedSegment>, String> {
    let value = response
        .json
        .as_ref()
        .ok_or_else(|| "structured response did not include parsed JSON".to_string())?;
    super::parse_translate_object(value, expected, chunk_number)
}

fn runtime_error(stage: &str, source: LlmRuntimeError) -> LlmTaskError {
    LlmTaskError::Runtime {
        stage: stage.to_string(),
        source,
    }
}

fn emit(observer: &dyn LlmTaskObserver, event: LlmTaskEvent) -> Result<(), LlmTaskError> {
    observer
        .on_event(event)
        .map_err(|reason| LlmTaskError::Observer { reason })
}

fn emit_progress(
    observer: &dyn LlmTaskObserver,
    task_id: &str,
    task_type: super::LlmTaskType,
    completed_chunks: usize,
    total_chunks: usize,
) -> Result<(), LlmTaskError> {
    emit(
        observer,
        LlmTaskEvent::Progress(super::LlmTaskProgressPayload {
            task_id: task_id.to_string(),
            task_type,
            completed_chunks,
            total_chunks,
        }),
    )
}
