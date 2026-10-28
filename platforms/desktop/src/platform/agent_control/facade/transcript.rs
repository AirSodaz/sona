use crate::services::DesktopServices;
use sona_core::history::mutation_repository::{
    HistoryCommitTranscriptEditRequest, HistoryCommitTranscriptEditResult,
};
use sona_core::history::{TranscriptSnapshotMetadata, TranscriptSnapshotReason};
use sona_core::transcription::transcript::TranscriptSegment;

use super::types::{
    EditTranscriptRequest, EditTranscriptResult, PatchSegmentsRequest, PatchSegmentsResult,
    ReadTranscriptRequest, ReadTranscriptResult, RevertTranscriptResult, SegmentPatchInput,
    UpdateTranslationsRequest, UpdateTranslationsResult,
};

pub async fn read_transcript(
    services: &DesktopServices,
    req: ReadTranscriptRequest,
) -> Result<ReadTranscriptResult, String> {
    let (task_status, error_message) = match services.task_ledger.list_tasks().await {
        Ok(tasks) => {
            if let Some(task) = tasks
                .iter()
                .find(|t| t.history_id.as_deref() == Some(&req.history_id))
            {
                let status_str = match task.status {
                    sona_core::task_ledger::types::TaskLedgerStatus::Running => "running",
                    sona_core::task_ledger::types::TaskLedgerStatus::Pending => "pending",
                    sona_core::task_ledger::types::TaskLedgerStatus::CancelRequested => {
                        "cancelling"
                    }
                    sona_core::task_ledger::types::TaskLedgerStatus::Cancelled => "cancelled",
                    sona_core::task_ledger::types::TaskLedgerStatus::Failed => "failed",
                    sona_core::task_ledger::types::TaskLedgerStatus::Succeeded => "completed",
                    sona_core::task_ledger::types::TaskLedgerStatus::Recoverable
                    | sona_core::task_ledger::types::TaskLedgerStatus::Interrupted => "failed",
                };
                (status_str.to_string(), task.error_message.clone())
            } else {
                ("completed".to_string(), None)
            }
        }
        Err(_) => ("completed".to_string(), None),
    };

    let hid = req.history_id.clone();
    let all_segments = services
        .history
        .query_db(move |service| service.load_transcript(&hid))
        .await
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    let total_segments_in_history = all_segments.len();

    let mut filtered: Vec<TranscriptSegment> = all_segments;

    if req.start_seconds.is_some() || req.end_seconds.is_some() {
        let start = req.start_seconds.unwrap_or(0.0);
        let end = req.end_seconds.unwrap_or(f64::INFINITY);
        filtered.retain(|seg| seg.end >= start && seg.start <= end);
    }

    if let Some(since_id) = &req.since_segment_id
        && let Some(idx) = filtered.iter().position(|seg| seg.id == *since_id)
    {
        filtered = filtered.split_off(idx + 1);
    }

    let offset = req.offset.unwrap_or(0);
    let limit = req.limit;
    let filtered_len = filtered.len();

    let (paged, has_more) = if offset >= filtered_len {
        (Vec::new(), false)
    } else {
        let remaining = &filtered[offset..];
        if let Some(lim) = limit {
            if remaining.len() > lim {
                (remaining[..lim].to_vec(), true)
            } else {
                (remaining.to_vec(), false)
            }
        } else {
            (remaining.to_vec(), false)
        }
    };

    let returned_segments = paged.len();

    let text = paged
        .iter()
        .map(|s| s.text.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    let translations: Vec<&str> = paged
        .iter()
        .filter_map(|s| s.translation.as_deref())
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .collect();
    let translation_text = if translations.is_empty() {
        None
    } else {
        Some(sona_core::text::cjk::join_segments_with_boundary_space(
            &translations,
        ))
    };

    Ok(ReadTranscriptResult {
        history_id: req.history_id,
        task_status,
        error_message,
        segments: paged,
        text,
        translation_text,
        total_segments_in_history,
        returned_segments,
        has_more,
    })
}

pub async fn edit_transcript(
    services: &DesktopServices,
    req: EditTranscriptRequest,
) -> Result<EditTranscriptResult, String> {
    let hid = req.history_id.clone();
    let base_segments = services
        .history
        .query_db(move |service| service.load_transcript(&hid))
        .await
        .map_err(|e| e.to_string())?
        .unwrap_or_default();

    let mut base_map: std::collections::HashMap<String, TranscriptSegment> = base_segments
        .iter()
        .map(|s| (s.id.clone(), s.clone()))
        .collect();

    let mut last_end = 0.0;
    let edited_segments: Vec<TranscriptSegment> = req
        .segments
        .into_iter()
        .map(|s| {
            let id = s.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            let base = base_map.remove(&id);
            let text = s
                .text
                .unwrap_or_else(|| base.as_ref().map(|b| b.text.clone()).unwrap_or_default());
            let start = s
                .start
                .unwrap_or_else(|| base.as_ref().map(|b| b.start).unwrap_or(last_end));
            let end = s
                .end
                .unwrap_or_else(|| base.as_ref().map(|b| b.end).unwrap_or(start + 1.0));
            last_end = end;
            let is_final = s
                .is_final
                .unwrap_or_else(|| base.as_ref().map(|b| b.is_final).unwrap_or(true));
            let translation = s
                .translation
                .or_else(|| base.as_ref().and_then(|b| b.translation.clone()));
            let speaker = base.as_ref().and_then(|b| b.speaker.clone());
            let speaker_attribution = base.as_ref().and_then(|b| b.speaker_attribution.clone());
            let timestamps = base.as_ref().and_then(|b| b.timestamps.clone());
            let tokens = base.as_ref().and_then(|b| b.tokens.clone());
            let durations = base.as_ref().and_then(|b| b.durations.clone());
            let timing = base.as_ref().and_then(|b| b.timing.clone());
            TranscriptSegment {
                id,
                text,
                start,
                end,
                is_final,
                timing,
                tokens,
                timestamps,
                durations,
                translation,
                draft_translation: None,
                speaker,
                speaker_attribution,
            }
        })
        .collect();

    let edit_session_id = uuid::Uuid::new_v4().to_string();
    let snapshot_reason = if req
        .reason
        .as_deref()
        .unwrap_or("")
        .to_lowercase()
        .contains("polish")
    {
        Some(TranscriptSnapshotReason::Polish)
    } else {
        None
    };

    let commit_req = HistoryCommitTranscriptEditRequest {
        history_id: req.history_id.clone(),
        edit_session_id,
        base_segments,
        edited_segments,
        reason: snapshot_reason,
    };

    let result = services
        .history
        .mutation_db(move |service| service.commit_transcript_edit(commit_req))
        .await
        .map_err(|e| e.to_string())?;

    let snapshot_id = match result {
        HistoryCommitTranscriptEditResult::Committed { snapshot, .. } => snapshot.id,
        HistoryCommitTranscriptEditResult::Unchanged => {
            format!("unchanged-{}", req.history_id)
        }
        HistoryCommitTranscriptEditResult::Conflict { .. } => {
            return Err("CONFLICT: Transcript was concurrently modified".to_string());
        }
    };

    let _ = services.emitter.emit(
        "transcript-updated",
        serde_json::json!({ "historyId": &req.history_id }),
    );

    Ok(EditTranscriptResult {
        success: true,
        snapshot_id,
    })
}

pub async fn patch_segments(
    services: &DesktopServices,
    req: PatchSegmentsRequest,
) -> Result<PatchSegmentsResult, String> {
    let hid = req.history_id.clone();
    let base_segments = services
        .history
        .query_db(move |service| service.load_transcript(&hid))
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "TRANSCRIPT_NOT_FOUND".to_string())?;

    let remove_set: std::collections::HashSet<String> =
        req.remove_ids.unwrap_or_default().into_iter().collect();

    let mut filtered_segments: Vec<TranscriptSegment> = base_segments
        .iter()
        .filter(|s| !remove_set.contains(&s.id))
        .cloned()
        .collect();

    let mut patch_map: std::collections::HashMap<String, SegmentPatchInput> = req
        .segments
        .into_iter()
        .map(|p| (p.id.clone(), p))
        .collect();

    let mut updated_count = 0;
    for seg in filtered_segments.iter_mut() {
        if let Some(patch) = patch_map.remove(&seg.id) {
            let mut modified = false;
            if let Some(text) = patch.text
                && seg.text != text
            {
                seg.text = text;
                modified = true;
            }
            if let Some(start) = patch.start
                && (seg.start - start).abs() > f64::EPSILON
            {
                seg.start = start;
                modified = true;
            }
            if let Some(end) = patch.end
                && (seg.end - end).abs() > f64::EPSILON
            {
                seg.end = end;
                modified = true;
            }
            if let Some(translation) = patch.translation {
                let trans_opt = if translation.trim().is_empty() {
                    None
                } else {
                    Some(translation)
                };
                if seg.translation != trans_opt {
                    seg.translation = trans_opt;
                    modified = true;
                }
            }
            if modified {
                updated_count += 1;
            }
        }
    }

    let mut last_end = filtered_segments.last().map(|s| s.end).unwrap_or(0.0);
    for (id, patch) in patch_map {
        let start = patch.start.unwrap_or(last_end);
        let end = patch.end.unwrap_or(start + 1.0);
        last_end = end;
        let text = patch.text.unwrap_or_default();
        let translation = patch.translation.filter(|t| !t.trim().is_empty());
        filtered_segments.push(TranscriptSegment {
            id,
            text,
            start,
            end,
            is_final: true,
            timing: None,
            tokens: None,
            timestamps: None,
            durations: None,
            translation,
            draft_translation: None,
            speaker: None,
            speaker_attribution: None,
        });
        updated_count += 1;
    }

    let total_segments = filtered_segments.len();
    let edit_session_id = uuid::Uuid::new_v4().to_string();
    let snapshot_reason = if req
        .reason
        .as_deref()
        .unwrap_or("")
        .to_lowercase()
        .contains("polish")
    {
        Some(TranscriptSnapshotReason::Polish)
    } else {
        None
    };

    let commit_req = HistoryCommitTranscriptEditRequest {
        history_id: req.history_id.clone(),
        edit_session_id,
        base_segments,
        edited_segments: filtered_segments,
        reason: snapshot_reason,
    };

    let result = services
        .history
        .mutation_db(move |service| service.commit_transcript_edit(commit_req))
        .await
        .map_err(|e| e.to_string())?;

    let snapshot_id = match result {
        HistoryCommitTranscriptEditResult::Committed { snapshot, .. } => snapshot.id,
        HistoryCommitTranscriptEditResult::Unchanged => {
            format!("unchanged-{}", req.history_id)
        }
        HistoryCommitTranscriptEditResult::Conflict { .. } => {
            return Err("CONFLICT: Transcript was concurrently modified".to_string());
        }
    };

    let _ = services.emitter.emit(
        "transcript-updated",
        serde_json::json!({ "historyId": &req.history_id }),
    );

    Ok(PatchSegmentsResult {
        success: true,
        snapshot_id,
        updated_count,
        total_segments,
    })
}

pub async fn update_translations(
    services: &DesktopServices,
    req: UpdateTranslationsRequest,
) -> Result<UpdateTranslationsResult, String> {
    let hid = req.history_id.clone();
    let base_segments = services
        .history
        .query_db(move |service| service.load_transcript(&hid))
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "TRANSCRIPT_NOT_FOUND".to_string())?;

    let mut edited_segments = base_segments.clone();
    let mut updated_count = 0;

    if req.clear_all.unwrap_or(false) {
        for seg in edited_segments.iter_mut() {
            if seg.translation.is_some() {
                seg.translation = None;
                updated_count += 1;
            }
        }
    }

    let mut translation_map: std::collections::HashMap<String, Option<String>> = req
        .translations
        .into_iter()
        .map(|item| {
            let val = match item.translation {
                Some(t) if !t.trim().is_empty() => Some(t),
                _ => None,
            };
            (item.id, val)
        })
        .collect();

    for seg in edited_segments.iter_mut() {
        if let Some(trans) = translation_map.remove(&seg.id)
            && seg.translation != trans
        {
            seg.translation = trans;
            updated_count += 1;
        }
    }

    let edit_session_id = uuid::Uuid::new_v4().to_string();
    let commit_req = HistoryCommitTranscriptEditRequest {
        history_id: req.history_id.clone(),
        edit_session_id,
        base_segments,
        edited_segments,
        reason: Some(TranscriptSnapshotReason::Translate),
    };

    let result = services
        .history
        .mutation_db(move |service| service.commit_transcript_edit(commit_req))
        .await
        .map_err(|e| e.to_string())?;

    let snapshot_id = match result {
        HistoryCommitTranscriptEditResult::Committed { snapshot, .. } => snapshot.id,
        HistoryCommitTranscriptEditResult::Unchanged => {
            format!("unchanged-{}", req.history_id)
        }
        HistoryCommitTranscriptEditResult::Conflict { .. } => {
            return Err("CONFLICT: Transcript was concurrently modified".to_string());
        }
    };

    let _ = services.emitter.emit(
        "transcript-updated",
        serde_json::json!({ "historyId": &req.history_id }),
    );

    Ok(UpdateTranslationsResult {
        success: true,
        snapshot_id,
        updated_count,
    })
}

pub async fn list_transcript_snapshots(
    services: &DesktopServices,
    history_id: String,
) -> Result<Vec<TranscriptSnapshotMetadata>, String> {
    let hid = history_id.clone();
    services
        .history
        .query_db(move |repo| repo.list_transcript_snapshots(&hid))
        .await
        .map_err(|e| e.to_string())
}

pub async fn revert_transcript_snapshot(
    services: &DesktopServices,
    history_id: String,
    snapshot_id: String,
) -> Result<RevertTranscriptResult, String> {
    let hid = history_id.clone();
    let sid = snapshot_id.clone();
    let snapshot = services
        .history
        .query_db(move |repo| repo.load_transcript_snapshot(&hid, &sid))
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| {
            format!(
                "SNAPSHOT_NOT_FOUND: Snapshot '{snapshot_id}' not found for history '{history_id}'"
            )
        })?;

    let hid2 = history_id.clone();
    let base_segments = services
        .history
        .query_db(move |repo| repo.load_transcript(&hid2))
        .await
        .map_err(|e| e.to_string())?
        .unwrap_or_default();

    let edited_segments = snapshot.segments;
    let segment_count = edited_segments.len();
    let edit_session_id = uuid::Uuid::new_v4().to_string();

    let commit_req = HistoryCommitTranscriptEditRequest {
        history_id: history_id.clone(),
        edit_session_id,
        base_segments,
        edited_segments,
        reason: Some(TranscriptSnapshotReason::Restore),
    };

    let result = services
        .history
        .mutation_db(move |service| service.commit_transcript_edit(commit_req))
        .await
        .map_err(|e| e.to_string())?;

    let new_snapshot_id = match result {
        HistoryCommitTranscriptEditResult::Committed { snapshot, .. } => snapshot.id,
        HistoryCommitTranscriptEditResult::Unchanged => {
            format!("unchanged-{}", history_id)
        }
        HistoryCommitTranscriptEditResult::Conflict { .. } => {
            return Err("CONFLICT: Transcript was concurrently modified".to_string());
        }
    };

    let _ = services.emitter.emit(
        "transcript-updated",
        serde_json::json!({ "historyId": &history_id }),
    );

    Ok(RevertTranscriptResult {
        success: true,
        snapshot_id: new_snapshot_id,
        segment_count,
    })
}
