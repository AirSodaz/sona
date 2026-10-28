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
    let is_stream = sona_core::history::is_network_url(&req.file_path);
    let path = std::path::PathBuf::from(&req.file_path);
    if !is_stream && !path.exists() {
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
    let history_id = uuid::Uuid::new_v4().to_string();

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis() as u64;
    let title = if is_stream {
        req.file_path.clone()
    } else {
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("batch-file")
            .to_string()
    };

    let save_req = sona_core::history::HistorySaveImportedFileRequest {
        id: Some(history_id.clone()),
        source_path: req.file_path.clone(),
        segments: vec![],
        duration: 0.0,
        tag_ids: Vec::new(),
        project_id: req.project_id.clone(),
        converted_source_path: None,
    };
    let _ = services
        .history
        .mutation_file(move |s| s.save_imported_file(save_req))
        .await
        .map_err(|e| format!("Failed to create history record: {e}"))?;

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
        history_id: Some(history_id.clone()),
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

    let (cancel_rx, cancel_guard) = services.asr.batch_cancel.register(&instance_id).await;
    let _ = cancel_rx;

    let task_services = services.clone();
    let task_instance_id = instance_id.clone();
    let task_history_id = history_id.clone();
    tokio::spawn(async move {
        let _cancel_guard = cancel_guard;
        let _sleep_guard = crate::platform::system::power::SleepPreventionGuard::acquire(
            "Agent Audio Batch Transcription",
        );
        let result = crate::integrations::asr::process_batch_file(
            task_services.emitter.clone(),
            &task_services.asr,
            path,
            save_to,
            asr_req,
            None,
            Some(task_instance_id.clone()),
        )
        .await;

        match result {
            Ok(segments) => {
                let duration_seconds = segments.iter().map(|s| s.end).fold(0.0f64, f64::max);
                let hid = task_history_id.clone();
                let segs_clone = segments.clone();
                let update_res = task_services
                    .history
                    .mutation_file(move |s| {
                        s.update_transcript(
                            sona_core::history::mutation_repository::HistoryUpdateTranscriptRequest {
                                history_id: hid,
                                segments: segs_clone,
                            },
                        )
                    })
                    .await;
                if let Err(e) = update_res {
                    let err_str = e.to_string();
                    let _ = task_services
                        .task_ledger
                        .patch_task(
                            task_instance_id,
                            sona_core::task_ledger::types::TaskLedgerPatch {
                                status: Some(
                                    sona_core::task_ledger::types::TaskLedgerStatus::Failed,
                                ),
                                error_message: Some(Some(err_str)),
                                stage: Some(Some("failed".to_string())),
                                ..Default::default()
                            },
                        )
                        .await;
                    return;
                }

                let hid_meta = task_history_id.clone();
                let meta_res = task_services
                    .history
                    .mutation_file(move |s| {
                        s.update_item_meta(
                            sona_core::history::mutation_repository::HistoryUpdateItemMetaRequest {
                                history_id: hid_meta,
                                updates:
                                    sona_core::history::mutation_repository::HistoryItemMetaPatch {
                                        duration: Some(duration_seconds),
                                        ..Default::default()
                                    },
                            },
                        )
                    })
                    .await;
                if let Err(e) = meta_res {
                    let err_str = e.to_string();
                    let _ = task_services
                        .task_ledger
                        .patch_task(
                            task_instance_id,
                            sona_core::task_ledger::types::TaskLedgerPatch {
                                status: Some(
                                    sona_core::task_ledger::types::TaskLedgerStatus::Failed,
                                ),
                                error_message: Some(Some(err_str)),
                                stage: Some(Some("failed".to_string())),
                                ..Default::default()
                            },
                        )
                        .await;
                    return;
                }

                let _ = task_services
                    .task_ledger
                    .patch_task(
                        task_instance_id,
                        sona_core::task_ledger::types::TaskLedgerPatch {
                            status: Some(
                                sona_core::task_ledger::types::TaskLedgerStatus::Succeeded,
                            ),
                            progress: Some(100.0),
                            history_id: Some(Some(task_history_id.clone())),
                            stage: Some(Some("completed".to_string())),
                            ..Default::default()
                        },
                    )
                    .await;

                let _ = task_services.emitter.emit(
                    "transcript-updated",
                    serde_json::json!({ "historyId": &task_history_id }),
                );
            }
            Err(e) => {
                let is_cancelled =
                    e.code() == "TASK_CANCELLED" || e.message.contains("Task cancelled");
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

                let hid_to_purge = task_history_id.clone();
                let _ = task_services
                    .history
                    .mutation_file(move |s| {
                        s.purge_items(
                            sona_core::history::mutation_repository::HistoryDeleteItemsRequest {
                                ids: vec![hid_to_purge],
                            },
                        )
                    })
                    .await;

                let _ = task_services
                    .task_ledger
                    .patch_task(
                        task_instance_id,
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
            }
        }
    });

    Ok(TranscribeFileResult {
        history_id,
        task_id: instance_id,
        status: "started".to_string(),
        is_stream,
        message: if is_stream {
            "Stream transcription task started in background".to_string()
        } else {
            "File transcription task started in background".to_string()
        },
        duration_seconds: 0.0,
        segment_count: 0,
        text: String::new(),
    })
}
pub async fn cancel_batch_task(
    services: &DesktopServices,
    instance_id: String,
) -> Result<bool, String> {
    let cancelled = services.asr.batch_cancel.cancel(&instance_id).await;
    let ledger_active = if !cancelled {
        if let Ok(tasks) = services.task_ledger.list_tasks().await {
            tasks.iter().any(|t| {
                t.id == instance_id
                    && matches!(
                        t.status,
                        sona_core::task_ledger::types::TaskLedgerStatus::Running
                            | sona_core::task_ledger::types::TaskLedgerStatus::Pending
                    )
            })
        } else {
            false
        }
    } else {
        false
    };

    if cancelled || ledger_active {
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
        Ok(true)
    } else {
        Ok(false)
    }
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
