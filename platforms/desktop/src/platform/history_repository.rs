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
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Runtime};

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

fn validate_history_transport<T: Serialize>(value: T) -> Result<T, String> {
    sona_ts_bind::validate_typescript_safe_integers(&value).map_err(|error| error.to_string())?;
    Ok(value)
}

async fn run_history_file_task_inner<T, F>(
    context: Arc<SqliteApplicationContext>,
    lock: Arc<Mutex<()>>,
    task: F,
) -> Result<T, String>
where
    T: Send + Serialize + 'static,
    F: FnOnce(SqliteHistoryStore) -> Result<T, HistoryStoreError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = lock.lock().map_err(|error| error.to_string())?;
        task(history_store(&context)).map_err(|e| e.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

pub async fn run_history_db_task<R, T, F>(app: &AppHandle<R>, task: F) -> Result<T, String>
where
    R: Runtime,
    T: Send + Serialize + 'static,
    F: FnOnce(SqliteHistoryStore) -> Result<T, HistoryStoreError> + Send + 'static,
{
    let context = crate::platform::database::sqlite_application_context(app);
    let result = llm_helpers::run_llm_db_task(context, task).await?;
    validate_history_transport(result)
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
    let context = crate::platform::database::sqlite_application_context(app);
    let result = run_history_file_task_inner(context, state.file_lock.clone(), task).await?;
    validate_history_transport(result)
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
    let context = crate::platform::database::sqlite_application_context(app);
    let lock = state.file_lock.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _guard = lock.lock().map_err(|error| error.to_string())?;
        let repository = Arc::new(history_store(&context));
        task(HistoryQueryService::new(repository)).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())??;
    validate_history_transport(result)
}

pub async fn run_history_query_db_task<R, T, F>(app: &AppHandle<R>, task: F) -> Result<T, String>
where
    R: Runtime,
    T: Send + Serialize + 'static,
    F: FnOnce(HistoryQueryService) -> Result<T, HistoryStoreError> + Send + 'static,
{
    let context = crate::platform::database::sqlite_application_context(app);
    let result = tauri::async_runtime::spawn_blocking(move || {
        let repository = Arc::new(history_store(&context));
        task(HistoryQueryService::new(repository)).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())??;
    validate_history_transport(result)
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
    let context = crate::platform::database::sqlite_application_context(app);
    let lock = state.file_lock.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _guard = lock.lock().map_err(|error| error.to_string())?;
        let repository = Arc::new(history_store(&context));
        task(HistoryMutationService::new(repository)).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())??;
    validate_history_transport(result)
}

pub async fn run_history_mutation_db_task<R, T, F>(app: &AppHandle<R>, task: F) -> Result<T, String>
where
    R: Runtime,
    T: Send + Serialize + 'static,
    F: FnOnce(HistoryMutationService) -> Result<T, HistoryMutationError> + Send + 'static,
{
    let context = crate::platform::database::sqlite_application_context(app);
    let result = tauri::async_runtime::spawn_blocking(move || {
        let repository = Arc::new(history_store(&context));
        task(HistoryMutationService::new(repository)).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())??;
    validate_history_transport(result)
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
    let context = crate::platform::database::sqlite_application_context(app);
    let archive = state.archive();
    tauri::async_runtime::spawn_blocking(move || {
        let repository = context.backup_state_repository();
        let adapter = FsBackupAdapter::with_archive(archive, repository, SystemClock);
        task(&adapter).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
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
    let context = crate::platform::database::sqlite_application_context(app);
    let app_local_data_dir = context.app_data_dir().to_path_buf();
    {
        let _guard = state.file_lock.lock().map_err(|error| error.to_string())?;
        history_store(&context)
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
