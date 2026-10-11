use crate::services::DesktopServices;
use sona_core::history::store::HistoryStore;
use sona_core::history::{
    HistorySummaryPayload, HistoryWorkspaceDateFilter, HistoryWorkspaceFilterType,
    HistoryWorkspaceQueryRequest, HistoryWorkspaceQueryResult, HistoryWorkspaceScope,
    HistoryWorkspaceSortOrder, TranscriptSummaryRecordPayload,
};

use super::types::{
    HistoryItemSummary, QueryHistoryRequest, QueryTrashRequest, SaveSummaryRequest,
    UpdateHistoryMetaRequest,
};

pub async fn query_history(
    services: &DesktopServices,
    req: QueryHistoryRequest,
) -> Result<Vec<HistoryItemSummary>, String> {
    let scope = if let Some(pid) = req.project_id {
        HistoryWorkspaceScope::Project { project_id: pid }
    } else {
        HistoryWorkspaceScope::All
    };

    let query_req = HistoryWorkspaceQueryRequest {
        scope,
        query: req.query,
        filter_type: HistoryWorkspaceFilterType::All,
        date_filter: HistoryWorkspaceDateFilter::All,
        sort_order: HistoryWorkspaceSortOrder::Newest,
        limit: req.limit.unwrap_or(20),
        offset: req.offset.unwrap_or(0),
    };

    let result: HistoryWorkspaceQueryResult = services
        .history
        .query_db(move |service| service.query_workspace(query_req))
        .await
        .map_err(|e| e.to_string())?;

    let summaries = result
        .filtered_items
        .into_iter()
        .map(|item| HistoryItemSummary {
            id: item.id,
            title: item.title,
            preview_text: item.preview_text,
            timestamp: item.timestamp,
            duration: item.duration,
            project_id: item.project_id,
        })
        .collect();

    Ok(summaries)
}

pub async fn delete_history(
    services: &DesktopServices,
    history_id: String,
    permanent: bool,
) -> Result<bool, String> {
    let hid = history_id.clone();
    if permanent {
        services
            .history
            .mutation_file(move |service| {
                service.purge_items(
                    sona_core::history::mutation_repository::HistoryDeleteItemsRequest {
                        ids: vec![hid],
                    },
                )
            })
            .await
            .map_err(|e| e.to_string())?;
    } else {
        let deleted_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_millis() as u64;
        services
            .history
            .mutation_file(move |service| {
                service.trash_items(
                    sona_core::history::mutation_repository::HistoryTrashItemsRequest {
                        ids: vec![hid],
                        deleted_at,
                    },
                )
            })
            .await
            .map_err(|e| e.to_string())?;
    }

    let _ = services.emitter.emit(
        "history-item-deleted",
        serde_json::json!({ "historyId": history_id, "permanent": permanent }),
    );

    Ok(true)
}

pub async fn update_history_meta(
    services: &DesktopServices,
    history_id: String,
    req: UpdateHistoryMetaRequest,
) -> Result<bool, String> {
    if req.title.is_some() || req.icon.is_some() {
        let patch = sona_core::history::mutation_repository::HistoryItemMetaPatch {
            title: req.title.clone(),
            icon: req.icon.clone().map(Some),
            ..Default::default()
        };
        let req_meta = sona_core::history::mutation_repository::HistoryUpdateItemMetaRequest {
            history_id: history_id.clone(),
            updates: patch,
        };
        services
            .history
            .mutation_db(move |service| service.update_item_meta(req_meta))
            .await
            .map_err(|e| e.to_string())?;
    }

    if let Some(raw_pid) = req.project_id {
        let normalized_pid = match raw_pid.trim() {
            "" => None,
            non_empty => Some(non_empty.to_string()),
        };
        let req_proj =
            sona_core::history::mutation_repository::HistoryUpdateProjectAssignmentsRequest {
                ids: vec![history_id.clone()],
                project_id: normalized_pid,
            };
        services
            .history
            .mutation_db(move |service| service.update_project_assignments(req_proj))
            .await
            .map_err(|e| e.to_string())?;
    }

    let _ = services.emitter.emit(
        "history-item-updated",
        serde_json::json!({ "id": &history_id }),
    );

    Ok(true)
}

pub async fn query_trash(
    services: &DesktopServices,
    req: QueryTrashRequest,
) -> Result<Vec<HistoryItemSummary>, String> {
    let query_req = HistoryWorkspaceQueryRequest {
        scope: HistoryWorkspaceScope::Trash,
        query: req.query,
        filter_type: HistoryWorkspaceFilterType::All,
        date_filter: HistoryWorkspaceDateFilter::All,
        sort_order: HistoryWorkspaceSortOrder::Newest,
        limit: req.limit.unwrap_or(20),
        offset: req.offset.unwrap_or(0),
    };

    let result: HistoryWorkspaceQueryResult = services
        .history
        .query_db(move |service| service.query_workspace(query_req))
        .await
        .map_err(|e| e.to_string())?;

    let summaries = result
        .filtered_items
        .into_iter()
        .map(|item| HistoryItemSummary {
            id: item.id,
            title: item.title,
            preview_text: item.preview_text,
            timestamp: item.timestamp,
            duration: item.duration,
            project_id: item.project_id,
        })
        .collect();

    Ok(summaries)
}

pub async fn restore_history(
    services: &DesktopServices,
    history_id: String,
) -> Result<bool, String> {
    let hid = history_id.clone();
    let req = sona_core::history::mutation_repository::HistoryDeleteItemsRequest { ids: vec![hid] };
    services
        .history
        .mutation_file(move |service| service.restore_items(req))
        .await
        .map_err(|e| e.to_string())?;

    let _ = services.emitter.emit(
        "history-item-restored",
        serde_json::json!({ "historyId": &history_id }),
    );

    Ok(true)
}

pub async fn save_summary(
    services: &DesktopServices,
    req: SaveSummaryRequest,
) -> Result<bool, String> {
    let hid = req.history_id.clone();
    let segments_opt = services
        .history
        .query_db(move |repo| repo.load_transcript(&hid))
        .await
        .map_err(|e| e.to_string())?;

    let source_fingerprint = match &segments_opt {
        Some(segs) => sona_core::llm::jobs::compute_summary_source_fingerprint(segs),
        None => String::new(),
    };

    // Retain existing template_id and thought if not provided
    let existing = load_summary(services, req.history_id.clone())
        .await
        .ok()
        .flatten();
    let existing_record = existing.as_ref().and_then(|s| s.record.as_ref());
    let template_id = req
        .template_id
        .or_else(|| existing.as_ref().map(|s| s.active_template_id.clone()))
        .unwrap_or_else(|| "default".to_string());
    let thought = req
        .thought
        .or_else(|| existing_record.and_then(|r| r.thought.clone()));

    let generated_at = chrono::Utc::now().to_rfc3339();

    let payload = HistorySummaryPayload {
        active_template_id: template_id.clone(),
        record: Some(TranscriptSummaryRecordPayload {
            template_id,
            content: req.content,
            thought,
            generated_at,
            source_fingerprint,
        }),
    };

    let target_hid = req.history_id.clone();
    services
        .history
        .db_task(move |repo| repo.save_summary(&target_hid, payload))
        .await
        .map_err(|e| e.to_string())?;

    let _ = services.emitter.emit(
        "summary-updated",
        serde_json::json!({ "historyId": &req.history_id }),
    );

    Ok(true)
}

pub async fn delete_summary(
    services: &DesktopServices,
    history_id: String,
) -> Result<bool, String> {
    let target_hid = history_id.clone();
    services
        .history
        .db_task(move |repo| repo.delete_summary(&target_hid))
        .await
        .map_err(|e| e.to_string())?;

    let _ = services.emitter.emit(
        "summary-updated",
        serde_json::json!({ "historyId": &history_id }),
    );

    Ok(true)
}

pub async fn load_summary(
    services: &DesktopServices,
    history_id: String,
) -> Result<Option<HistorySummaryPayload>, String> {
    services
        .history
        .db_task(move |repo| repo.load_summary(&history_id))
        .await
        .map_err(|e| e.to_string())
}
