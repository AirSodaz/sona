use serde_json::Value;
use tauri::{AppHandle, Runtime, State};

use crate::integrations::asr::TranscriptSegment;
use crate::platform::history_repository::{
    BackupManifest, ExportBackupArchiveRequest, HistoryAudioCleanupReport,
    HistoryAudioCleanupRequest, HistoryCreateLiveDraftRequest, HistoryItemRecord,
    HistoryListOptions, HistoryRepositoryState, HistorySaveImportedFileRequest,
    HistorySaveRecordingRequest, HistoryWorkspaceDateFilter, HistoryWorkspaceFilterType,
    HistoryWorkspaceQueryRequest, HistoryWorkspaceQueryResult, HistoryWorkspaceScope,
    HistoryWorkspaceSortOrder, LiveRecordingDraftResult, PreparedBackupImport,
    PreparedBackupImportState, TranscriptDiffResult, TranscriptDiffRow, TranscriptSnapshotMetadata,
    TranscriptSnapshotReason, TranscriptSnapshotRecord,
};
use sona_core::history_store::HistoryStore;

#[tauri::command]
pub async fn history_list_items<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<HistoryItemRecord>, String> {
    let opts = HistoryListOptions { limit, offset };
    crate::platform::history_repository::run_history_file_task(
        &app,
        state.inner(),
        move |repository| repository.list_items_with_reconciled_live_drafts_paginated(opts),
    )
    .await
}

#[tauri::command]
pub async fn history_query_workspace<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    scope: HistoryWorkspaceScope,
    query: String,
    filter_type: HistoryWorkspaceFilterType,
    date_filter: HistoryWorkspaceDateFilter,
    sort_order: HistoryWorkspaceSortOrder,
) -> Result<HistoryWorkspaceQueryResult, String> {
    let request = HistoryWorkspaceQueryRequest {
        scope,
        query,
        filter_type,
        date_filter,
        sort_order,
    };
    crate::platform::history_repository::run_history_file_task(
        &app,
        state.inner(),
        move |repository| repository.query_workspace(request),
    )
    .await
}

#[tauri::command]
pub async fn history_create_live_draft<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    id: Option<String>,
    audio_extension: String,
    project_id: Option<String>,
    icon: Option<String>,
) -> Result<LiveRecordingDraftResult, String> {
    let request = HistoryCreateLiveDraftRequest {
        id,
        audio_extension,
        project_id,
        icon,
    };
    crate::platform::history_repository::run_history_file_task(
        &app,
        state.inner(),
        move |repository| repository.create_live_draft(request),
    )
    .await
}

#[tauri::command]
pub async fn history_complete_live_draft<R: Runtime>(
    app: AppHandle<R>,
    history_id: String,
    segments: Value,
    duration: f64,
) -> Result<HistoryItemRecord, String> {
    crate::platform::history_repository::run_history_db_task(&app, move |repository| {
        repository.complete_live_draft(&history_id, segments, duration)
    })
    .await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn history_save_recording<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    segments: Value,
    duration: f64,
    project_id: Option<String>,
    audio_bytes: Option<Vec<u8>>,
    native_audio_path: Option<String>,
    audio_extension: Option<String>,
) -> Result<HistoryItemRecord, String> {
    let request = HistorySaveRecordingRequest {
        segments,
        duration,
        project_id,
        audio_bytes,
        native_audio_path,
        audio_extension,
    };
    crate::platform::history_repository::run_history_file_task(
        &app,
        state.inner(),
        move |repository| repository.save_recording(request),
    )
    .await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn history_save_imported_file<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    id: Option<String>,
    source_path: String,
    segments: Value,
    duration: f64,
    project_id: Option<String>,
    converted_source_path: Option<String>,
) -> Result<HistoryItemRecord, String> {
    let request = HistorySaveImportedFileRequest {
        id,
        source_path,
        segments,
        duration,
        project_id,
        converted_source_path,
    };
    crate::platform::history_repository::run_history_file_task(
        &app,
        state.inner(),
        move |repository| repository.save_imported_file(request),
    )
    .await
}

#[tauri::command]
pub async fn history_delete_items<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    ids: Vec<String>,
) -> Result<(), String> {
    crate::platform::history_repository::run_history_file_task(
        &app,
        state.inner(),
        move |repository| repository.delete_items(&ids),
    )
    .await
}

#[tauri::command]
pub async fn history_load_transcript<R: Runtime>(
    app: AppHandle<R>,
    history_id: String,
) -> Result<Option<Vec<TranscriptSegment>>, String> {
    crate::platform::history_repository::run_history_db_task(&app, move |repository| {
        repository.load_transcript(&history_id)
    })
    .await
}

#[tauri::command]
pub async fn history_update_transcript<R: Runtime>(
    app: AppHandle<R>,
    history_id: String,
    segments: Value,
) -> Result<HistoryItemRecord, String> {
    crate::platform::history_repository::run_history_db_task(&app, move |repository| {
        repository.update_transcript(&history_id, segments)
    })
    .await
}

#[tauri::command]
pub async fn history_create_transcript_snapshot<R: Runtime>(
    app: AppHandle<R>,
    history_id: String,
    reason: TranscriptSnapshotReason,
    segments: Value,
) -> Result<TranscriptSnapshotMetadata, String> {
    crate::platform::history_repository::run_history_db_task(&app, move |repository| {
        repository.create_transcript_snapshot(&history_id, reason, segments)
    })
    .await
}

#[tauri::command]
pub async fn history_list_transcript_snapshots<R: Runtime>(
    app: AppHandle<R>,
    history_id: String,
) -> Result<Vec<TranscriptSnapshotMetadata>, String> {
    crate::platform::history_repository::run_history_db_task(&app, move |repository| {
        repository.list_transcript_snapshots(&history_id)
    })
    .await
}

#[tauri::command]
pub async fn history_load_transcript_snapshot<R: Runtime>(
    app: AppHandle<R>,
    history_id: String,
    snapshot_id: String,
) -> Result<Option<TranscriptSnapshotRecord>, String> {
    crate::platform::history_repository::run_history_db_task(&app, move |repository| {
        repository.load_transcript_snapshot(&history_id, &snapshot_id)
    })
    .await
}

#[tauri::command]
pub fn history_build_transcript_diff(
    snapshot_segments: Vec<TranscriptSegment>,
    current_segments: Vec<TranscriptSegment>,
) -> Result<TranscriptDiffResult, String> {
    Ok(crate::platform::history_repository::build_transcript_diff(
        snapshot_segments,
        current_segments,
    ))
}

#[tauri::command]
pub fn history_restore_transcript_diff_rows(
    rows: Vec<TranscriptDiffRow>,
    selected_row_ids: Vec<String>,
) -> Result<Vec<TranscriptSegment>, String> {
    Ok(crate::platform::history_repository::restore_transcript_diff_rows(rows, selected_row_ids))
}

#[tauri::command]
pub async fn history_update_item_meta<R: Runtime>(
    app: AppHandle<R>,
    history_id: String,
    updates: Value,
) -> Result<(), String> {
    crate::platform::history_repository::run_history_db_task(&app, move |repository| {
        repository.update_item_meta(&history_id, updates)
    })
    .await
}

#[tauri::command]
pub async fn history_update_project_assignments<R: Runtime>(
    app: AppHandle<R>,
    ids: Vec<String>,
    project_id: Option<String>,
) -> Result<(), String> {
    crate::platform::history_repository::run_history_db_task(&app, move |repository| {
        repository.update_project_assignments(&ids, project_id)
    })
    .await
}

#[tauri::command]
pub async fn history_reassign_project<R: Runtime>(
    app: AppHandle<R>,
    current_project_id: String,
    next_project_id: Option<String>,
) -> Result<(), String> {
    crate::platform::history_repository::run_history_db_task(&app, move |repository| {
        repository.reassign_project(current_project_id, next_project_id)
    })
    .await
}

#[tauri::command]
pub async fn history_load_summary<R: Runtime>(
    app: AppHandle<R>,
    history_id: String,
) -> Result<Option<Value>, String> {
    crate::platform::history_repository::run_history_db_task(&app, move |repository| {
        repository.load_summary(&history_id)
    })
    .await
}

#[tauri::command]
pub async fn history_save_summary<R: Runtime>(
    app: AppHandle<R>,
    history_id: String,
    summary_payload: Value,
) -> Result<(), String> {
    crate::platform::history_repository::run_history_db_task(&app, move |repository| {
        repository.save_summary(&history_id, summary_payload)
    })
    .await
}

#[tauri::command]
pub async fn history_delete_summary<R: Runtime>(
    app: AppHandle<R>,
    history_id: String,
) -> Result<(), String> {
    crate::platform::history_repository::run_history_db_task(&app, move |repository| {
        repository.delete_summary(&history_id)
    })
    .await
}

#[tauri::command]
pub async fn history_resolve_audio_path<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    history_id: String,
) -> Result<Option<String>, String> {
    crate::platform::history_repository::run_history_file_task(
        &app,
        state.inner(),
        move |repository| repository.resolve_audio_path(&history_id),
    )
    .await
}

#[tauri::command]
pub async fn history_preview_audio_cleanup<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    retention_days: Option<u64>,
    exclude_history_id: Option<String>,
) -> Result<HistoryAudioCleanupReport, String> {
    let request = HistoryAudioCleanupRequest {
        retention_days,
        exclude_history_id,
    };
    crate::platform::history_repository::run_history_file_task(
        &app,
        state.inner(),
        move |repository| repository.preview_audio_cleanup(request),
    )
    .await
}

#[tauri::command]
pub async fn history_cleanup_audio<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    retention_days: Option<u64>,
    exclude_history_id: Option<String>,
) -> Result<HistoryAudioCleanupReport, String> {
    let request = HistoryAudioCleanupRequest {
        retention_days,
        exclude_history_id,
    };
    crate::platform::history_repository::run_history_file_task(
        &app,
        state.inner(),
        move |repository| repository.cleanup_audio(request),
    )
    .await
}

#[tauri::command]
pub async fn history_open_folder<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
) -> Result<(), String> {
    crate::platform::history_repository::open_history_folder(&app, state.inner()).await
}

#[tauri::command]
pub async fn export_backup_archive<R: Runtime>(
    app: AppHandle<R>,
    history_state: State<'_, HistoryRepositoryState>,
    request: ExportBackupArchiveRequest,
) -> Result<BackupManifest, String> {
    crate::platform::history_repository::export_backup_archive(&app, history_state.inner(), request)
        .await
}

#[tauri::command]
pub async fn prepare_backup_import(
    state: State<'_, PreparedBackupImportState>,
    archive_path: String,
) -> Result<PreparedBackupImport, String> {
    crate::platform::history_repository::prepare_backup_import(state.inner(), archive_path).await
}

#[tauri::command]
pub async fn apply_prepared_history_import<R: Runtime>(
    app: AppHandle<R>,
    history_state: State<'_, HistoryRepositoryState>,
    prepared_state: State<'_, PreparedBackupImportState>,
    import_id: String,
) -> Result<(), String> {
    crate::platform::history_repository::apply_prepared_history_import(
        &app,
        history_state.inner(),
        prepared_state.inner(),
        import_id,
    )
    .await
}

#[tauri::command]
pub async fn dispose_prepared_backup_import(
    state: State<'_, PreparedBackupImportState>,
    import_id: String,
) -> Result<(), String> {
    crate::platform::history_repository::dispose_prepared_backup_import(state.inner(), import_id)
        .await
}
