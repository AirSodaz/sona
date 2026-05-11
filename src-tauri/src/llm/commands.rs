use super::network::{validate_llm_api_host, LlmApiUrl};
use super::*;
use futures_util::future::BoxFuture;
use log::info;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

fn polished_segment_id(item: &PolishedSegment) -> &str {
    item.id.as_str()
}

fn translated_segment_id(item: &TranslatedSegment) -> &str {
    item.id.as_str()
}

#[derive(Clone)]
struct CommandEventEmitter {
    app: AppHandle,
}

impl CommandEventEmitter {
    fn new(app: AppHandle) -> Self {
        Self { app }
    }

    fn emit<T>(&self, event: &str, payload: T) -> Result<(), String>
    where
        T: Serialize + Clone,
    {
        self.app
            .emit(event, payload)
            .map_err(|error| error.to_string())
    }

    fn emit_progress(&self, payload: LlmTaskProgressPayload) -> Result<(), String> {
        self.emit(LLM_TASK_PROGRESS_EVENT, payload)
    }

    fn emit_chunk<T>(&self, payload: LlmTaskChunkPayload<T>) -> Result<(), String>
    where
        T: Serialize + Clone,
    {
        self.emit(LLM_TASK_CHUNK_EVENT, payload)
    }

    fn emit_summary_text(&self, task_id: &str, text: &str, delta: &str) -> Result<(), String> {
        self.emit(
            LLM_TASK_TEXT_EVENT,
            LlmTaskTextPayload {
                task_id: task_id.to_string(),
                task_type: LlmTaskType::Summary,
                text: text.to_string(),
                delta: delta.to_string(),
            },
        )
    }
}

#[derive(Clone)]
struct UsageRecorder {
    app: AppHandle,
    config: LlmConfig,
    category: LlmUsageCategory,
}

impl UsageRecorder {
    fn new(app: AppHandle, config: LlmConfig, category: LlmUsageCategory) -> Self {
        Self {
            app,
            config,
            category,
        }
    }

    fn record(&self, response: &StandardLlmResponse) {
        let occurred_at = chrono::Utc::now().to_rfc3339();
        if let Err(error) = llm_usage::record_usage(
            &self.app,
            llm_usage::UsageRecord {
                occurred_at: occurred_at.clone(),
                provider: self.config.provider,
                category: self.category,
                usage: response.usage.clone(),
            },
        ) {
            log::warn!(
                "[LLM] failed to persist usage: provider={:?} category={:?} error={}",
                self.config.provider,
                self.category,
                error
            );
        }

        emit_llm_usage_event(
            &self.app,
            &self.config,
            self.category,
            occurred_at,
            response.usage.clone(),
        );
    }
}

pub(crate) async fn generate_llm_text_command(
    app: AppHandle,
    request: LlmGenerateRequest,
) -> Result<String, String> {
    validate_llm_config(&request.config)?;

    if request.input.trim().is_empty() {
        return Err("Input cannot be empty".to_string());
    }

    let category = request.source.unwrap_or(LlmGenerateSource::Generic).into();
    let usage = UsageRecorder::new(app, request.config.clone(), category);
    let response = generate_with_rig(request).await?;
    usage.record(&response);
    Ok(response.text)
}

pub(crate) async fn polish_transcript_segments_command(
    app: AppHandle,
    request: PolishSegmentsRequest,
) -> Result<Vec<PolishedSegment>, String> {
    polish_transcript_segments_with_observer(app, request, |_| Ok(())).await
}

pub(super) async fn polish_transcript_segments_with_observer<F>(
    app: AppHandle,
    request: PolishSegmentsRequest,
    on_chunk_items: F,
) -> Result<Vec<PolishedSegment>, String>
where
    F: FnMut(&[PolishedSegment]) -> Result<(), String> + Send + 'static,
{
    validate_task_request(&request.task_id, &request.config)?;

    if request.config.provider == LlmProvider::GoogleTranslate {
        return Err("Google Translate does not support transcript polishing".to_string());
    }

    let config = request.config.clone();
    let context = request.context.clone();
    let keywords = request.keywords.clone();
    let chunk_events = CommandEventEmitter::new(app.clone());
    let progress_events = CommandEventEmitter::new(app.clone());
    let usage = UsageRecorder::new(app, request.config.clone(), LlmUsageCategory::Polish);
    let on_chunk_items = std::sync::Arc::new(std::sync::Mutex::new(on_chunk_items));

    run_streaming_segment_task(
        SegmentTaskContext {
            task_id: &request.task_id,
            task_type: LlmTaskType::Polish,
            segments: &request.segments,
            chunk_size: request.chunk_size,
            prompt_char_budget: DEFAULT_SEGMENT_PROMPT_CHAR_BUDGET,
        },
        StreamingSegmentTaskConfig {
            build_prompt: move |chunk: &[LlmSegmentInput]| {
                build_polish_prompt(chunk, context.as_deref(), keywords.as_deref())
            },
            parse_chunk: parse_polish_chunk,
            build_request: move |prompt: String| LlmGenerateRequest {
                config: config.clone(),
                input: prompt,
                source: None,
            },
            get_output_id: polished_segment_id,
            on_success: move |response: &StandardLlmResponse| usage.record(response),
            emit_chunk: move |payload: LlmTaskChunkPayload<PolishedSegment>| {
                {
                    let mut on_chunk_items =
                        on_chunk_items.lock().map_err(|error| error.to_string())?;
                    (on_chunk_items)(&payload.items)?;
                }
                chunk_events.emit_chunk(payload)
            },
            emit_progress: move |payload| progress_events.emit_progress(payload),
        },
    )
    .await
}

pub(crate) async fn translate_transcript_segments_command(
    app: AppHandle,
    request: TranslateSegmentsRequest,
) -> Result<Vec<TranslatedSegment>, String> {
    translate_transcript_segments_with_observer(app, request, |_| Ok(())).await
}

pub(super) async fn translate_transcript_segments_with_observer<F>(
    app: AppHandle,
    request: TranslateSegmentsRequest,
    on_chunk_items: F,
) -> Result<Vec<TranslatedSegment>, String>
where
    F: FnMut(&[TranslatedSegment]) -> Result<(), String> + Send + 'static,
{
    validate_task_request(&request.task_id, &request.config)?;

    if request.target_language.trim().is_empty() {
        return Err("Target language cannot be empty".to_string());
    }

    let config = request.config.clone();
    let target_language = request.target_language.clone();
    let events = CommandEventEmitter::new(app.clone());
    let usage = UsageRecorder::new(app, request.config.clone(), LlmUsageCategory::Translation);
    let on_chunk_items = std::sync::Arc::new(std::sync::Mutex::new(on_chunk_items));

    if config.provider == LlmProvider::GoogleTranslate
        || config.provider == LlmProvider::GoogleTranslateFree
    {
        let translate_provider = config.provider;
        let base_url = LlmApiUrl::parse(&config.base_url)?;
        let client = base_url.client()?;
        let chunk_events = events.clone();
        let progress_events = events.clone();
        let usage = usage.clone();
        let chunk_observer = on_chunk_items.clone();
        return run_segment_task(
            SegmentTaskContext {
                task_id: &request.task_id,
                task_type: LlmTaskType::Translate,
                segments: &request.segments,
                chunk_size: request.chunk_size,
                prompt_char_budget: DEFAULT_SEGMENT_PROMPT_CHAR_BUDGET,
            },
            BufferedSegmentTaskConfig {
                build_prompt: move |chunk: &[LlmSegmentInput]| {
                    let texts: Vec<String> = chunk.iter().map(|s| s.text.clone()).collect();
                    serde_json::to_string(&texts).unwrap_or_default()
                },
                parse_chunk: move |response_text: &str,
                                   chunk: &[LlmSegmentInput],
                                   chunk_number: usize| {
                    if translate_provider == LlmProvider::GoogleTranslate {
                        let parsed: GoogleTranslateResponse =
                            serde_json::from_str(response_text).map_err(|error| {
                                chunk_error(
                                    LlmTaskType::Translate,
                                    chunk_number,
                                    format!("invalid JSON response: {error}"),
                                )
                            })?;

                        if parsed.data.translations.len() != chunk.len() {
                            return Err(chunk_error(
                                LlmTaskType::Translate,
                                chunk_number,
                                format!(
                                    "expected {} objects but received {}",
                                    chunk.len(),
                                    parsed.data.translations.len()
                                ),
                            ));
                        }

                        let mut translated_segments = Vec::with_capacity(chunk.len());
                        for (index, translation) in parsed.data.translations.into_iter().enumerate()
                        {
                            translated_segments.push(TranslatedSegment {
                                id: chunk[index].id.clone(),
                                translation: translation.translated_text,
                            });
                        }

                        Ok(translated_segments)
                    } else {
                        // GoogleTranslateFree returns raw JSON array of translations
                        let translations: Vec<String> =
                            serde_json::from_str(response_text).map_err(|e| {
                                chunk_error(
                                    LlmTaskType::Translate,
                                    chunk_number,
                                    format!("invalid free translation JSON: {e}"),
                                )
                            })?;

                        if translations.len() != chunk.len() {
                            return Err(chunk_error(
                                LlmTaskType::Translate,
                                chunk_number,
                                format!(
                                    "expected {} translations but received {}",
                                    chunk.len(),
                                    translations.len()
                                ),
                            ));
                        }

                        Ok(chunk
                            .iter()
                            .zip(translations)
                            .map(|(s, t)| TranslatedSegment {
                                id: s.id.clone(),
                                translation: t,
                            })
                            .collect())
                    }
                },
                generate_text: move |prompt: String| {
                    let config = config.clone();
                    let target_language = target_language.clone();
                    let client = client.clone();
                    let base_url = base_url.clone();
                    let future: BoxFuture<'static, Result<StandardLlmResponse, String>> =
                        Box::pin(async move {
                            let texts: Vec<String> =
                                serde_json::from_str(&prompt).unwrap_or_default();

                            if config.provider == LlmProvider::GoogleTranslate {
                                let payload = GoogleTranslateRequest {
                                    q: texts,
                                    target: target_language,
                                    format: "text".to_string(),
                                };

                                let url = base_url.clone();
                                let response = client
                                    .post(url.reqwest_url())
                                    .header("x-goog-api-key", config.api_key)
                                    .json(&payload)
                                    .send()
                                    .await
                                    .map_err(|e| e.to_string())?;

                                let status = response.status();
                                let text = response.text().await.unwrap_or_default();

                                if !status.is_success() {
                                    return Err(format!(
                                        "Google Translate API Error: {} {}",
                                        status, text
                                    ));
                                }

                                Ok(StandardLlmResponse { text, usage: None })
                            } else {
                                info!(
                                    "[LLM] google_translate_free chunk fan-out: items={} max_concurrency={}",
                                    texts.len(),
                                    GOOGLE_TRANSLATE_FREE_MAX_CONCURRENCY
                                );
                                let translations = run_google_translate_free_requests_in_order(
                                    texts,
                                    GOOGLE_TRANSLATE_FREE_MAX_CONCURRENCY,
                                    move |index, text| {
                                        let client = client.clone();
                                        let base_url = base_url.clone();
                                        let target_language = target_language.clone();
                                        async move {
                                            execute_google_translate_free_request(
                                                index,
                                                text,
                                                target_language,
                                                move |text, target_language| {
                                                    let client = client.clone();
                                                    let base_url = base_url.clone();
                                                    async move {
                                                        fetch_google_translate_free_translation(
                                                            &client,
                                                            &base_url,
                                                            target_language.as_str(),
                                                            text.as_str(),
                                                        )
                                                        .await
                                                    }
                                                },
                                                |delay| async move {
                                                    tokio::time::sleep(delay).await;
                                                },
                                            )
                                            .await
                                        }
                                    },
                                )
                                .await?;
                                Ok(StandardLlmResponse {
                                    text: serde_json::to_string(&translations)
                                        .unwrap_or_default(),
                                    usage: None,
                                })
                            }
                        });
                    future
                },
                on_success: move |response: &StandardLlmResponse| usage.record(response),
                emit_chunk: move |payload: LlmTaskChunkPayload<TranslatedSegment>| {
                    {
                        let mut on_chunk_items =
                            chunk_observer.lock().map_err(|error| error.to_string())?;
                        (on_chunk_items)(&payload.items)?;
                    }
                    chunk_events.emit_chunk(payload)
                },
                emit_progress: move |payload| progress_events.emit_progress(payload),
            },
        )
        .await;
    }

    let chunk_events = events.clone();
    let progress_events = events;
    let chunk_observer = on_chunk_items.clone();
    run_streaming_segment_task(
        SegmentTaskContext {
            task_id: &request.task_id,
            task_type: LlmTaskType::Translate,
            segments: &request.segments,
            chunk_size: request.chunk_size,
            prompt_char_budget: DEFAULT_SEGMENT_PROMPT_CHAR_BUDGET,
        },
        StreamingSegmentTaskConfig {
            build_prompt: move |chunk: &[LlmSegmentInput]| {
                build_translate_prompt(chunk, &target_language)
            },
            parse_chunk: parse_translate_chunk,
            build_request: move |prompt: String| LlmGenerateRequest {
                config: config.clone(),
                input: prompt,
                source: None,
            },
            get_output_id: translated_segment_id,
            on_success: move |response: &StandardLlmResponse| usage.record(response),
            emit_chunk: move |payload: LlmTaskChunkPayload<TranslatedSegment>| {
                {
                    let mut on_chunk_items =
                        chunk_observer.lock().map_err(|error| error.to_string())?;
                    (on_chunk_items)(&payload.items)?;
                }
                chunk_events.emit_chunk(payload)
            },
            emit_progress: move |payload| progress_events.emit_progress(payload),
        },
    )
    .await
}

pub(crate) async fn summarize_transcript_command(
    app: AppHandle,
    request: SummarizeTranscriptRequest,
) -> Result<TranscriptSummaryResult, String> {
    validate_task_request(&request.task_id, &request.config)?;
    validate_summary_provider(request.config.provider)?;

    let task_id = request.task_id.clone();
    let streamed_task_id = task_id.clone();
    let buffered_config = request.config.clone();
    let streamed_config = request.config.clone();
    let template = request.template;
    let progress_events = CommandEventEmitter::new(app.clone());
    let stream_events = progress_events.clone();
    let buffered_usage = UsageRecorder::new(
        app.clone(),
        request.config.clone(),
        LlmUsageCategory::Summary,
    );
    let streamed_usage = buffered_usage.clone();

    run_summary_task(
        &task_id,
        &template,
        &request.segments,
        request.chunk_char_budget,
        move |prompt| {
            let config = buffered_config.clone();
            let usage = buffered_usage.clone();
            Box::pin(async move {
                let response = generate_with_rig(LlmGenerateRequest {
                    config,
                    input: prompt,
                    source: None,
                })
                .await?;
                usage.record(&response);
                Ok(response.text)
            })
        },
        move |prompt| {
            let config = streamed_config.clone();
            let task_id = streamed_task_id.clone();
            let events = stream_events.clone();
            let usage = streamed_usage.clone();
            Box::pin(async move {
                let response = generate_with_optional_streaming(
                    LlmGenerateRequest {
                        config,
                        input: prompt,
                        source: None,
                    },
                    &mut |text, delta| events.emit_summary_text(task_id.as_str(), text, delta),
                )
                .await?;
                usage.record(&response);
                Ok(response.text)
            })
        },
        move |payload| progress_events.emit_progress(payload),
    )
    .await
}

pub(crate) async fn list_llm_models_command(
    request: LlmModelsRequest,
) -> Result<Vec<String>, String> {
    if !provider_supports_model_listing(&request.provider) {
        return Ok(vec![]);
    }
    validate_llm_api_host(&request.base_url)?;

    let base_url = LlmApiUrl::parse(&request.base_url)?;
    let client = base_url.client()?;

    match request.provider {
        LlmProvider::Gemini => get_gemini_models(&client, &request.api_key, &base_url).await,
        LlmProvider::Ollama => get_openai_models(&client, &request.api_key, &base_url, true).await,
        _ => get_openai_models(&client, &request.api_key, &base_url, false).await,
    }
}
