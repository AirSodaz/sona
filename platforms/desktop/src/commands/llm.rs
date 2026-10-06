use crate::integrations::llm::{
    LlmCompletionRequest, LlmCompletionResponse, LlmConfig, LlmGenerateRequest, LlmModelSummary,
    LlmModelsRequest, PolishSegmentsRequest, PolishedSegment, SummarizeTranscriptRequest,
    TranscriptLlmJobRequest, TranscriptLlmJobResult, TranscriptSummaryResult,
    TranslateSegmentsRequest, TranslatedSegment,
};
use crate::platform::history_repository::HistoryRepositoryState;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn generate_llm_text(
    app: AppHandle,
    request: LlmGenerateRequest,
) -> Result<String, String> {
    crate::integrations::llm::generate_llm_text_command(app, request).await
}

#[tauri::command]
pub async fn complete_llm(
    app: AppHandle,
    request: LlmCompletionRequest,
) -> Result<LlmCompletionResponse, String> {
    crate::integrations::llm::complete_llm_command(app, request).await
}

#[tauri::command]
pub async fn polish_transcript_segments(
    app: AppHandle,
    request: PolishSegmentsRequest,
) -> Result<Vec<PolishedSegment>, String> {
    crate::integrations::llm::polish_transcript_segments_command(app, request).await
}

#[tauri::command]
pub async fn translate_transcript_segments(
    app: AppHandle,
    request: TranslateSegmentsRequest,
) -> Result<Vec<TranslatedSegment>, String> {
    crate::integrations::llm::translate_transcript_segments_command(app, request).await
}

#[tauri::command]
pub async fn summarize_transcript(
    app: AppHandle,
    request: SummarizeTranscriptRequest,
) -> Result<TranscriptSummaryResult, String> {
    crate::integrations::llm::summarize_transcript_command(app, request).await
}

#[tauri::command]
pub async fn run_transcript_llm_job(
    app: AppHandle,
    state: State<'_, HistoryRepositoryState>,
    request: TranscriptLlmJobRequest,
) -> Result<TranscriptLlmJobResult, String> {
    crate::integrations::llm::run_transcript_llm_job_command(app, state, request).await
}

#[tauri::command]
pub async fn list_llm_models(
    app: AppHandle,
    request: LlmModelsRequest,
) -> Result<Vec<LlmModelSummary>, String> {
    let models_dir =
        crate::platform::storage_location::resolve_active_models_dir_for_app(&app).ok();
    crate::integrations::llm::list_llm_models_with_models_dir(request, models_dir).await
}

#[tauri::command]
pub async fn describe_llm_model(
    app: AppHandle,
    config: LlmConfig,
) -> Result<Option<LlmModelSummary>, String> {
    let models_dir =
        crate::platform::storage_location::resolve_active_models_dir_for_app(&app).ok();
    crate::integrations::llm::describe_llm_model_with_models_dir(config, models_dir).await
}

#[tauri::command]
pub async fn llm_usage_ensure_storage(_app: AppHandle) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn llm_usage_read_raw(app: AppHandle) -> Result<String, String> {
    crate::platform::llm_usage::read_raw(&app)
}

#[tauri::command]
pub async fn llm_usage_replace_raw(app: AppHandle, content: String) -> Result<(), String> {
    crate::platform::llm_usage::replace_raw(&app, content)
}

#[tauri::command]
pub async fn list_local_llm_cards(
    app: AppHandle,
) -> Result<sona_core::llm::local_models::LocalLlmCardsResponse, String> {
    let models_dir = crate::platform::storage_location::resolve_active_models_dir_for_app(&app)?;
    Ok(sona_core::llm::local_models::discover_local_llm_cards(
        &models_dir,
    ))
}

#[tauri::command]
pub async fn import_local_llm_file(app: AppHandle, source_path: String) -> Result<String, String> {
    let models_dir = crate::platform::storage_location::resolve_active_models_dir_for_app(&app)?;
    let source = std::path::PathBuf::from(source_path);

    tokio::task::spawn_blocking(move || {
        sona_core::llm::local_models::import_local_llm_file(&models_dir, &source)
            .map(|p| p.to_string_lossy().into_owned())
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("Model import task failed: {e}"))?
}
