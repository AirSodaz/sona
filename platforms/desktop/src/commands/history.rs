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
use sona_core::history::HistorySummaryPayload;
use sona_core::history::mutation_repository::{
    HistoryCompleteLiveDraftRequest, HistoryCreateTranscriptSnapshotRequest,
    HistoryDeleteItemsRequest, HistoryItemMetaPatch, HistoryReassignProjectRequest,
    HistoryUpdateItemMetaRequest, HistoryUpdateProjectAssignmentsRequest,
    HistoryUpdateTranscriptRequest,
};
use sona_core::history_store::HistoryStore;

fn validate_history_input<T: serde::Serialize + ?Sized>(value: &T) -> Result<(), String> {
    sona_ts_bind::validate_typescript_safe_integers(value)
}

#[tauri::command]
pub async fn history_list_items<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<HistoryItemRecord>, String> {
    validate_history_input(&(limit, offset))?;
    let opts = HistoryListOptions { limit, offset };
    crate::platform::history_repository::run_history_query_file_task(
        &app,
        state.inner(),
        move |service| service.list_items(opts),
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
    crate::platform::history_repository::run_history_query_file_task(
        &app,
        state.inner(),
        move |service| service.query_workspace(request),
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
    validate_history_input(&request)?;
    crate::platform::history_repository::run_history_mutation_file_task(
        &app,
        state.inner(),
        move |service| service.create_live_draft(request),
    )
    .await
}

#[tauri::command]
pub async fn history_complete_live_draft<R: Runtime>(
    app: AppHandle<R>,
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
    crate::platform::history_repository::run_history_mutation_db_task(&app, move |service| {
        service.complete_live_draft(request)
    })
    .await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn history_save_recording<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
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
        project_id,
        audio_bytes,
        native_audio_path,
        audio_extension,
    };
    validate_history_input(&request)?;
    crate::platform::history_repository::run_history_mutation_file_task(
        &app,
        state.inner(),
        move |service| service.save_recording(request),
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
        project_id,
        converted_source_path,
    };
    validate_history_input(&request)?;
    crate::platform::history_repository::run_history_mutation_file_task(
        &app,
        state.inner(),
        move |service| service.save_imported_file(request),
    )
    .await
}

#[tauri::command]
pub async fn history_delete_items<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    ids: Vec<String>,
) -> Result<(), String> {
    let request = HistoryDeleteItemsRequest { ids };
    crate::platform::history_repository::run_history_mutation_file_task(
        &app,
        state.inner(),
        move |service| service.delete_items(request),
    )
    .await
}

#[tauri::command]
pub async fn history_load_transcript<R: Runtime>(
    app: AppHandle<R>,
    history_id: String,
) -> Result<Option<Vec<TranscriptSegment>>, String> {
    crate::platform::history_repository::run_history_query_db_task(&app, move |service| {
        service.load_transcript(&history_id)
    })
    .await
}

#[tauri::command]
pub async fn history_update_transcript<R: Runtime>(
    app: AppHandle<R>,
    history_id: String,
    segments: Vec<TranscriptSegment>,
) -> Result<HistoryItemRecord, String> {
    let request = HistoryUpdateTranscriptRequest {
        history_id,
        segments,
    };
    validate_history_input(&request)?;
    crate::platform::history_repository::run_history_mutation_db_task(&app, move |service| {
        service.update_transcript(request)
    })
    .await
}

#[tauri::command]
pub async fn history_create_transcript_snapshot<R: Runtime>(
    app: AppHandle<R>,
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
    crate::platform::history_repository::run_history_mutation_db_task(&app, move |service| {
        service.create_transcript_snapshot(request)
    })
    .await
}

#[tauri::command]
pub async fn history_list_transcript_snapshots<R: Runtime>(
    app: AppHandle<R>,
    history_id: String,
) -> Result<Vec<TranscriptSnapshotMetadata>, String> {
    crate::platform::history_repository::run_history_query_db_task(&app, move |service| {
        service.list_transcript_snapshots(&history_id)
    })
    .await
}

#[tauri::command]
pub async fn history_load_transcript_snapshot<R: Runtime>(
    app: AppHandle<R>,
    history_id: String,
    snapshot_id: String,
) -> Result<Option<TranscriptSnapshotRecord>, String> {
    crate::platform::history_repository::run_history_query_db_task(&app, move |service| {
        service.load_transcript_snapshot(&history_id, &snapshot_id)
    })
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
pub async fn history_update_item_meta<R: Runtime>(
    app: AppHandle<R>,
    history_id: String,
    updates: HistoryItemMetaPatch,
) -> Result<(), String> {
    let request = HistoryUpdateItemMetaRequest {
        history_id,
        updates,
    };
    validate_history_input(&request)?;
    crate::platform::history_repository::run_history_mutation_db_task(&app, move |service| {
        service.update_item_meta(request)
    })
    .await
}

#[tauri::command]
pub async fn history_update_project_assignments<R: Runtime>(
    app: AppHandle<R>,
    ids: Vec<String>,
    project_id: Option<String>,
) -> Result<(), String> {
    let request = HistoryUpdateProjectAssignmentsRequest { ids, project_id };
    crate::platform::history_repository::run_history_mutation_db_task(&app, move |service| {
        service.update_project_assignments(request)
    })
    .await
}

#[tauri::command]
pub async fn history_reassign_project<R: Runtime>(
    app: AppHandle<R>,
    current_project_id: String,
    next_project_id: Option<String>,
) -> Result<(), String> {
    let request = HistoryReassignProjectRequest {
        current_project_id,
        next_project_id,
    };
    crate::platform::history_repository::run_history_mutation_db_task(&app, move |service| {
        service.reassign_project(request)
    })
    .await
}

#[tauri::command]
pub async fn history_load_summary<R: Runtime>(
    app: AppHandle<R>,
    history_id: String,
) -> Result<Option<HistorySummaryPayload>, String> {
    crate::platform::history_repository::run_history_db_task(&app, move |repository| {
        repository.load_summary(&history_id)
    })
    .await
}

#[tauri::command]
pub async fn history_save_summary<R: Runtime>(
    app: AppHandle<R>,
    history_id: String,
    summary_payload: HistorySummaryPayload,
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
    validate_history_input(&request)?;
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
    validate_history_input(&request)?;
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
    state: State<'_, PreparedBackupImportState>,
    request: ExportBackupArchiveRequest,
) -> Result<BackupManifest, String> {
    crate::platform::history_repository::export_backup_archive(&app, state.inner(), request).await
}

#[tauri::command]
pub async fn prepare_backup_import<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, PreparedBackupImportState>,
    archive_path: String,
) -> Result<PreparedBackupImport, String> {
    crate::platform::history_repository::prepare_backup_import(&app, state.inner(), archive_path)
        .await
}

#[tauri::command]
pub async fn apply_prepared_history_import<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, PreparedBackupImportState>,
    import_id: String,
) -> Result<(), String> {
    crate::platform::history_repository::apply_prepared_history_import(
        &app,
        state.inner(),
        import_id,
    )
    .await
}

#[tauri::command]
pub async fn dispose_prepared_backup_import<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, PreparedBackupImportState>,
    import_id: String,
) -> Result<(), String> {
    crate::platform::history_repository::dispose_prepared_backup_import(
        &app,
        state.inner(),
        import_id,
    )
    .await
}
