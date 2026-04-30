fn normalize_chunk_size(chunk_size: Option<usize>) -> usize {
    chunk_size
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_SEGMENT_CHUNK_SIZE)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlannedSegmentChunk {
    start: usize,
    end: usize,
    prompt: String,
}

struct SegmentTaskContext<'a> {
    task_id: &'a str,
    task_type: LlmTaskType,
    segments: &'a [LlmSegmentInput],
    chunk_size: Option<usize>,
    prompt_char_budget: usize,
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

struct BufferedSegmentTaskConfig<
    BuildPrompt,
    ParseChunk,
    GenerateFn,
    OnSuccessFn,
    EmitChunkFn,
    EmitProgressFn,
> {
    build_prompt: BuildPrompt,
    parse_chunk: ParseChunk,
    generate_text: GenerateFn,
    on_success: OnSuccessFn,
    emit_chunk: EmitChunkFn,
    emit_progress: EmitProgressFn,
}

struct StreamingSegmentTaskConfig<
    BuildPrompt,
    ParseChunk,
    BuildRequest,
    GetId,
    OnSuccessFn,
    EmitChunkFn,
    EmitProgressFn,
> {
    build_prompt: BuildPrompt,
    parse_chunk: ParseChunk,
    build_request: BuildRequest,
    get_output_id: GetId,
    on_success: OnSuccessFn,
    emit_chunk: EmitChunkFn,
    emit_progress: EmitProgressFn,
}

#[derive(Debug, Clone)]
enum GoogleTranslateFreeAttemptError {
    HttpStatus {
        status: StatusCode,
        retry_after: Option<Duration>,
    },
    Message(String),
}

fn prompt_char_count(prompt: &str) -> usize {
    prompt.chars().count()
}

fn plan_segment_task_chunks<BuildPrompt>(
    task_id: &str,
    task_type: LlmTaskType,
    segments: &[LlmSegmentInput],
    chunk_size: Option<usize>,
    prompt_char_budget: usize,
    build_prompt: &mut BuildPrompt,
) -> Vec<PlannedSegmentChunk>
where
    BuildPrompt: FnMut(&[LlmSegmentInput]) -> String,
{
    if segments.is_empty() {
        return Vec::new();
    }

    if chunk_size.is_some() {
        // An explicit chunk-size override wins over dynamic planning so callers
        // can force deterministic segment counts when needed.
        let normalized_chunk_size = normalize_chunk_size(chunk_size);
        let mut planned = Vec::with_capacity(segments.len().div_ceil(normalized_chunk_size));

        for start in (0..segments.len()).step_by(normalized_chunk_size) {
            let end = (start + normalized_chunk_size).min(segments.len());
            planned.push(PlannedSegmentChunk {
                start,
                end,
                prompt: build_prompt(&segments[start..end]),
            });
        }

        info!(
            "[LLM] {} task {} planned {} chunk(s) with fixed segment override: chunk_size={}",
            task_label(task_type),
            task_id,
            planned.len(),
            normalized_chunk_size
        );

        return planned;
    }

    let prompt_char_budget = prompt_char_budget.max(1);
    let mut planned = Vec::new();
    let mut chunk_start = 0usize;
    let mut chunk_end = 0usize;
    let mut current_prompt: Option<String> = None;

    while chunk_start < segments.len() {
        let candidate_end = chunk_end + 1;
        let candidate_prompt = build_prompt(&segments[chunk_start..candidate_end]);
        let candidate_prompt_chars = prompt_char_count(&candidate_prompt);

        if candidate_prompt_chars <= prompt_char_budget {
            // Grow the current chunk greedily until adding one more segment
            // would exceed the prompt budget.
            chunk_end = candidate_end;
            current_prompt = Some(candidate_prompt);

            if chunk_end == segments.len() {
                planned.push(PlannedSegmentChunk {
                    start: chunk_start,
                    end: chunk_end,
                    prompt: current_prompt
                        .take()
                        .expect("accepted chunk should always have a prompt"),
                });
                break;
            }

            continue;
        }

        if chunk_end == chunk_start {
            // If even a single segment exceeds budget, we still send it alone
            // instead of dropping it. That preserves task completeness and lets
            // the provider return the real failure, if any.
            warn!(
                "[LLM] {} task {} single segment exceeded prompt budget and will be sent alone: segment_id={} prompt_chars={} prompt_budget_chars={}",
                task_label(task_type),
                task_id,
                segments[chunk_start].id,
                candidate_prompt_chars,
                prompt_char_budget
            );
            planned.push(PlannedSegmentChunk {
                start: chunk_start,
                end: candidate_end,
                prompt: candidate_prompt,
            });
            chunk_start = candidate_end;
            chunk_end = chunk_start;
            current_prompt = None;
            continue;
        }

        planned.push(PlannedSegmentChunk {
            start: chunk_start,
            end: chunk_end,
            prompt: current_prompt
                .take()
                .expect("previous accepted chunk should always have a prompt"),
        });
        let next_chunk_start = chunk_end;
        chunk_start = next_chunk_start;
        chunk_end = next_chunk_start;
    }

    info!(
        "[LLM] {} task {} planned {} chunk(s) with dynamic prompt budget: context_budget_chars={} prompt_budget_chars={}",
        task_label(task_type),
        task_id,
        planned.len(),
        DEFAULT_SEGMENT_CONTEXT_CHAR_BUDGET,
        prompt_char_budget
    );

    planned
}

fn format_attempt_label(attempts: usize) -> &'static str {
    if attempts == 1 {
        "attempt"
    } else {
        "attempts"
    }
}

fn parse_google_translate_free_retry_after(
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

    if let Some(outer_arr) = body.as_array() {
        if let Some(inner_arr) = outer_arr.get(0).and_then(|value| value.as_array()) {
            for part in inner_arr {
                if let Some(text) = part.get(0).and_then(|value| value.as_str()) {
                    translated.push_str(text);
                }
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

async fn fetch_google_translate_free_translation(
    client: &Client,
    base_url: &str,
    target_language: &str,
    text: &str,
) -> Result<String, GoogleTranslateFreeAttemptError> {
    let url = format!(
        "{}?client=gtx&sl=auto&tl={}&dt=t&q={}",
        base_url.trim_end_matches('/'),
        target_language,
        urlencoding::encode(text)
    );
    let response = client
        .get(&url)
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

async fn execute_google_translate_free_request<FetchFn, FetchFuture, SleepFn, SleepFuture>(
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

async fn run_google_translate_free_requests_in_order<RunFn, RunFuture>(
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
        LlmTaskType::Summary => "summary",
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
    let parsed = parse_json_array_or_ndjson::<PolishedSegment>(
        response_text,
        LlmTaskType::Polish,
        chunk_number,
    )?;
    validate_segment_ids(
        &parsed,
        expected,
        LlmTaskType::Polish,
        chunk_number,
        |item| &item.id,
    )?;
    Ok(parsed)
}

fn parse_translate_chunk(
    response_text: &str,
    expected: &[LlmSegmentInput],
    chunk_number: usize,
) -> Result<Vec<TranslatedSegment>, String> {
    let parsed = parse_json_array_or_ndjson::<TranslatedSegment>(
        response_text,
        LlmTaskType::Translate,
        chunk_number,
    )?;
    validate_segment_ids(
        &parsed,
        expected,
        LlmTaskType::Translate,
        chunk_number,
        |item| &item.id,
    )?;
    Ok(parsed)
}

fn build_polish_prompt(
    segments: &[LlmSegmentInput],
    context: Option<&str>,
    keywords: Option<&str>,
) -> String {
    let json_str = serde_json::to_string(segments).unwrap_or_else(|_| "[]".to_string());
    let mut prompt = String::new();

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
    prompt.push_str(
        "1. Output newline-delimited JSON (NDJSON) only. Do not wrap the result in a JSON array.\n",
    );
    prompt.push_str("2. Each output line must be one valid JSON object. Do not include markdown formatting like ```json.\n");
    prompt.push_str(
        "3. Return the EXACT SAME 'id' field, and the polished text in the 'text' field.\n",
    );
    prompt.push_str(&format!(
        "4. Do not combine or split segments. There must be exactly {} JSON lines in the output.\n\n",
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
1. Output newline-delimited JSON (NDJSON) only. Do not wrap the result in a JSON array.\n\
2. Each output line must be one valid JSON object. Do not include markdown formatting like ```json.\n\
3. Return objects with the EXACT SAME 'id' field, but replace 'text' with 'translation'.\n\
4. Do not combine or split segments. There must be exactly {} JSON lines in the output.\n\n\
Input:\n\
{}",
        language_name(target_language),
        segments.len(),
        json_str
    )
}

fn format_summary_timestamp(seconds: f32) -> String {
    let total_seconds = seconds.max(0.0).floor() as u64;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let secs = total_seconds % 60;
    format!("{hours:02}:{minutes:02}:{secs:02}")
}

fn format_summary_segments_for_prompt(segments: &[SummarySegmentInput]) -> String {
    segments
        .iter()
        .filter_map(|segment| {
            let text = segment.text.trim();
            if text.is_empty() {
                return None;
            }

            Some(format!(
                "[{} - {}] {}",
                format_summary_timestamp(segment.start),
                format_summary_timestamp(segment.end),
                text
            ))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_summary_chunk_prompt(
    template: &SummaryTemplateConfig,
    segments: &[SummarySegmentInput],
    chunk_number: usize,
    total_chunks: usize,
) -> String {
    // Phase 1 of long-summary generation: summarize one transcript slice into a
    // mergeable intermediate result that can later be combined with siblings.
    format!(
        "You are preparing an intermediate transcript summary using the \"{template_name}\" template.\n\
Use the same language as the transcript. Do not translate or switch languages.\n\
Follow this structure:\n\
{structure}\n\n\
This is chunk {chunk_number} of {total_chunks}. Capture only information supported by this chunk.\n\
Keep it concise, factual, and easy to merge later.\n\
Do not use markdown code fences.\n\n\
Transcript chunk:\n\
{chunk_text}",
        template_name = template.name.trim(),
        structure = template.instructions.trim(),
        chunk_number = chunk_number,
        total_chunks = total_chunks,
        chunk_text = format_summary_segments_for_prompt(segments),
    )
}

fn build_summary_finalize_prompt(
    template: &SummaryTemplateConfig,
    partial_summaries: &[String],
) -> String {
    // Phase 2 of long-summary generation: merge intermediate summaries into a
    // single final summary while removing overlap across chunks.
    format!(
        "You are combining intermediate transcript summaries into one final summary using the \"{template_name}\" template.\n\
Use the same language as the transcript. Do not translate or switch languages.\n\
Follow this structure:\n\
{structure}\n\n\
Merge overlapping points, keep the wording concise, and preserve only information supported by the intermediate summaries.\n\
Do not use markdown code fences.\n\n\
Intermediate summaries:\n\
{partials}",
        template_name = template.name.trim(),
        structure = template.instructions.trim(),
        partials = partial_summaries
            .iter()
            .enumerate()
            .map(|(index, item)| format!("[Chunk {}]\n{}", index + 1, item.trim()))
            .collect::<Vec<_>>()
            .join("\n\n"),
    )
}

fn build_summary_direct_prompt(
    template: &SummaryTemplateConfig,
    segments: &[SummarySegmentInput],
) -> String {
    // Short transcripts skip the chunk/finalize pipeline and go straight to one
    // final-summary prompt.
    format!(
        "You are creating a final transcript summary using the \"{template_name}\" template.\n\
Use the same language as the transcript. Do not translate or switch languages.\n\
Follow this structure:\n\
{structure}\n\n\
Keep the summary concise, factual, and limited to information supported by the transcript.\n\
Do not use markdown code fences.\n\n\
Transcript:\n\
{chunk_text}",
        template_name = template.name.trim(),
        structure = template.instructions.trim(),
        chunk_text = format_summary_segments_for_prompt(segments),
    )
}

fn normalize_summary_chunk_char_budget(chunk_char_budget: Option<usize>) -> usize {
    chunk_char_budget
        .unwrap_or(DEFAULT_SUMMARY_CHUNK_CHAR_BUDGET)
        .max(MIN_SUMMARY_CHUNK_CHAR_BUDGET)
}

fn estimate_summary_segment_chars(segment: &SummarySegmentInput) -> usize {
    // This is a cheap character heuristic, not token accounting. We only need a
    // stable approximation to keep prompt sizes bounded before model calls.
    segment.text.chars().count() + 24
}

fn split_summary_segments(
    segments: &[SummarySegmentInput],
    chunk_char_budget: usize,
) -> Vec<Vec<SummarySegmentInput>> {
    if segments.is_empty() {
        return Vec::new();
    }

    let mut chunks = Vec::new();
    let mut current_chunk = Vec::new();
    let mut current_chars = 0usize;

    for segment in segments {
        let segment_chars = estimate_summary_segment_chars(segment);

        if !current_chunk.is_empty() && current_chars + segment_chars > chunk_char_budget {
            // Start a new chunk before we exceed the rough prompt budget. The
            // budget is intentionally approximate; exact token limits are left
            // to the provider/model layer.
            chunks.push(current_chunk);
            current_chunk = Vec::new();
            current_chars = 0;
        }

        current_chars += segment_chars;
        current_chunk.push(segment.clone());
    }

    if !current_chunk.is_empty() {
        chunks.push(current_chunk);
    }

    chunks
}

fn validate_summary_provider(provider: LlmProvider) -> Result<(), String> {
    if provider == LlmProvider::GoogleTranslate || provider == LlmProvider::GoogleTranslateFree {
        return Err("Google Translate does not support transcript summaries".to_string());
    }

    Ok(())
}

fn summary_task_error(stage: impl AsRef<str>, error: impl Into<String>) -> String {
    format!("summary {} failed: {}", stage.as_ref(), error.into())
}

async fn run_summary_task<GenerateFn, GenerateStreamFn, EmitProgressFn>(
    task_id: &str,
    template: &SummaryTemplateConfig,
    segments: &[SummarySegmentInput],
    chunk_char_budget: Option<usize>,
    mut generate_text: GenerateFn,
    mut generate_streamed_text: GenerateStreamFn,
    mut emit_progress: EmitProgressFn,
) -> Result<TranscriptSummaryResult, String>
where
    GenerateFn: FnMut(String) -> BoxFuture<'static, Result<String, String>>,
    GenerateStreamFn: FnMut(String) -> BoxFuture<'static, Result<String, String>>,
    EmitProgressFn: FnMut(LlmTaskProgressPayload) -> Result<(), String>,
{
    if segments.is_empty() {
        return Err("Transcript cannot be empty".to_string());
    }

    let normalized_budget = normalize_summary_chunk_char_budget(chunk_char_budget);
    let chunks = split_summary_segments(segments, normalized_budget);
    let stream_direct_final = chunks.len() == 1;
    let total_chunks = if stream_direct_final {
        1
    } else {
        chunks.len() + 1
    };

    if stream_direct_final {
        let final_prompt = build_summary_direct_prompt(template, &chunks[0]);
        let final_summary = generate_streamed_text(final_prompt)
            .await
            .map_err(|error| summary_task_error("final summary", error))?;

        emit_progress(LlmTaskProgressPayload {
            task_id: task_id.to_string(),
            task_type: LlmTaskType::Summary,
            completed_chunks: total_chunks,
            total_chunks,
        })?;

        return Ok(TranscriptSummaryResult {
            template_id: template.id.clone(),
            content: final_summary.trim().to_string(),
        });
    }

    let mut partial_summaries = Vec::with_capacity(chunks.len());

    for (chunk_index, chunk) in chunks.iter().enumerate() {
        let chunk_number = chunk_index + 1;
        let prompt = build_summary_chunk_prompt(template, chunk, chunk_number, chunks.len());
        let intermediate_summary = generate_text(prompt)
            .await
            .map_err(|error| summary_task_error(format!("chunk {}", chunk_number), error))?;

        partial_summaries.push(intermediate_summary.trim().to_string());

        emit_progress(LlmTaskProgressPayload {
            task_id: task_id.to_string(),
            task_type: LlmTaskType::Summary,
            completed_chunks: chunk_number,
            total_chunks,
        })?;
    }

    let final_prompt = build_summary_finalize_prompt(template, &partial_summaries);
    let final_summary = generate_streamed_text(final_prompt)
        .await
        .map_err(|error| summary_task_error("final synthesis", error))?;

    emit_progress(LlmTaskProgressPayload {
        task_id: task_id.to_string(),
        task_type: LlmTaskType::Summary,
        completed_chunks: total_chunks,
        total_chunks,
    })?;

    Ok(TranscriptSummaryResult {
        template_id: template.id.clone(),
        content: final_summary.trim().to_string(),
    })
}

async fn run_segment_task<
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

async fn run_streaming_segment_task<
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
        drop(emit_streamed_line);

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
