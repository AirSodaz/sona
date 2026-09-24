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
    let mut cards = Vec::new();
    let mut seen_filenames = std::collections::HashSet::new();

    // 1. Process known presets
    for preset in sona_core::llm::local_models::local_llm_models() {
        seen_filenames.insert(preset.filename.to_lowercase());

        let found_path_buf = preset.find_installed_path(&models_dir);
        let found_size = found_path_buf
            .as_ref()
            .and_then(|p| std::fs::metadata(p).ok())
            .map(|m| m.len());
        let is_installed = found_path_buf.is_some();
        let found_path = found_path_buf.map(|p| p.to_string_lossy().into_owned());
        cards.push(preset.to_model_card(is_installed, found_path, found_size));
    }

    // 2. Discover custom GGUF models in models_dir
    if let Ok(entries) = std::fs::read_dir(&models_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file()
                && let Some(ext) = path.extension().and_then(|e| e.to_str())
                && ext.eq_ignore_ascii_case("gguf")
                && let Some(file_name) = path.file_name().and_then(|f| f.to_str())
                && !seen_filenames.contains(&file_name.to_lowercase())
                && !sona_core::llm::local_models::is_non_llm_model_file(file_name)
            {
                let stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(file_name);
                if sona_core::llm::local_models::is_non_llm_model_file(stem) {
                    continue;
                }
                let file_size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                let formatted_size =
                    format!("{:.1} GB", file_size as f64 / (1024.0 * 1024.0 * 1024.0));
                let quant = extract_quantization_from_filename(file_name);
                cards.push(sona_core::llm::local_models::LocalLlmModelCard {
                    id: format!("custom-{}", stem),
                    name: stem.to_string(),
                    model: stem.to_string(),
                    filename: file_name.to_string(),
                    description: "settings.descriptions.custom_local_model".to_string(),
                    backend: "llama.cpp".to_string(),
                    context_window: 131072,
                    max_output_tokens: 4096,
                    size: formatted_size,
                    parameters: None,
                    quantization: quant,
                    modalities: vec!["text".to_string()],
                    languages: vec!["auto".to_string()],
                    capabilities: vec![
                        "polish".to_string(),
                        "summary".to_string(),
                        "translate".to_string(),
                    ],
                    is_recommended: false,
                    is_installed: true,
                    installed_path: Some(path.to_string_lossy().into_owned()),
                    installed_size_bytes: Some(file_size),
                    download_url: None,
                    download_size_bytes: None,
                });
            }
        }
    }

    Ok(sona_core::llm::local_models::LocalLlmCardsResponse {
        models_dir: models_dir.to_string_lossy().into_owned(),
        cards,
    })
}
#[tauri::command]
pub async fn import_local_llm_file(app: AppHandle, source_path: String) -> Result<String, String> {
    let source = std::path::PathBuf::from(&source_path);
    if !source.is_file() {
        return Err(format!("File does not exist: {}", source_path));
    }

    let ext = source.extension().and_then(|e| e.to_str()).unwrap_or("");
    if !ext.eq_ignore_ascii_case("gguf") {
        return Err("Only .gguf files can be imported as local LLM models".to_string());
    }

    let file_name = source
        .file_name()
        .and_then(|f| f.to_str())
        .ok_or_else(|| "Invalid source filename".to_string())?
        .to_string();

    let models_dir = crate::platform::storage_location::resolve_active_models_dir_for_app(&app)?;

    tokio::task::spawn_blocking(move || {
        std::fs::create_dir_all(&models_dir).map_err(|e| e.to_string())?;

        let target_path = models_dir.join(&file_name);
        if let (Ok(can_source), Ok(can_target)) =
            (source.canonicalize(), target_path.canonicalize())
            && can_source == can_target
        {
            return Ok(target_path.to_string_lossy().into_owned());
        }

        let temp_target = models_dir.join(format!("{file_name}.importing"));
        std::fs::copy(&source, &temp_target)
            .map_err(|e| format!("Failed to copy model file: {e}"))?;
        std::fs::rename(&temp_target, &target_path).map_err(|e| {
            let _ = std::fs::remove_file(&temp_target);
            format!("Failed to finalize imported model: {e}")
        })?;

        Ok(target_path.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| format!("Model import task failed: {e}"))?
}

fn extract_quantization_from_filename(filename: &str) -> Option<String> {
    let lower = filename.to_lowercase();
    let quants = [
        "q4_k_m", "q4_k_s", "q4_k_l", "q4_0", "q4_1", "q5_k_m", "q5_k_s", "q5_k_l", "q5_0", "q5_1",
        "q8_0", "q2_k", "q3_k_m", "q3_k_s", "q3_k_l", "q6_k", "iq4_nl", "iq4_xs", "iq3_xxs",
        "iq2_xxs", "iq1_s", "f16", "f32", "bf16",
    ];
    for q in quants {
        if lower.contains(q) {
            return Some(q.to_uppercase());
        }
    }
    None
}
