use crate::domain::{BuiltinLlmProvider, LlmProvider};
use futures_util::future::BoxFuture;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[cfg(feature = "specta")]
use specta::Type;

pub const DEFAULT_SEGMENT_CHUNK_SIZE: usize = 30;
pub const DEFAULT_SEGMENT_CONTEXT_CHAR_BUDGET: usize = 32_000;
pub const DEFAULT_SEGMENT_PROMPT_CHAR_BUDGET: usize = DEFAULT_SEGMENT_CONTEXT_CHAR_BUDGET / 2;
pub const DEFAULT_SUMMARY_CHUNK_CHAR_BUDGET: usize = 6000;
pub const MIN_SUMMARY_CHUNK_CHAR_BUDGET: usize = 1200;

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "snake_case")]
pub enum LlmProviderStrategy {
    OpenAi,
    OpenAiResponses,
    #[serde(rename = "azure_openai")]
    AzureOpenAi,
    Anthropic,
    Gemini,
    Ollama,
    DeepSeek,
    MoonshotAi,
    MoonshotCn,
    Xiaomi,
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
    Copilot,
    #[serde(rename = "google_translate")]
    GoogleTranslate,
    #[serde(rename = "google_translate_free")]
    GoogleTranslateFree,
    OpenAiCompatible,
    OpenAiCompatibleCustomPath,
}

impl LlmProviderStrategy {
    pub fn from_provider(provider: &LlmProvider) -> Self {
        match provider {
            LlmProvider::Custom(_) => Self::OpenAiCompatible,
            LlmProvider::Builtin(b) => match b {
                BuiltinLlmProvider::OpenAi => Self::OpenAi,
                BuiltinLlmProvider::OpenAiResponses => Self::OpenAiResponses,
                BuiltinLlmProvider::AzureOpenai => Self::AzureOpenAi,
                BuiltinLlmProvider::Anthropic => Self::Anthropic,
                BuiltinLlmProvider::Gemini => Self::Gemini,
                BuiltinLlmProvider::Ollama => Self::Ollama,
                BuiltinLlmProvider::MoonshotAi => Self::MoonshotAi,
                BuiltinLlmProvider::MoonshotCn => Self::MoonshotCn,
                BuiltinLlmProvider::Xiaomi => Self::Xiaomi,
                BuiltinLlmProvider::Perplexity => Self::Perplexity,
                BuiltinLlmProvider::Copilot => Self::Copilot,
                BuiltinLlmProvider::Volcengine => Self::OpenAiCompatibleCustomPath,
                BuiltinLlmProvider::GoogleTranslate => Self::GoogleTranslate,
                BuiltinLlmProvider::GoogleTranslateFree => Self::GoogleTranslateFree,
                _ => Self::OpenAiCompatible,
            },
        }
    }
}

impl<'de> Deserialize<'de> for LlmProviderStrategy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "open_ai" => Self::OpenAi,
            "open_ai_responses" | "openai_responses" => Self::OpenAiResponses,
            "azure_openai" => Self::AzureOpenAi,
            "anthropic" => Self::Anthropic,
            "gemini" => Self::Gemini,
            "ollama" => Self::Ollama,
            "deep_seek" => Self::DeepSeek,
            "kimi" => Self::Kimi,
            "silicon_flow" => Self::SiliconFlow,
            "qwen" => Self::Qwen,
            "qwen_portal" => Self::QwenPortal,
            "minimax_global" => Self::MinimaxGlobal,
            "minimax_cn" => Self::MinimaxCn,
            "openrouter" | "open_router" => Self::OpenRouter,
            "lm_studio" => Self::LmStudio,
            "groq" => Self::Groq,
            "x_ai" => Self::XAi,
            "mistral_ai" => Self::MistralAi,
            "perplexity" => Self::Perplexity,
            "volcengine" => Self::Volcengine,
            "chatglm" => Self::Chatglm,
            "copilot" | "github_copilot" => Self::Copilot,
            "google_translate" => Self::GoogleTranslate,
            "google_translate_free" => Self::GoogleTranslateFree,
            "open_ai_compatible" | "openai_compatible" => Self::OpenAiCompatible,
            "open_ai_compatible_custom_path" | "openai_compatible_custom_path" => {
                Self::OpenAiCompatibleCustomPath
            }
            _ => Self::OpenAiCompatible,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "snake_case")]
pub enum LlmTaskType {
    Polish,
    Translate,
    Summary,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct SummaryTemplateConfig {
    pub id: String,
    pub name: String,
    pub instructions: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct LlmSegmentInput {
    pub id: String,
    pub text: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct SummarySegmentInput {
    pub id: String,
    pub text: String,
    pub start: f32,
    pub end: f32,
    pub is_final: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct PolishedSegment {
    pub id: String,
    pub text: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct TranslatedSegment {
    pub id: String,
    pub translation: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSummaryResult {
    pub template_id: String,
    pub content: String,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct LlmTaskProgressPayload {
    pub task_id: String,
    pub task_type: LlmTaskType,
    pub completed_chunks: usize,
    pub total_chunks: usize,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct LlmTaskChunkPayload<T> {
    pub task_id: String,
    pub task_type: LlmTaskType,
    pub chunk_index: usize,
    pub total_chunks: usize,
    pub items: Vec<T>,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct LlmTaskTextPayload {
    pub task_id: String,
    pub task_type: LlmTaskType,
    pub text: String,
    pub delta: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedSegmentChunk {
    pub start: usize,
    pub end: usize,
    pub prompt: String,
}

fn normalize_chunk_size(chunk_size: Option<usize>) -> usize {
    chunk_size
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_SEGMENT_CHUNK_SIZE)
}

pub fn prompt_char_count(prompt: &str) -> usize {
    prompt.chars().count()
}

pub fn plan_segment_task_chunks<BuildPrompt>(
    _task_id: &str,
    _task_type: LlmTaskType,
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

    planned
}

pub fn clean_json_response(response_text: &str) -> String {
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

pub fn chunk_error(
    task_type: LlmTaskType,
    chunk_number: usize,
    error: impl Into<String>,
) -> String {
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

pub fn normalize_incremental_json_line(line: &str) -> Option<String> {
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

pub fn parse_json_array_or_ndjson<T: DeserializeOwned>(
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

pub fn parse_polish_chunk(
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

pub fn parse_translate_chunk(
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

pub fn build_polish_prompt(
    segments: &[LlmSegmentInput],
    context: Option<&str>,
    keywords: Option<&str>,
) -> String {
    let json_str = serde_json::to_string(segments).unwrap_or_else(|_| "[]".to_string());
    let mut prompt = String::new();

    if let Some(value) = context
        && !value.trim().is_empty()
    {
        prompt.push_str("[User Context]\n");
        prompt.push_str(value.trim());
        prompt.push_str("\n\n");
    }

    if let Some(value) = keywords
        && !value.trim().is_empty()
    {
        prompt.push_str("[User Keywords]\n");
        prompt.push_str(value.trim());
        prompt.push_str("\n\n");
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

pub fn build_translate_prompt(
    segments: &[LlmSegmentInput],
    target_language: &str,
    target_language_name: Option<&str>,
) -> String {
    let json_str = serde_json::to_string(segments).unwrap_or_else(|_| "[]".to_string());
    let resolved_name = target_language_name.unwrap_or(target_language);

    format!(
        "You are a professional translator. Translate the following array of text segments into {}.\n\
CRITICAL INSTRUCTIONS:\n\
1. Output newline-delimited JSON (NDJSON) only. Do not wrap the result in a JSON array.\n\
2. Each output line must be one valid JSON object. Do not include markdown formatting like ```json.\n\
3. Return objects with the EXACT SAME 'id' field, but replace 'text' with 'translation'.\n\
4. Do not combine or split segments. There must be exactly {} JSON lines in the output.\n\n\
Input:\n\
{}",
        resolved_name,
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

pub fn build_summary_chunk_prompt(
    template: &SummaryTemplateConfig,
    segments: &[SummarySegmentInput],
    chunk_number: usize,
    total_chunks: usize,
) -> String {
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

pub fn build_summary_finalize_prompt(
    template: &SummaryTemplateConfig,
    partial_summaries: &[String],
) -> String {
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
    segment.text.chars().count() + 24
}

pub fn split_summary_segments(
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

pub fn validate_summary_strategy(strategy: LlmProviderStrategy) -> Result<(), String> {
    if matches!(
        strategy,
        LlmProviderStrategy::GoogleTranslate | LlmProviderStrategy::GoogleTranslateFree
    ) {
        return Err("Google Translate does not support transcript summaries".to_string());
    }

    Ok(())
}

fn summary_task_error(stage: impl AsRef<str>, error: impl Into<String>) -> String {
    format!("summary {} failed: {}", stage.as_ref(), error.into())
}

pub async fn run_summary_task<GenerateFn, GenerateStreamFn, EmitProgressFn>(
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
