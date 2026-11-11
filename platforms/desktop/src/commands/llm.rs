use crate::integrations::llm::{
    LlmCompletionRequest, LlmCompletionResponse, LlmConfig, LlmGenerateRequest, LlmModelSummary,
    LlmModelsRequest, PolishSegmentsRequest, PolishedSegment, SummarizeTranscriptRequest,
    TranscriptLlmJobRequest, TranscriptLlmJobResult, TranscriptSummaryResult,
    TranslateSegmentsRequest, TranslatedSegment,
};
use crate::services::DesktopServices;
use tauri::State;

#[tauri::command]
pub async fn generate_llm_text(
    services: State<'_, DesktopServices>,
    request: LlmGenerateRequest,
) -> Result<String, String> {
    crate::integrations::llm::generate_llm_text_command(&services, request).await
}

#[tauri::command]
pub async fn complete_llm(
    services: State<'_, DesktopServices>,
    request: LlmCompletionRequest,
) -> Result<LlmCompletionResponse, String> {
    crate::integrations::llm::complete_llm_command(&services, request).await
}

#[tauri::command]
pub async fn polish_transcript_segments(
    services: State<'_, DesktopServices>,
    request: PolishSegmentsRequest,
) -> Result<Vec<PolishedSegment>, String> {
    crate::integrations::llm::polish_transcript_segments_command(&services, request).await
}

#[tauri::command]
pub async fn translate_transcript_segments(
    services: State<'_, DesktopServices>,
    request: TranslateSegmentsRequest,
) -> Result<Vec<TranslatedSegment>, String> {
    crate::integrations::llm::translate_transcript_segments_command(&services, request).await
}

#[tauri::command]
pub async fn summarize_transcript(
    services: State<'_, DesktopServices>,
    request: SummarizeTranscriptRequest,
) -> Result<TranscriptSummaryResult, String> {
    crate::integrations::llm::summarize_transcript_command(&services, request).await
}

#[tauri::command]
pub async fn run_transcript_llm_job(
    services: State<'_, DesktopServices>,
    request: TranscriptLlmJobRequest,
) -> Result<TranscriptLlmJobResult, String> {
    crate::integrations::llm::run_transcript_llm_job_command(&services, request).await
}

#[tauri::command]
pub async fn list_llm_models(
    services: State<'_, DesktopServices>,
    request: LlmModelsRequest,
) -> Result<Vec<LlmModelSummary>, String> {
    let models_dir = Some(services.active_models_dir());
    let cache_dir = Some(services.active_data_dir().join("cache"));
    crate::integrations::llm::list_llm_models_with_dirs(request, models_dir, cache_dir).await
}

#[tauri::command]
pub async fn describe_llm_model(
    services: State<'_, DesktopServices>,
    config: LlmConfig,
) -> Result<Option<LlmModelSummary>, String> {
    let models_dir = Some(services.active_models_dir());
    let cache_dir = Some(services.active_data_dir().join("cache"));
    crate::integrations::llm::describe_llm_model_with_dirs(config, models_dir, cache_dir).await
}

#[tauri::command]
pub async fn purge_local_model_cache(services: State<'_, DesktopServices>) -> Result<(), String> {
    services.local_model_pool.purge_all_idle().await;
    services.asr.clear_model_caches().await;
    Ok(())
}

#[tauri::command]
pub async fn llm_usage_ensure_storage() -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn llm_usage_read_raw(services: State<'_, DesktopServices>) -> Result<String, String> {
    let db = services.sqlite.current_database()?;
    crate::platform::llm_usage::read_raw(db.as_ref())
}

#[tauri::command]
pub async fn llm_usage_replace_raw(
    services: State<'_, DesktopServices>,
    content: String,
) -> Result<(), String> {
    let db = services.sqlite.current_database()?;
    crate::platform::llm_usage::replace_raw(db.as_ref(), content)
}

#[tauri::command]
pub async fn list_local_llm_cards(
    services: State<'_, DesktopServices>,
) -> Result<sona_core::llm::local_models::LocalLlmCardsResponse, String> {
    let models_dir = services.active_models_dir();
    Ok(sona_core::llm::local_models::discover_local_llm_cards(
        &models_dir,
    ))
}

#[tauri::command]
pub async fn import_local_llm_file(
    services: State<'_, DesktopServices>,
    source_path: String,
) -> Result<String, String> {
    let models_dir = services.active_models_dir();
    let source = std::path::PathBuf::from(source_path);

    tokio::task::spawn_blocking(move || {
        sona_core::llm::local_models::import_local_llm_file(&models_dir, &source)
            .map(|p| p.to_string_lossy().into_owned())
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("Model import task failed: {e}"))?
}

#[tauri::command(async)]
pub async fn list_local_nmt_cards(
    services: State<'_, DesktopServices>,
) -> Result<sona_core::llm::local_models::LocalLlmCardsResponse, String> {
    let models_dir = services.active_models_dir();
    crate::platform::blocking::spawn_blocking_map(move || {
        Ok::<_, String>(sona_core::llm::local_models::discover_local_nmt_cards(
            &models_dir,
        ))
    })
    .await
}

#[tauri::command(async)]
pub async fn test_local_nmt_model(
    services: State<'_, DesktopServices>,
    model_id: String,
    text: String,
) -> Result<String, String> {
    let models_dir = services.active_models_dir();
    let lower_id = model_id.trim().to_lowercase();
    let stripped = lower_id.strip_prefix("opus-mt-").unwrap_or(&lower_id);
    let parts: Vec<&str> = stripped.split('-').collect();
    if parts.len() < 2 {
        return Err(format!(
            "Cannot determine language pair from model ID: {model_id}"
        ));
    }
    let src = parts[0];
    let tgt = parts[1];
    let lease = services
        .local_model_pool
        .acquire_onnx_model(src, tgt, &models_dir)
        .map_err(|e| e.to_string())?;
    use sona_core::ports::translation::TranslationEnginePort;
    lease
        .translate(&text, src, tgt)
        .await
        .map_err(|e| e.to_string())
}
