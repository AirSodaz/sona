pub mod backup;
pub(crate) mod llm_helpers;
pub(crate) mod repository;
mod state;
pub use repository::HistoryRepository;
pub use sona_sqlite::history_store as sqlite_store;
pub use sqlite_store::SqliteHistoryStore;
#[cfg(test)]
pub(crate) mod test_support;
mod types;

// Re-exports from sona-core history submodules
pub(crate) use sona_core::history::fs_utils;
pub use sona_core::history::{item_factory, transcript_diff, transcript_payload, workspace_query};

pub use state::{HistoryRepositoryState, PreparedBackupImportState};
pub use types::{
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

pub(crate) const HISTORY_DIR_NAME: &str = "history";
pub(crate) const HISTORY_INDEX_FILE_NAME: &str = "index.json";
pub(crate) const SUMMARY_FILE_SUFFIX: &str = ".summary.json";
pub(crate) const HISTORY_VERSIONS_DIR_NAME: &str = "versions";
pub(crate) const CONFIG_DIR_NAME: &str = "config";
pub(crate) const CONFIG_FILE_NAME: &str = "sona-config.json";
pub(crate) const PROJECTS_DIR_NAME: &str = "projects";
pub(crate) const PROJECTS_INDEX_FILE_NAME: &str = "index.json";
pub(crate) const AUTOMATION_DIR_NAME: &str = "automation";
pub(crate) const AUTOMATION_RULES_FILE_NAME: &str = "rules.json";
pub(crate) const AUTOMATION_PROCESSED_FILE_NAME: &str = "processed.json";
pub(crate) const ANALYTICS_DIR_NAME: &str = "analytics";
pub(crate) const ANALYTICS_USAGE_FILE_NAME: &str = "llm-usage.json";

pub(crate) const BACKUP_SCHEMA_VERSION: u64 = 1;
pub(crate) const BACKUP_HISTORY_MODE: &str = "light";
