pub(crate) mod llm_helpers;
mod state;
use serde::Serialize;
use sona_application::history::HistoryMutationService;
use sona_application::history::HistoryQueryService;
use sona_archive::FsBackupAdapter;
use sona_core::backup::{
    BackupApplyPreparedImportRequest, BackupError, BackupExportRequest, BackupPrepareImportRequest,
};
use sona_core::history::mutation_repository::HistoryMutationError;
use sona_core::history_store::{HistoryStore, HistoryStoreError};
use sona_runtime_fs::{SystemClock, UuidGenerator};
pub use sona_sqlite::history_store as sqlite_store;
use sona_sqlite::{SqliteApplicationContext, SqliteBackupStateRepository};
pub use sqlite_store::SqliteHistoryStore;
use std::sync::Arc;

use crate::platform::database::DesktopSqliteState;
use crate::services::db_runner::{
    map_err_string, run_sqlite_task_locked_transport, run_sqlite_task_transport, spawn_blocking_map,
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

/// Pure Rust History domain service without any Tauri dependency.
#[derive(Clone)]
pub struct HistoryService {
    sqlite: DesktopSqliteState,
    state: HistoryRepositoryState,
    backup_state: PreparedBackupImportState,
}

impl HistoryService {
    pub fn new(
        sqlite: DesktopSqliteState,
        state: HistoryRepositoryState,
        backup_state: PreparedBackupImportState,
    ) -> Self {
        Self {
            sqlite,
            state,
            backup_state,
        }
    }

    pub fn sqlite(&self) -> &DesktopSqliteState {
        &self.sqlite
    }

    pub fn state(&self) -> &HistoryRepositoryState {
        &self.state
    }

    pub fn backup_state(&self) -> &PreparedBackupImportState {
        &self.backup_state
    }

    pub fn is_file_task_active(&self) -> bool {
        self.state.is_file_task_active()
    }

    pub async fn query_db<T, F>(&self, task: F) -> Result<T, String>
    where
        T: Send + Serialize + 'static,
        F: FnOnce(HistoryQueryService) -> Result<T, HistoryStoreError> + Send + 'static,
    {
        run_history_query_db_task(&self.sqlite, task).await
    }

    pub async fn mutation_db<T, F>(&self, task: F) -> Result<T, String>
    where
        T: Send + Serialize + 'static,
        F: FnOnce(HistoryMutationService) -> Result<T, HistoryMutationError> + Send + 'static,
    {
        run_history_mutation_db_task(&self.sqlite, task).await
    }

    pub async fn mutation_file<T, F>(&self, task: F) -> Result<T, String>
    where
        T: Send + Serialize + 'static,
        F: FnOnce(HistoryMutationService) -> Result<T, HistoryMutationError> + Send + 'static,
    {
        run_history_mutation_file_task(&self.sqlite, &self.state, task).await
    }

    pub async fn file_task<T, F>(&self, task: F) -> Result<T, String>
    where
        T: Send + Serialize + 'static,
        F: FnOnce(SqliteHistoryStore) -> Result<T, HistoryStoreError> + Send + 'static,
    {
        run_history_file_task(&self.sqlite, &self.state, task).await
    }

    pub async fn db_task<T, F>(&self, task: F) -> Result<T, String>
    where
        T: Send + Serialize + 'static,
        F: FnOnce(SqliteHistoryStore) -> Result<T, HistoryStoreError> + Send + 'static,
    {
        run_history_db_task(&self.sqlite, task).await
    }

    pub async fn export_backup_archive(
        &self,
        request: ExportBackupArchiveRequest,
    ) -> Result<BackupManifest, String> {
        export_backup_archive(&self.sqlite, &self.backup_state, request).await
    }

    pub async fn prepare_backup_import(
        &self,
        archive_path: String,
    ) -> Result<PreparedBackupImport, String> {
        prepare_backup_import(&self.sqlite, &self.backup_state, archive_path).await
    }

    pub async fn apply_prepared_history_import(&self, import_id: String) -> Result<(), String> {
        apply_prepared_history_import(&self.sqlite, &self.backup_state, import_id).await
    }

    pub async fn dispose_prepared_backup_import(&self, import_id: String) -> Result<(), String> {
        dispose_prepared_backup_import(&self.sqlite, &self.backup_state, import_id).await
    }

    pub fn ensure_history_folder(&self) -> Result<std::path::PathBuf, String> {
        ensure_history_folder(&self.sqlite, &self.state)
    }
}

pub async fn run_history_db_task<T, F>(sqlite: &DesktopSqliteState, task: F) -> Result<T, String>
where
    T: Send + Serialize + 'static,
    F: FnOnce(SqliteHistoryStore) -> Result<T, HistoryStoreError> + Send + 'static,
{
    run_sqlite_task_transport(sqlite, move |context| task(history_store(&context))).await
}

pub async fn run_history_file_task<T, F>(
    sqlite: &DesktopSqliteState,
    state: &HistoryRepositoryState,
    task: F,
) -> Result<T, String>
where
    T: Send + Serialize + 'static,
    F: FnOnce(SqliteHistoryStore) -> Result<T, HistoryStoreError> + Send + 'static,
{
    run_sqlite_task_locked_transport(sqlite, state.file_lock.clone(), move |context| {
        task(history_store(&context))
    })
    .await
}

pub async fn run_history_query_db_task<T, F>(
    sqlite: &DesktopSqliteState,
    task: F,
) -> Result<T, String>
where
    T: Send + Serialize + 'static,
    F: FnOnce(HistoryQueryService) -> Result<T, HistoryStoreError> + Send + 'static,
{
    run_sqlite_task_transport(sqlite, move |context| {
        let repository = Arc::new(history_store(&context));
        task(HistoryQueryService::new(repository))
    })
    .await
}

pub async fn run_history_mutation_file_task<T, F>(
    sqlite: &DesktopSqliteState,
    state: &HistoryRepositoryState,
    task: F,
) -> Result<T, String>
where
    T: Send + Serialize + 'static,
    F: FnOnce(HistoryMutationService) -> Result<T, HistoryMutationError> + Send + 'static,
{
    run_sqlite_task_locked_transport(sqlite, state.file_lock.clone(), move |context| {
        let repository = Arc::new(history_store(&context));
        task(HistoryMutationService::new(repository))
    })
    .await
}

pub async fn run_history_mutation_db_task<T, F>(
    sqlite: &DesktopSqliteState,
    task: F,
) -> Result<T, String>
where
    T: Send + Serialize + 'static,
    F: FnOnce(HistoryMutationService) -> Result<T, HistoryMutationError> + Send + 'static,
{
    run_sqlite_task_transport(sqlite, move |context| {
        let repository = Arc::new(history_store(&context));
        task(HistoryMutationService::new(repository))
    })
    .await
}

async fn run_backup_adapter_task<T, F>(
    sqlite: &DesktopSqliteState,
    state: &PreparedBackupImportState,
    task: F,
) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&FsBackupAdapter<SqliteBackupStateRepository, SystemClock>) -> Result<T, BackupError>
        + Send
        + 'static,
{
    let context = sqlite.current_context()?;
    let archive = state.archive();
    spawn_blocking_map(move || {
        let repository = context.backup_state_repository();
        let adapter = FsBackupAdapter::with_archive(archive, repository, SystemClock);
        task(&adapter)
    })
    .await
}

pub async fn export_backup_archive(
    sqlite: &DesktopSqliteState,
    state: &PreparedBackupImportState,
    request: ExportBackupArchiveRequest,
) -> Result<BackupManifest, String> {
    run_backup_adapter_task(sqlite, state, move |adapter| {
        adapter.export_archive(BackupExportRequest {
            archive_path: request.archive_path,
            app_version: request.app_version,
        })
    })
    .await
}

pub async fn prepare_backup_import(
    sqlite: &DesktopSqliteState,
    state: &PreparedBackupImportState,
    archive_path: String,
) -> Result<PreparedBackupImport, String> {
    run_backup_adapter_task(sqlite, state, move |adapter| {
        adapter.prepare_import(BackupPrepareImportRequest { archive_path })
    })
    .await
}

pub async fn apply_prepared_history_import(
    sqlite: &DesktopSqliteState,
    state: &PreparedBackupImportState,
    import_id: String,
) -> Result<(), String> {
    run_backup_adapter_task(sqlite, state, move |adapter| {
        adapter
            .apply_prepared_import(BackupApplyPreparedImportRequest {
                import_id,
                default_rule_set_name: "Default Rules".to_string(),
            })
            .map(|_| ())
    })
    .await
}

pub async fn dispose_prepared_backup_import(
    sqlite: &DesktopSqliteState,
    state: &PreparedBackupImportState,
    import_id: String,
) -> Result<(), String> {
    run_backup_adapter_task(sqlite, state, move |adapter| {
        adapter.dispose_prepared_import(&import_id)
    })
    .await
}

/// Prepares and returns the history directory path without invoking OS opener.
pub fn ensure_history_folder(
    sqlite: &DesktopSqliteState,
    state: &HistoryRepositoryState,
) -> Result<std::path::PathBuf, String> {
    let context = sqlite.current_context()?;
    let app_local_data_dir = context.app_data_dir().to_path_buf();
    {
        let _guard = state.file_lock.lock().map_err(map_err_string)?;
        history_store(&context)
            .ensure_ready()
            .map_err(map_err_string)?;
    }
    Ok(app_local_data_dir.join(HISTORY_DIR_NAME))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_history_service_without_tauri() {
        let temp = tempfile::tempdir().unwrap();
        let ctx = Arc::new(sona_sqlite::SqliteApplicationContext::open(temp.path()).unwrap());
        let sqlite = DesktopSqliteState::new(ctx);
        let repo_state = HistoryRepositoryState::default();
        let backup_state = PreparedBackupImportState::default();
        let history_service = HistoryService::new(sqlite, repo_state, backup_state);

        let items = history_service
            .query_db(|service| service.list_items(HistoryListOptions::default()))
            .await
            .unwrap();
        assert_eq!(items.len(), 0);

        let folder = history_service.ensure_history_folder().unwrap();
        assert!(folder.ends_with(HISTORY_DIR_NAME));
    }
}
