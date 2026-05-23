mod commands;
mod jobs;
#[path = "llm_usage.rs"]
pub(crate) mod llm_usage;
mod network;
mod providers;
mod streaming;
mod tasks;
#[cfg(test)]
mod tests;
mod types;

const DEFAULT_SEGMENT_CHUNK_SIZE: usize = 30;
const DEFAULT_SEGMENT_CONTEXT_CHAR_BUDGET: usize = 32_000;
const DEFAULT_SEGMENT_PROMPT_CHAR_BUDGET: usize = DEFAULT_SEGMENT_CONTEXT_CHAR_BUDGET / 2;
const GOOGLE_TRANSLATE_FREE_MAX_CONCURRENCY: usize = 2;
const GOOGLE_TRANSLATE_FREE_MAX_RETRIES: usize = 2;
const GOOGLE_TRANSLATE_FREE_MAX_RETRY_AFTER_SECS: u64 = 5;
const GOOGLE_TRANSLATE_FREE_RETRY_DELAYS_MS: [u64; GOOGLE_TRANSLATE_FREE_MAX_RETRIES] = [500, 1000];
const DEFAULT_SUMMARY_CHUNK_CHAR_BUDGET: usize = 6000;
const MIN_SUMMARY_CHUNK_CHAR_BUDGET: usize = 1200;
const LLM_TASK_PROGRESS_EVENT: &str = "llm-task-progress";
const LLM_TASK_CHUNK_EVENT: &str = "llm-task-chunk";
const LLM_TASK_TEXT_EVENT: &str = "llm-task-text";
const LLM_TRANSCRIPT_JOB_UPDATE_EVENT: &str = "llm-transcript-job-update";
const LLM_USAGE_RECORDED_EVENT: &str = "llm-usage-recorded";

pub(crate) use providers::*;
pub(crate) use streaming::*;
pub(crate) use tasks::*;
pub use types::*;

#[tauri::command]
pub async fn generate_llm_text(
    app: tauri::AppHandle,
    request: LlmGenerateRequest,
) -> Result<String, String> {
    commands::generate_llm_text_command(app, request).await
}

#[tauri::command]
pub async fn polish_transcript_segments(
    app: tauri::AppHandle,
    request: PolishSegmentsRequest,
) -> Result<Vec<PolishedSegment>, String> {
    commands::polish_transcript_segments_command(app, request).await
}

#[tauri::command]
pub async fn translate_transcript_segments(
    app: tauri::AppHandle,
    request: TranslateSegmentsRequest,
) -> Result<Vec<TranslatedSegment>, String> {
    commands::translate_transcript_segments_command(app, request).await
}

#[tauri::command]
pub async fn summarize_transcript(
    app: tauri::AppHandle,
    request: SummarizeTranscriptRequest,
) -> Result<TranscriptSummaryResult, String> {
    commands::summarize_transcript_command(app, request).await
}

#[tauri::command]
pub async fn run_transcript_llm_job(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::history_repository::HistoryRepositoryState>,
    request: TranscriptLlmJobRequest,
) -> Result<TranscriptLlmJobResult, String> {
    jobs::run_transcript_llm_job_command(app, state, request).await
}

#[tauri::command]
pub async fn list_llm_models(request: LlmModelsRequest) -> Result<Vec<LlmModelSummary>, String> {
    commands::list_llm_models_command(request).await
}
