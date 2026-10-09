use tauri::{AppHandle, Runtime, State};

use crate::integrations::asr::TranscriptSegment;
use crate::platform::history_repository::{
    BackupManifest, ExportBackupArchiveRequest, HistoryAudioCleanupReport,
    HistoryAudioCleanupRequest, HistoryCreateLiveDraftRequest, HistoryItemRecord,
    HistoryListOptions, HistorySaveImportedFileRequest, HistorySaveRecordingRequest,
    HistoryWorkspaceDateFilter, HistoryWorkspaceFilterType, HistoryWorkspaceQueryRequest,
    HistoryWorkspaceQueryResult, HistoryWorkspaceScope, HistoryWorkspaceSortOrder,
    LiveRecordingDraftResult, PreparedBackupImport, TranscriptDiffResult, TranscriptDiffRow,
    TranscriptSnapshotMetadata, TranscriptSnapshotReason, TranscriptSnapshotRecord,
};
use crate::services::DesktopServices;
use sona_core::history::HistorySummaryPayload;
use sona_core::history::mutation_repository::{
    HistoryCommitTranscriptEditRequest, HistoryCommitTranscriptEditResult,
    HistoryCompleteLiveDraftRequest, HistoryCreateTranscriptSnapshotRequest,
    HistoryDeleteItemsRequest, HistoryItemMetaPatch, HistoryReplaceTagAssignmentsRequest,
    HistoryTrashItemsRequest, HistoryUpdateItemMetaRequest, HistoryUpdateTagAssignmentsRequest,
    HistoryUpdateTranscriptRequest,
};
use sona_core::history_store::HistoryStore;

fn validate_history_input<T: serde::Serialize + ?Sized>(value: &T) -> Result<(), String> {
    sona_ts_bind::validate_typescript_safe_integers(value).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn history_list_items(
    services: State<'_, DesktopServices>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<HistoryItemRecord>, String> {
    validate_history_input(&(limit, offset))?;
    let opts = HistoryListOptions { limit, offset };
    services
        .history
        .query_db(move |service| service.list_items(opts))
        .await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn history_query_workspace(
    services: State<'_, DesktopServices>,
    scope: HistoryWorkspaceScope,
    query: String,
    filter_type: HistoryWorkspaceFilterType,
    date_filter: HistoryWorkspaceDateFilter,
    sort_order: HistoryWorkspaceSortOrder,
    limit: usize,
    offset: usize,
) -> Result<HistoryWorkspaceQueryResult, String> {
    let request = HistoryWorkspaceQueryRequest {
        scope,
        query,
        filter_type,
        date_filter,
        sort_order,
        limit,
        offset,
    };
    validate_history_input(&request)?;
    services
        .history
        .query_db(move |service| service.query_workspace(request))
        .await
}

#[tauri::command]
pub async fn history_create_live_draft(
    services: State<'_, DesktopServices>,
    id: Option<String>,
    audio_extension: String,
    project_id: Option<String>,
    icon: Option<String>,
) -> Result<LiveRecordingDraftResult, String> {
    let request = HistoryCreateLiveDraftRequest {
        id,
        audio_extension,
        tag_ids: Vec::new(),
        project_id,
        icon,
    };
    validate_history_input(&request)?;
    services
        .history
        .mutation_file(move |service| service.create_live_draft(request))
        .await
}

#[tauri::command]
pub async fn history_complete_live_draft(
    services: State<'_, DesktopServices>,
    history_id: String,
    segments: Vec<TranscriptSegment>,
    duration: f64,
) -> Result<HistoryItemRecord, String> {
    let request = HistoryCompleteLiveDraftRequest {
        history_id,
        segments,
        duration,
    };
    validate_history_input(&request)?;
    services
        .history
        .mutation_db(move |service| service.complete_live_draft(request))
        .await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn history_save_recording(
    services: State<'_, DesktopServices>,
    segments: Vec<TranscriptSegment>,
    duration: f64,
    project_id: Option<String>,
    audio_bytes: Option<Vec<u8>>,
    native_audio_path: Option<String>,
    audio_extension: Option<String>,
) -> Result<HistoryItemRecord, String> {
    let request = HistorySaveRecordingRequest {
        segments,
        duration,
        tag_ids: Vec::new(),
        project_id,
        audio_bytes,
        native_audio_path,
        audio_extension,
    };
    validate_history_input(&request)?;
    services
        .history
        .mutation_file(move |service| service.save_recording(request))
        .await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn history_save_recording_to_project(
    services: State<'_, DesktopServices>,
    segments: Vec<TranscriptSegment>,
    duration: f64,
    project_id: Option<String>,
    audio_bytes: Option<Vec<u8>>,
    native_audio_path: Option<String>,
    audio_extension: Option<String>,
) -> Result<HistoryItemRecord, String> {
    let request = HistorySaveRecordingRequest {
        segments,
        duration,
        tag_ids: Vec::new(),
        project_id,
        audio_bytes,
        native_audio_path,
        audio_extension,
    };
    validate_history_input(&request)?;
    services
        .history
        .mutation_file(move |service| service.save_recording(request))
        .await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn history_save_imported_file(
    services: State<'_, DesktopServices>,
    id: Option<String>,
    source_path: String,
    segments: Vec<TranscriptSegment>,
    duration: f64,
    project_id: Option<String>,
    converted_source_path: Option<String>,
) -> Result<HistoryItemRecord, String> {
    let request = HistorySaveImportedFileRequest {
        id,
        source_path,
        segments,
        duration,
        tag_ids: Vec::new(),
        project_id,
        converted_source_path,
    };
    validate_history_input(&request)?;
    services
        .history
        .mutation_file(move |service| service.save_imported_file(request))
        .await
}

#[tauri::command]
pub async fn history_delete_items(
    services: State<'_, DesktopServices>,
    ids: Vec<String>,
) -> Result<(), String> {
    let deleted_at = u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_millis(),
    )
    .map_err(|error| error.to_string())?;
    let request = HistoryTrashItemsRequest { ids, deleted_at };
    services
        .history
        .mutation_file(move |service| service.trash_items(request))
        .await
}

#[tauri::command]
pub async fn history_trash_items(
    services: State<'_, DesktopServices>,
    ids: Vec<String>,
    deleted_at: u64,
) -> Result<(), String> {
    let request = HistoryTrashItemsRequest { ids, deleted_at };
    validate_history_input(&request)?;
    services
        .history
        .mutation_file(move |service| service.trash_items(request))
        .await
}

#[tauri::command]
pub async fn history_restore_items(
    services: State<'_, DesktopServices>,
    ids: Vec<String>,
) -> Result<(), String> {
    validate_history_input(&ids)?;
    let request = HistoryDeleteItemsRequest { ids };
    services
        .history
        .mutation_file(move |service| service.restore_items(request))
        .await
}

#[tauri::command]
pub async fn history_purge_items(
    services: State<'_, DesktopServices>,
    ids: Vec<String>,
) -> Result<(), String> {
    validate_history_input(&ids)?;
    let request = HistoryDeleteItemsRequest { ids };
    services
        .history
        .mutation_file(move |service| service.purge_items(request))
        .await
}

#[tauri::command]
pub async fn history_load_transcript(
    services: State<'_, DesktopServices>,
    history_id: String,
) -> Result<Option<Vec<TranscriptSegment>>, String> {
    services
        .history
        .query_db(move |service| service.load_transcript(&history_id))
        .await
}

#[tauri::command]
pub async fn history_update_transcript(
    services: State<'_, DesktopServices>,
    history_id: String,
    segments: Vec<TranscriptSegment>,
) -> Result<HistoryItemRecord, String> {
    let request = HistoryUpdateTranscriptRequest {
        history_id,
        segments,
    };
    validate_history_input(&request)?;
    services
        .history
        .mutation_db(move |service| service.update_transcript(request))
        .await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn history_save_imported_file_to_project(
    services: State<'_, DesktopServices>,
    id: Option<String>,
    source_path: String,
    segments: Vec<TranscriptSegment>,
    duration: f64,
    project_id: Option<String>,
    converted_source_path: Option<String>,
) -> Result<HistoryItemRecord, String> {
    let request = HistorySaveImportedFileRequest {
        id,
        source_path,
        segments,
        duration,
        tag_ids: Vec::new(),
        project_id,
        converted_source_path,
    };
    validate_history_input(&request)?;
    services
        .history
        .mutation_file(move |service| service.save_imported_file(request))
        .await
}

#[tauri::command]
pub async fn history_commit_transcript_edit(
    services: State<'_, DesktopServices>,
    history_id: String,
    edit_session_id: String,
    base_segments: Vec<TranscriptSegment>,
    edited_segments: Vec<TranscriptSegment>,
) -> Result<HistoryCommitTranscriptEditResult, String> {
    let request = HistoryCommitTranscriptEditRequest {
        history_id,
        edit_session_id,
        base_segments,
        edited_segments,
    };
    validate_history_input(&request)?;
    services
        .history
        .mutation_db(move |service| service.commit_transcript_edit(request))
        .await
}

#[tauri::command]
pub async fn history_create_transcript_snapshot(
    services: State<'_, DesktopServices>,
    history_id: String,
    reason: TranscriptSnapshotReason,
    segments: Vec<TranscriptSegment>,
) -> Result<TranscriptSnapshotMetadata, String> {
    let request = HistoryCreateTranscriptSnapshotRequest {
        history_id,
        reason,
        segments,
    };
    validate_history_input(&request)?;
    services
        .history
        .mutation_db(move |service| service.create_transcript_snapshot(request))
        .await
}

#[tauri::command]
pub async fn history_list_transcript_snapshots(
    services: State<'_, DesktopServices>,
    history_id: String,
) -> Result<Vec<TranscriptSnapshotMetadata>, String> {
    services
        .history
        .query_db(move |service| service.list_transcript_snapshots(&history_id))
        .await
}

#[tauri::command]
pub async fn history_load_transcript_snapshot(
    services: State<'_, DesktopServices>,
    history_id: String,
    snapshot_id: String,
) -> Result<Option<TranscriptSnapshotRecord>, String> {
    services
        .history
        .query_db(move |service| service.load_transcript_snapshot(&history_id, &snapshot_id))
        .await
}

#[tauri::command]
pub fn history_build_transcript_diff(
    snapshot_segments: Vec<TranscriptSegment>,
    current_segments: Vec<TranscriptSegment>,
) -> Result<TranscriptDiffResult, String> {
    validate_history_input(&snapshot_segments)?;
    validate_history_input(&current_segments)?;
    let result = crate::platform::history_repository::build_transcript_diff(
        snapshot_segments,
        current_segments,
    );
    validate_history_input(&result)?;
    Ok(result)
}

#[tauri::command]
pub fn history_restore_transcript_diff_rows(
    rows: Vec<TranscriptDiffRow>,
    selected_row_ids: Vec<String>,
) -> Result<Vec<TranscriptSegment>, String> {
    validate_history_input(&rows)?;
    let result =
        crate::platform::history_repository::restore_transcript_diff_rows(rows, selected_row_ids);
    validate_history_input(&result)?;
    Ok(result)
}

#[tauri::command]
pub async fn history_update_item_meta(
    services: State<'_, DesktopServices>,
    history_id: String,
    updates: HistoryItemMetaPatch,
) -> Result<(), String> {
    let request = HistoryUpdateItemMetaRequest {
        history_id,
        updates,
    };
    validate_history_input(&request)?;
    services
        .history
        .mutation_db(move |service| service.update_item_meta(request))
        .await
}

#[tauri::command]
pub async fn history_update_tag_assignments(
    services: State<'_, DesktopServices>,
    ids: Vec<String>,
    add_tag_ids: Vec<String>,
    remove_tag_ids: Vec<String>,
) -> Result<(), String> {
    let request = HistoryUpdateTagAssignmentsRequest {
        ids,
        add_tag_ids,
        remove_tag_ids,
    };
    services
        .history
        .mutation_db(move |service| service.update_tag_assignments(request))
        .await
}

#[tauri::command]
pub async fn history_replace_tag_assignments(
    services: State<'_, DesktopServices>,
    ids: Vec<String>,
    tag_ids: Vec<String>,
) -> Result<(), String> {
    let request = HistoryReplaceTagAssignmentsRequest { ids, tag_ids };
    services
        .history
        .mutation_db(move |service| service.replace_tag_assignments(request))
        .await
}

#[tauri::command]
pub async fn history_update_project_assignments(
    services: State<'_, DesktopServices>,
    ids: Vec<String>,
    project_id: Option<String>,
) -> Result<(), String> {
    history_replace_tag_assignments(services, ids, project_id.into_iter().collect()).await
}

#[tauri::command]
pub async fn history_reassign_project(
    services: State<'_, DesktopServices>,
    current_project_id: String,
    next_project_id: Option<String>,
) -> Result<(), String> {
    let items = services
        .history
        .query_db(move |service| {
            service.list_items(HistoryListOptions {
                limit: None,
                offset: None,
            })
        })
        .await?;
    let ids = items
        .into_iter()
        .filter(|item| item.tag_ids.contains(&current_project_id))
        .map(|item| item.id)
        .collect();
    let request = HistoryUpdateTagAssignmentsRequest {
        ids,
        add_tag_ids: next_project_id.into_iter().collect(),
        remove_tag_ids: vec![current_project_id],
    };
    services
        .history
        .mutation_db(move |service| service.update_tag_assignments(request))
        .await
}

#[tauri::command]
pub async fn history_load_summary(
    services: State<'_, DesktopServices>,
    history_id: String,
) -> Result<Option<HistorySummaryPayload>, String> {
    services
        .history
        .db_task(move |repository| repository.load_summary(&history_id))
        .await
}

#[tauri::command]
pub async fn history_save_summary(
    services: State<'_, DesktopServices>,
    history_id: String,
    summary_payload: HistorySummaryPayload,
) -> Result<(), String> {
    services
        .history
        .db_task(move |repository| repository.save_summary(&history_id, summary_payload))
        .await
}

#[tauri::command]
pub async fn history_delete_summary(
    services: State<'_, DesktopServices>,
    history_id: String,
) -> Result<(), String> {
    services
        .history
        .db_task(move |repository| repository.delete_summary(&history_id))
        .await
}

#[tauri::command]
pub async fn history_resolve_audio_path(
    services: State<'_, DesktopServices>,
    history_id: String,
) -> Result<Option<String>, String> {
    services
        .history
        .file_task(move |repository| repository.resolve_audio_path(&history_id))
        .await
}

#[tauri::command]
pub async fn history_preview_audio_cleanup(
    services: State<'_, DesktopServices>,
    retention_days: Option<u64>,
    exclude_history_id: Option<String>,
) -> Result<HistoryAudioCleanupReport, String> {
    let request = HistoryAudioCleanupRequest {
        retention_days,
        exclude_history_id,
    };
    validate_history_input(&request)?;
    services
        .history
        .file_task(move |repository| repository.preview_audio_cleanup(request))
        .await
}

#[tauri::command]
pub async fn history_cleanup_audio(
    services: State<'_, DesktopServices>,
    retention_days: Option<u64>,
    exclude_history_id: Option<String>,
) -> Result<HistoryAudioCleanupReport, String> {
    let request = HistoryAudioCleanupRequest {
        retention_days,
        exclude_history_id,
    };
    validate_history_input(&request)?;
    services
        .history
        .file_task(move |repository| repository.cleanup_audio(request))
        .await
}

#[tauri::command]
pub async fn history_open_folder<R: Runtime>(
    app: AppHandle<R>,
    services: State<'_, DesktopServices>,
) -> Result<(), String> {
    let folder_path = services.history.ensure_history_folder()?;
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_path(folder_path.to_string_lossy(), None::<&str>)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn export_backup_archive(
    services: State<'_, DesktopServices>,
    request: ExportBackupArchiveRequest,
) -> Result<BackupManifest, String> {
    services.history.export_backup_archive(request).await
}

#[tauri::command]
pub async fn prepare_backup_import(
    services: State<'_, DesktopServices>,
    archive_path: String,
) -> Result<PreparedBackupImport, String> {
    services.history.prepare_backup_import(archive_path).await
}

#[tauri::command]
pub async fn apply_prepared_history_import(
    services: State<'_, DesktopServices>,
    import_id: String,
) -> Result<(), String> {
    services
        .history
        .apply_prepared_history_import(import_id)
        .await
}

#[tauri::command]
pub async fn dispose_prepared_backup_import(
    services: State<'_, DesktopServices>,
    import_id: String,
) -> Result<(), String> {
    services
        .history
        .dispose_prepared_backup_import(import_id)
        .await
}
