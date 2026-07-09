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
