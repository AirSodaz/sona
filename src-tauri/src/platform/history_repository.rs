pub(crate) mod llm_helpers;
mod state;
pub use sona_sqlite::history_backup as backup;
pub use sona_sqlite::history_store as sqlite_store;
pub use sqlite_store::SqliteHistoryStore;
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
