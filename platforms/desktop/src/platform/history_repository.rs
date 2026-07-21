pub(crate) mod llm_helpers;
mod state;
use serde::Serialize;
use sona_archive::FsBackupAdapter;
use sona_core::backup::{
    BackupApplyPreparedImportRequest, BackupError, BackupExportRequest, BackupPrepareImportRequest,
};
use sona_core::history::mutation_repository::HistoryMutationError;
use sona_core::history::mutation_service::HistoryMutationService;
use sona_core::history::query_service::HistoryQueryService;
use sona_core::history_store::{HistoryStore, HistoryStoreError};
use sona_runtime_fs::{SystemClock, UuidGenerator};
pub use sona_sqlite::history_store as sqlite_store;
use sona_sqlite::{SqliteApplicationContext, SqliteBackupStateRepository};
pub use sqlite_store::SqliteHistoryStore;
use std::sync::Arc;
use tauri::{AppHandle, Runtime};

use crate::platform::blocking::{
    map_err_string, spawn_blocking_map, with_sqlite_context_locked_transport,
    with_sqlite_context_transport,
};

// Re-exports from history platform adapter modules
pub use sona_core::history::transcript_diff::{
    build_transcript_diff, restore_transcript_diff_rows,
};
pub use sona_core::history::{
    BackupManifest, BackupManifestCounts, BackupManifestScopes, ExportBackupArchiveRequest,
    HistoryAudioCleanupReport, HistoryAudioCleanupRequest, HistoryAudioStatus,
    HistoryBackupSnapshot, HistoryCreateLiveDraftRequest, HistoryDraftSource, HistoryItemKind,
    HistoryItemRecord, HistoryItemStatus, HistoryListOptions, HistorySaveImportedFileRequest,
    HistorySaveRecordingRequest, HistoryWorkspaceDateFilter, HistoryWorkspaceFilterType,
    HistoryWorkspaceItemCounts, HistoryWorkspaceItemSearchMatch, HistoryWorkspaceQueryRequest,
    HistoryWorkspaceQueryResult, HistoryWorkspaceScope, HistoryWorkspaceSearchRange,
    HistoryWorkspaceSearchSnippet, HistoryWorkspaceSortOrder, HistoryWorkspaceSummary,
    LiveRecordingDraftResult, PreparedBackupImport, TranscriptDiffResult, TranscriptDiffRow,
    TranscriptDiffStatus, TranscriptSnapshotMetadata, TranscriptSnapshotReason,
    TranscriptSnapshotRecord,
};
pub use sona_core::history::{item_factory, transcript_payload, workspace_query};
pub use state::{HistoryRepositoryState, PreparedBackupImportState};

pub(crate) const HISTORY_DIR_NAME: &str = "history";

pub(crate) fn history_store(context: &SqliteApplicationContext) -> SqliteHistoryStore {
    context.history_store(Arc::new(SystemClock), Arc::new(UuidGenerator))
}

pub async fn run_history_db_task<R, T, F>(app: &AppHandle<R>, task: F) -> Result<T, String>
where
    R: Runtime,
    T: Send + Serialize + 'static,
    F: FnOnce(SqliteHistoryStore) -> Result<T, HistoryStoreError> + Send + 'static,
{
    with_sqlite_context_transport(app, move |context| task(history_store(&context))).await
}

pub async fn run_history_file_task<R, T, F>(
    app: &AppHandle<R>,
    state: &HistoryRepositoryState,
    task: F,
) -> Result<T, String>
where
    R: Runtime,
    T: Send + Serialize + 'static,
    F: FnOnce(SqliteHistoryStore) -> Result<T, HistoryStoreError> + Send + 'static,
{
    with_sqlite_context_locked_transport(app, state.file_lock.clone(), move |context| {
        task(history_store(&context))
    })
    .await
}

pub async fn run_history_query_file_task<R, T, F>(
    app: &AppHandle<R>,
    state: &HistoryRepositoryState,
    task: F,
) -> Result<T, String>
where
    R: Runtime,
    T: Send + Serialize + 'static,
    F: FnOnce(HistoryQueryService) -> Result<T, HistoryStoreError> + Send + 'static,
{
    with_sqlite_context_locked_transport(app, state.file_lock.clone(), move |context| {
        let repository = Arc::new(history_store(&context));
        task(HistoryQueryService::new(repository))
    })
    .await
}

pub async fn run_history_query_db_task<R, T, F>(app: &AppHandle<R>, task: F) -> Result<T, String>
where
    R: Runtime,
    T: Send + Serialize + 'static,
    F: FnOnce(HistoryQueryService) -> Result<T, HistoryStoreError> + Send + 'static,
{
    with_sqlite_context_transport(app, move |context| {
        let repository = Arc::new(history_store(&context));
        task(HistoryQueryService::new(repository))
    })
    .await
}

pub async fn run_history_mutation_file_task<R, T, F>(
    app: &AppHandle<R>,
    state: &HistoryRepositoryState,
    task: F,
) -> Result<T, String>
where
    R: Runtime,
    T: Send + Serialize + 'static,
    F: FnOnce(HistoryMutationService) -> Result<T, HistoryMutationError> + Send + 'static,
{
    with_sqlite_context_locked_transport(app, state.file_lock.clone(), move |context| {
        let repository = Arc::new(history_store(&context));
        task(HistoryMutationService::new(repository))
    })
    .await
}

pub async fn run_history_mutation_db_task<R, T, F>(app: &AppHandle<R>, task: F) -> Result<T, String>
where
    R: Runtime,
    T: Send + Serialize + 'static,
    F: FnOnce(HistoryMutationService) -> Result<T, HistoryMutationError> + Send + 'static,
{
    with_sqlite_context_transport(app, move |context| {
        let repository = Arc::new(history_store(&context));
        task(HistoryMutationService::new(repository))
    })
    .await
}

async fn run_backup_adapter_task<R, T, F>(
    app: &AppHandle<R>,
    state: &PreparedBackupImportState,
    task: F,
) -> Result<T, String>
where
    R: Runtime,
    T: Send + 'static,
    F: FnOnce(&FsBackupAdapter<SqliteBackupStateRepository, SystemClock>) -> Result<T, BackupError>
        + Send
        + 'static,
{
    let context = crate::platform::blocking::sqlite_context(app);
    let archive = state.archive();
    spawn_blocking_map(move || {
        let repository = context.backup_state_repository();
        let adapter = FsBackupAdapter::with_archive(archive, repository, SystemClock);
        task(&adapter)
    })
    .await
}

pub async fn export_backup_archive<R: Runtime>(
    app: &AppHandle<R>,
    state: &PreparedBackupImportState,
    request: ExportBackupArchiveRequest,
) -> Result<BackupManifest, String> {
    run_backup_adapter_task(app, state, move |adapter| {
        adapter.export_archive(BackupExportRequest {
            archive_path: request.archive_path,
            app_version: request.app_version,
        })
    })
    .await
}

pub async fn prepare_backup_import<R: Runtime>(
    app: &AppHandle<R>,
    state: &PreparedBackupImportState,
    archive_path: String,
) -> Result<PreparedBackupImport, String> {
    run_backup_adapter_task(app, state, move |adapter| {
        adapter.prepare_import(BackupPrepareImportRequest { archive_path })
    })
    .await
}

pub async fn apply_prepared_history_import<R: Runtime>(
    app: &AppHandle<R>,
    state: &PreparedBackupImportState,
    import_id: String,
) -> Result<(), String> {
    run_backup_adapter_task(app, state, move |adapter| {
        adapter
            .apply_prepared_import(BackupApplyPreparedImportRequest {
                import_id,
                default_rule_set_name: "Default Rules".to_string(),
            })
            .map(|_| ())
    })
    .await
}

pub async fn dispose_prepared_backup_import<R: Runtime>(
    app: &AppHandle<R>,
    state: &PreparedBackupImportState,
    import_id: String,
) -> Result<(), String> {
    run_backup_adapter_task(app, state, move |adapter| {
        adapter.dispose_prepared_import(&import_id)
    })
    .await
}

pub async fn open_history_folder<R: Runtime>(
    app: &AppHandle<R>,
    state: &HistoryRepositoryState,
) -> Result<(), String> {
    let context = crate::platform::blocking::sqlite_context(app);
    let app_local_data_dir = context.app_data_dir().to_path_buf();
    {
        let _guard = state.file_lock.lock().map_err(map_err_string)?;
        history_store(&context)
            .ensure_ready()
            .map_err(map_err_string)?;
    }

    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_path(
            app_local_data_dir.join(HISTORY_DIR_NAME).to_string_lossy(),
            None::<&str>,
        )
        .map_err(map_err_string)
}
