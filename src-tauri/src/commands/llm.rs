use tauri::{AppHandle, State};
use crate::integrations::llm::{
    LlmGenerateRequest, PolishSegmentsRequest, PolishedSegment,
    TranslateSegmentsRequest, TranslatedSegment, SummarizeTranscriptRequest,
    TranscriptSummaryResult, TranscriptLlmJobRequest, TranscriptLlmJobResult,
    LlmModelsRequest, LlmModelSummary,
};
use crate::repositories::history::HistoryRepositoryState;

#[tauri::command]
pub async fn generate_llm_text(
    app: AppHandle,
    request: LlmGenerateRequest,
) -> Result<String, String> {
    crate::integrations::llm::commands::generate_llm_text_command(app, request).await
}

#[tauri::command]
pub async fn polish_transcript_segments(
    app: AppHandle,
    request: PolishSegmentsRequest,
) -> Result<Vec<PolishedSegment>, String> {
    crate::integrations::llm::commands::polish_transcript_segments_command(app, request).await
}

#[tauri::command]
pub async fn translate_transcript_segments(
    app: AppHandle,
    request: TranslateSegmentsRequest,
) -> Result<Vec<TranslatedSegment>, String> {
    crate::integrations::llm::commands::translate_transcript_segments_command(app, request).await
}

#[tauri::command]
pub async fn summarize_transcript(
    app: AppHandle,
    request: SummarizeTranscriptRequest,
) -> Result<TranscriptSummaryResult, String> {
    crate::integrations::llm::commands::summarize_transcript_command(app, request).await
}

#[tauri::command]
pub async fn run_transcript_llm_job(
    app: AppHandle,
    state: State<'_, HistoryRepositoryState>,
    request: TranscriptLlmJobRequest,
) -> Result<TranscriptLlmJobResult, String> {
    crate::integrations::llm::jobs::run_transcript_llm_job_command(app, state, request).await
}

#[tauri::command]
pub async fn list_llm_models(request: LlmModelsRequest) -> Result<Vec<LlmModelSummary>, String> {
    crate::integrations::llm::commands::list_llm_models_command(request).await
}

#[tauri::command]
pub async fn llm_usage_ensure_storage(app: AppHandle) -> Result<(), String> {
    crate::integrations::llm::llm_usage::llm_usage_ensure_storage(app).await
}

#[tauri::command]
pub async fn llm_usage_read_raw(app: AppHandle) -> Result<String, String> {
    crate::integrations::llm::llm_usage::llm_usage_read_raw(app).await
}

#[tauri::command]
pub async fn llm_usage_replace_raw(
    app: AppHandle,
    content: String,
) -> Result<(), String> {
    crate::integrations::llm::llm_usage::llm_usage_replace_raw(app, content).await
}
