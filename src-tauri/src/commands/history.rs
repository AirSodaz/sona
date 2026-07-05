use serde_json::Value;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, Runtime, State};

use crate::core::database::Database;
use crate::core::history_store::{HistoryStore, HistoryStoreError};
use crate::core::paths::{PathKind, PathProvider};
use crate::integrations::asr::TranscriptSegment;
use crate::repositories::history::SqliteHistoryStore;
use crate::repositories::history::backup::{
    apply_prepared_history_import_inner, export_backup_archive_inner, prepare_backup_import_inner,
};
use crate::repositories::history::fs_utils::remove_path_if_exists;
use crate::repositories::history::{
    BackupManifest, ExportBackupArchiveRequest, HISTORY_DIR_NAME, HistoryAudioCleanupReport,
    HistoryAudioCleanupRequest, HistoryCreateLiveDraftRequest, HistoryItemRecord,
    HistoryListOptions, HistoryRepositoryState, HistorySaveImportedFileRequest,
    HistorySaveRecordingRequest, HistoryWorkspaceDateFilter, HistoryWorkspaceFilterType,
    HistoryWorkspaceQueryRequest, HistoryWorkspaceQueryResult, HistoryWorkspaceScope,
    HistoryWorkspaceSortOrder, LiveRecordingDraftResult, PreparedBackupImport,
    PreparedBackupImportState, TranscriptDiffResult, TranscriptDiffRow, TranscriptSnapshotMetadata,
    TranscriptSnapshotReason, TranscriptSnapshotRecord,
};

async fn run_history_file_task_inner<T, F>(
    app_local_data_dir: PathBuf,
    db: Arc<Database>,
    lock: Arc<Mutex<()>>,
    task: F,
) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(SqliteHistoryStore) -> Result<T, HistoryStoreError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = lock.lock().map_err(|error| error.to_string())?;
        task(SqliteHistoryStore::new(app_local_data_dir, db)).map_err(|e| e.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

async fn run_history_db_task<R, T, F>(app: &AppHandle<R>, task: F) -> Result<T, String>
where
    R: Runtime,
    T: Send + 'static,
    F: FnOnce(SqliteHistoryStore) -> Result<T, HistoryStoreError> + Send + 'static,
{
    let app_local_data_dir = (app as &dyn PathProvider).resolve_path(PathKind::AppLocalData)?;
    let db = Arc::clone(app.state::<Arc<Database>>().inner());
    crate::repositories::history::llm_helpers::run_llm_db_task(app_local_data_dir, db, task).await
}

async fn run_history_file_task<R, T, F>(
    app: &AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    task: F,
) -> Result<T, String>
where
    R: Runtime,
    T: Send + 'static,
    F: FnOnce(SqliteHistoryStore) -> Result<T, HistoryStoreError> + Send + 'static,
{
    let app_local_data_dir = (app as &dyn PathProvider).resolve_path(PathKind::AppLocalData)?;
    let db = Arc::clone(app.state::<Arc<Database>>().inner());
    run_history_file_task_inner(app_local_data_dir, db, state.file_lock.clone(), task).await
}

#[tauri::command]
pub async fn history_list_items<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<HistoryItemRecord>, String> {
    let opts = HistoryListOptions { limit, offset };
    run_history_file_task(&app, state, move |repository| {
        repository.list_items_with_reconciled_live_drafts_paginated(opts)
    })
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
    run_history_file_task(&app, state, move |repository| {
        repository.query_workspace(request)
    })
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
    run_history_file_task(&app, state, move |repository| {
        repository.create_live_draft(request)
    })
    .await
}

#[tauri::command]
pub async fn history_complete_live_draft<R: Runtime>(
    app: AppHandle<R>,
    history_id: String,
    segments: Value,
    duration: f64,
) -> Result<HistoryItemRecord, String> {
    run_history_db_task(&app, move |repository| {
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
    run_history_file_task(&app, state, move |repository| {
        repository.save_recording(request)
    })
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
    run_history_file_task(&app, state, move |repository| {
        repository.save_imported_file(request)
    })
    .await
}

#[tauri::command]
pub async fn history_delete_items<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
    ids: Vec<String>,
) -> Result<(), String> {
    run_history_file_task(&app, state, move |repository| repository.delete_items(&ids)).await
}

#[tauri::command]
pub async fn history_load_transcript<R: Runtime>(
    app: AppHandle<R>,
    history_id: String,
) -> Result<Option<Vec<TranscriptSegment>>, String> {
    run_history_db_task(&app, move |repository| {
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
    run_history_db_task(&app, move |repository| {
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
    run_history_db_task(&app, move |repository| {
        repository.create_transcript_snapshot(&history_id, reason, segments)
    })
    .await
}

#[tauri::command]
pub async fn history_list_transcript_snapshots<R: Runtime>(
    app: AppHandle<R>,
    history_id: String,
) -> Result<Vec<TranscriptSnapshotMetadata>, String> {
    run_history_db_task(&app, move |repository| {
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
    run_history_db_task(&app, move |repository| {
        repository.load_transcript_snapshot(&history_id, &snapshot_id)
    })
    .await
}

#[tauri::command]
pub fn history_build_transcript_diff(
    snapshot_segments: Vec<TranscriptSegment>,
    current_segments: Vec<TranscriptSegment>,
) -> Result<TranscriptDiffResult, String> {
    Ok(
        crate::repositories::history::transcript_diff::build_transcript_diff(
            snapshot_segments,
            current_segments,
        ),
    )
}

#[tauri::command]
pub fn history_restore_transcript_diff_rows(
    rows: Vec<TranscriptDiffRow>,
    selected_row_ids: Vec<String>,
) -> Result<Vec<TranscriptSegment>, String> {
    Ok(
        crate::repositories::history::transcript_diff::restore_transcript_diff_rows(
            rows,
            selected_row_ids,
        ),
    )
}

#[tauri::command]
pub async fn history_update_item_meta<R: Runtime>(
    app: AppHandle<R>,
    history_id: String,
    updates: Value,
) -> Result<(), String> {
    run_history_db_task(&app, move |repository| {
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
    run_history_db_task(&app, move |repository| {
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
    run_history_db_task(&app, move |repository| {
        repository.reassign_project(current_project_id, next_project_id)
    })
    .await
}

#[tauri::command]
pub async fn history_load_summary<R: Runtime>(
    app: AppHandle<R>,
    history_id: String,
) -> Result<Option<Value>, String> {
    run_history_db_task(&app, move |repository| repository.load_summary(&history_id)).await
}

#[tauri::command]
pub async fn history_save_summary<R: Runtime>(
    app: AppHandle<R>,
    history_id: String,
    summary_payload: Value,
) -> Result<(), String> {
    run_history_db_task(&app, move |repository| {
        repository.save_summary(&history_id, summary_payload)
    })
    .await
}

#[tauri::command]
pub async fn history_delete_summary<R: Runtime>(
    app: AppHandle<R>,
    history_id: String,
) -> Result<(), String> {
    run_history_db_task(&app, move |repository| {
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
    run_history_file_task(&app, state, move |repository| {
        repository.resolve_audio_path(&history_id)
    })
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
    run_history_file_task(&app, state, move |repository| {
        repository.preview_audio_cleanup(request)
    })
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
    run_history_file_task(&app, state, move |repository| {
        repository.cleanup_audio(request)
    })
    .await
}

#[tauri::command]
pub async fn history_open_folder<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, HistoryRepositoryState>,
) -> Result<(), String> {
    let app_local_data_dir = (&app as &dyn PathProvider).resolve_path(PathKind::AppLocalData)?;
    let db = Arc::clone(app.state::<Arc<Database>>().inner());
    {
        let _guard = state.file_lock.lock().map_err(|error| error.to_string())?;
        SqliteHistoryStore::new(app_local_data_dir.clone(), db)
            .ensure_ready()
            .map_err(|e| e.to_string())?;
    }

    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_path(
            app_local_data_dir.join(HISTORY_DIR_NAME).to_string_lossy(),
            None::<&str>,
        )
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn export_backup_archive<R: Runtime>(
    app: AppHandle<R>,
    history_state: State<'_, HistoryRepositoryState>,
    request: ExportBackupArchiveRequest,
) -> Result<BackupManifest, String> {
    let app_local_data_dir = (&app as &dyn PathProvider).resolve_path(PathKind::AppLocalData)?;
    let db = Arc::clone(app.state::<Arc<Database>>().inner());
    let lock = history_state.file_lock.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = lock.lock().map_err(|error| error.to_string())?;
        export_backup_archive_inner(&app_local_data_dir, db, request)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn prepare_backup_import(
    state: State<'_, PreparedBackupImportState>,
    archive_path: String,
) -> Result<PreparedBackupImport, String> {
    let archive_path_buf = PathBuf::from(&archive_path);
    let (prepared, snapshot) = tauri::async_runtime::spawn_blocking(move || {
        prepare_backup_import_inner(&archive_path_buf)
    })
    .await
    .map_err(|error| error.to_string())??;

    state.insert(prepared.import_id.clone(), snapshot)?;
    Ok(prepared)
}

#[tauri::command]
pub async fn apply_prepared_history_import<R: Runtime>(
    app: AppHandle<R>,
    history_state: State<'_, HistoryRepositoryState>,
    prepared_state: State<'_, PreparedBackupImportState>,
    import_id: String,
) -> Result<(), String> {
    let Some(snapshot) = prepared_state.get(&import_id)? else {
        return Err(format!("Prepared backup import not found: {import_id}"));
    };

    let app_local_data_dir = (&app as &dyn PathProvider).resolve_path(PathKind::AppLocalData)?;
    let db = Arc::clone(app.state::<Arc<Database>>().inner());
    let lock = history_state.file_lock.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = lock.lock().map_err(|error| error.to_string())?;
        apply_prepared_history_import_inner(
            &app_local_data_dir,
            db,
            &import_id,
            &snapshot.extraction_dir,
        )
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn dispose_prepared_backup_import(
    state: State<'_, PreparedBackupImportState>,
    import_id: String,
) -> Result<(), String> {
    let Some(snapshot) = state.remove(&import_id)? else {
        return Ok(());
    };

    tauri::async_runtime::spawn_blocking(move || {
        let _archive_path = snapshot.archive_path;
        remove_path_if_exists(&snapshot.extraction_dir)
    })
    .await
    .map_err(|error| error.to_string())?
}
