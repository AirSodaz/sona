pub(crate) mod llm_helpers;
mod state;
use crate::platform::paths::{PathKind, PathProvider, TauriPathProvider};
use sona_core::history_store::{HistoryStore, HistoryStoreError};
use sona_sqlite::Database;
pub use sona_sqlite::history_backup as backup;
pub use sona_sqlite::history_store as sqlite_store;
pub use sqlite_store::SqliteHistoryStore;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Runtime};
#[cfg(test)]
pub(crate) mod test_support;

// Re-exports from history platform adapter modules
pub use sona_core::history::{item_factory, transcript_diff, transcript_payload, workspace_query};
pub(crate) use sona_sqlite::history_fs_utils as fs_utils;

pub use sona_core::history::{
    BackupManifest, BackupManifestCounts, BackupManifestScopes, ExportBackupArchiveRequest,
    HistoryAudioCleanupReport, HistoryAudioCleanupRequest, HistoryAudioStatus,
    HistoryBackupSnapshot, HistoryCreateLiveDraftRequest, HistoryDraftSource, HistoryItemKind,
    HistoryItemRecord, HistoryItemStatus, HistoryListOptions, HistorySaveImportedFileRequest,
    HistorySaveRecordingRequest, HistoryWorkspaceDateFilter, HistoryWorkspaceFilterType,
    HistoryWorkspaceItemCounts, HistoryWorkspaceItemSearchMatch, HistoryWorkspaceQueryRequest,
    HistoryWorkspaceQueryResult, HistoryWorkspaceScope, HistoryWorkspaceSearchRange,
    HistoryWorkspaceSearchSnippet, HistoryWorkspaceSortOrder, HistoryWorkspaceSummary,
    LiveRecordingDraftResult, PreparedBackupImport, PreparedBackupImportSnapshot,
    TranscriptDiffResult, TranscriptDiffRow, TranscriptDiffStatus, TranscriptSnapshotMetadata,
    TranscriptSnapshotReason, TranscriptSnapshotRecord,
};
pub use state::{HistoryRepositoryState, PreparedBackupImportState};

pub(crate) const HISTORY_DIR_NAME: &str = "history";

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

pub async fn run_history_db_task<R, T, F>(app: &AppHandle<R>, task: F) -> Result<T, String>
where
    R: Runtime,
    T: Send + 'static,
    F: FnOnce(SqliteHistoryStore) -> Result<T, HistoryStoreError> + Send + 'static,
{
    let app_local_data_dir =
        TauriPathProvider::from_app(app).resolve_path(PathKind::AppLocalData)?;
    let db = crate::platform::database::sqlite_database(app);
    llm_helpers::run_llm_db_task(app_local_data_dir, db, task).await
}

pub async fn run_history_file_task<R, T, F>(
    app: &AppHandle<R>,
    state: &HistoryRepositoryState,
    task: F,
) -> Result<T, String>
where
    R: Runtime,
    T: Send + 'static,
    F: FnOnce(SqliteHistoryStore) -> Result<T, HistoryStoreError> + Send + 'static,
{
    let app_local_data_dir =
        TauriPathProvider::from_app(app).resolve_path(PathKind::AppLocalData)?;
    let db = crate::platform::database::sqlite_database(app);
    run_history_file_task_inner(app_local_data_dir, db, state.file_lock.clone(), task).await
}

pub async fn export_backup_archive<R: Runtime>(
    app: &AppHandle<R>,
    state: &HistoryRepositoryState,
    request: ExportBackupArchiveRequest,
) -> Result<BackupManifest, String> {
    let app_local_data_dir =
        TauriPathProvider::from_app(app).resolve_path(PathKind::AppLocalData)?;
    let db = crate::platform::database::sqlite_database(app);
    let lock = state.file_lock.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = lock.lock().map_err(|error| error.to_string())?;
        backup::export_backup_archive_inner(&app_local_data_dir, db, request)
    })
    .await
    .map_err(|error| error.to_string())?
}

pub async fn prepare_backup_import(
    state: &PreparedBackupImportState,
    archive_path: String,
) -> Result<PreparedBackupImport, String> {
    let archive_path_buf = PathBuf::from(&archive_path);
    let (prepared, snapshot) = tauri::async_runtime::spawn_blocking(move || {
        backup::prepare_backup_import_inner(&archive_path_buf)
    })
    .await
    .map_err(|error| error.to_string())??;

    state.insert(prepared.import_id.clone(), snapshot)?;
    Ok(prepared)
}

pub async fn apply_prepared_history_import<R: Runtime>(
    app: &AppHandle<R>,
    history_state: &HistoryRepositoryState,
    prepared_state: &PreparedBackupImportState,
    import_id: String,
) -> Result<(), String> {
    let Some(snapshot) = prepared_state.get(&import_id)? else {
        return Err(format!("Prepared backup import not found: {import_id}"));
    };

    let app_local_data_dir =
        TauriPathProvider::from_app(app).resolve_path(PathKind::AppLocalData)?;
    let db = crate::platform::database::sqlite_database(app);
    let lock = history_state.file_lock.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = lock.lock().map_err(|error| error.to_string())?;
        backup::apply_prepared_history_import_inner(
            &app_local_data_dir,
            db,
            &import_id,
            &snapshot.extraction_dir,
        )
    })
    .await
    .map_err(|error| error.to_string())?
}

pub async fn dispose_prepared_backup_import(
    state: &PreparedBackupImportState,
    import_id: String,
) -> Result<(), String> {
    let Some(snapshot) = state.remove(&import_id)? else {
        return Ok(());
    };

    tauri::async_runtime::spawn_blocking(move || {
        fs_utils::remove_path_if_exists(&snapshot.extraction_dir)
    })
    .await
    .map_err(|error| error.to_string())?
}

pub async fn open_history_folder<R: Runtime>(
    app: &AppHandle<R>,
    state: &HistoryRepositoryState,
) -> Result<(), String> {
    let app_local_data_dir =
        TauriPathProvider::from_app(app).resolve_path(PathKind::AppLocalData)?;
    let db = crate::platform::database::sqlite_database(app);
    {
        let _guard = state.file_lock.lock().map_err(|error| error.to_string())?;
        SqliteHistoryStore::new(app_local_data_dir.clone(), db)
            .ensure_ready()
            .map_err(|error| error.to_string())?;
    }

    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_path(
            app_local_data_dir.join(HISTORY_DIR_NAME).to_string_lossy(),
            None::<&str>,
        )
        .map_err(|error| error.to_string())
}
