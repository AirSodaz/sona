use crate::services::DesktopServices;
use sona_core::export::{ExportFormat, ExportMode};

use super::asr_config::resolve_batch_asr_request_from_config;
use super::types::{
    ExportTranscriptRequest, ExportTranscriptResult, TranscribeFileRequest, TranscribeFileResult,
};

pub async fn transcribe_file(
    services: &DesktopServices,
    req: TranscribeFileRequest,
) -> Result<TranscribeFileResult, String> {
    let _sleep_guard = crate::platform::system::power::SleepPreventionGuard::acquire(
        "Agent Audio Batch Transcription",
    );
    let path = std::path::PathBuf::from(&req.file_path);
    if !path.exists() {
        return Err(format!("File not found: {}", req.file_path));
    }

    let cfg_val = services
        .config
        .load()
        .map_err(|e| format!("Failed to load config: {e}"))?
        .unwrap_or_else(|| serde_json::json!({}));
    let asr_req = resolve_batch_asr_request_from_config(&cfg_val, req.language.as_deref())?;

    let save_to = req.save_to_path.as_ref().map(std::path::PathBuf::from);
    let instance_id = req
        .instance_id
        .unwrap_or_else(|| format!("mcp-transcribe-{}", uuid::Uuid::new_v4()));

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis() as u64;
    let title = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("batch-file")
        .to_string();

    let initial_record = sona_core::task_ledger::types::TaskLedgerRecord {
        id: instance_id.clone(),
        kind: sona_core::task_ledger::types::TaskLedgerKind::BatchImport,
        status: sona_core::task_ledger::types::TaskLedgerStatus::Running,
        title,
        progress: 0.0,
        created_at: now_ms,
        updated_at: now_ms,
        retryable: false,
        cancelable: true,
        recoverable: false,
        stage: Some("transcribing".to_string()),
        history_id: None,
        tag_ids: req.project_id.clone().into_iter().collect(),
        file_path: Some(req.file_path.clone()),
        automation_rule_id: None,
        tag_automation_rule_id: None,
        automation_profile_id: None,
        automation_profile_source: None,
        source_fingerprint: None,
        error_message: None,
        template_id: None,
        target_language: None,
    };
    let _ = services.task_ledger.upsert_task(initial_record).await;

    let segments = match crate::integrations::asr::process_batch_file(
        services.emitter.clone(),
        &services.asr,
        path,
        save_to,
        asr_req,
        None,
        Some(instance_id.clone()),
    )
    .await
    {
        Ok(s) => s,
        Err(e) => {
            let is_cancelled = e.code() == "TASK_CANCELLED" || e.message.contains("Task cancelled");
            let status = if is_cancelled {
                sona_core::task_ledger::types::TaskLedgerStatus::Cancelled
            } else {
                sona_core::task_ledger::types::TaskLedgerStatus::Failed
            };
            let stage = if is_cancelled {
                "cancelled".to_string()
            } else {
                "failed".to_string()
            };
            let err_str = if is_cancelled {
                super::AgentControlError::TaskCancelled(e.message).to_string()
            } else {
                e.to_string()
            };
            let _ = services
                .task_ledger
                .patch_task(
                    instance_id,
                    sona_core::task_ledger::types::TaskLedgerPatch {
                        status: Some(status),
                        error_message: if is_cancelled {
                            None
                        } else {
                            Some(Some(err_str.clone()))
                        },
                        stage: Some(Some(stage)),
                        ..Default::default()
                    },
                )
                .await;
            return Err(err_str);
        }
    };

    let duration_seconds = segments.iter().map(|s| s.end).fold(0.0f64, f64::max);
    let segment_count = segments.len();
    let text = segments
        .iter()
        .map(|s| s.text.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    let save_req = sona_core::history::HistorySaveImportedFileRequest {
        id: None,
        source_path: req.file_path.clone(),
        segments,
        duration: duration_seconds,
        tag_ids: Vec::new(),
        project_id: req.project_id,
        converted_source_path: None,
    };
    let history_item = match services
        .history
        .mutation_file(move |s| s.save_imported_file(save_req))
        .await
    {
        Ok(item) => item,
        Err(e) => {
            let err_str = e.to_string();
            let _ = services
                .task_ledger
                .patch_task(
                    instance_id,
                    sona_core::task_ledger::types::TaskLedgerPatch {
                        status: Some(sona_core::task_ledger::types::TaskLedgerStatus::Failed),
                        error_message: Some(Some(err_str.clone())),
                        stage: Some(Some("failed".to_string())),
                        ..Default::default()
                    },
                )
                .await;
            return Err(err_str);
        }
    };
    let history_id = history_item.id;

    let _ = services
        .task_ledger
        .patch_task(
            instance_id,
            sona_core::task_ledger::types::TaskLedgerPatch {
                status: Some(sona_core::task_ledger::types::TaskLedgerStatus::Succeeded),
                progress: Some(100.0),
                history_id: Some(Some(history_id.clone())),
                stage: Some(Some("completed".to_string())),
                ..Default::default()
            },
        )
        .await;

    let _ = services.emitter.emit(
        "transcript-updated",
        serde_json::json!({ "historyId": &history_id }),
    );
    Ok(TranscribeFileResult {
        history_id,
        duration_seconds,
        segment_count,
        text,
    })
}

pub async fn cancel_batch_task(
    services: &DesktopServices,
    instance_id: String,
) -> Result<bool, String> {
    let cancelled = services.asr.batch_cancel.cancel(&instance_id).await;
    if cancelled {
        let _ = services
            .task_ledger
            .patch_task(
                instance_id,
                sona_core::task_ledger::types::TaskLedgerPatch {
                    status: Some(sona_core::task_ledger::types::TaskLedgerStatus::Cancelled),
                    stage: Some(Some("cancelled".to_string())),
                    ..Default::default()
                },
            )
            .await;
    }
    Ok(cancelled)
}

pub async fn export_transcript(
    services: &DesktopServices,
    req: ExportTranscriptRequest,
) -> Result<ExportTranscriptResult, String> {
    let format = match req.format.trim().to_ascii_lowercase().as_str() {
        "markdown" | "md" => ExportFormat::Md,
        "json" => ExportFormat::Json,
        "txt" => ExportFormat::Txt,
        "srt" => ExportFormat::Srt,
        "vtt" => ExportFormat::Vtt,
        other => return Err(format!("Unsupported export format: {other}")),
    };

    let mode = match req
        .mode
        .as_deref()
        .unwrap_or("original")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "translation" => ExportMode::Translation,
        "bilingual" => ExportMode::Bilingual,
        "original" | "clean" | "" => ExportMode::Original,
        other => return Err(format!("Unsupported export mode: {other}")),
    };

    let hid = req.history_id.clone();
    let segments = services
        .history
        .query_db(move |repo| repo.load_transcript(&hid))
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Transcript not found for history ID: {}", req.history_id))?;

    let segment_count = segments.len();
    let export_req = sona_core::export::ExportTranscriptFileRequest {
        segments,
        format,
        mode,
        output_path: req.output_path.clone(),
        bilingual_order: None,
    };

    tokio::task::spawn_blocking(move || {
        sona_export::export_transcript_file(export_req).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;

    Ok(ExportTranscriptResult {
        success: true,
        output_path: req.output_path,
        format: req.format,
        segment_count,
    })
}
