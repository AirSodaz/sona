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

        let candidate_paths = [
            models_dir.join(&preset.filename),
            models_dir.join(&preset.id).join(&preset.filename),
        ];

        let mut found_path = None;
        let mut found_size = None;
        for path in &candidate_paths {
            if path.is_file()
                && let Ok(meta) = std::fs::metadata(path)
            {
                found_path = Some(path.to_string_lossy().into_owned());
                found_size = Some(meta.len());
                break;
            }
        }

        let is_installed = found_path.is_some();
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
            {
                let stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(file_name);
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
                    languages: vec!["auto".to_string()],
                    capabilities: vec![
                        "chat".to_string(),
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

fn extract_quantization_from_filename(filename: &str) -> Option<String> {
    let lower = filename.to_lowercase();
    let quants = [
        "q4_k_m", "q4_k_s", "q4_0", "q4_1", "q5_k_m", "q5_k_s", "q5_0", "q5_1", "q8_0", "q2_k",
        "q3_k_m", "q6_k", "f16", "f32",
    ];
    for q in quants {
        if lower.contains(q) {
            return Some(q.to_uppercase());
        }
    }
    None
}
