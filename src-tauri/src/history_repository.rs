mod backup;
pub(crate) mod commands;
mod fs_utils;
mod repository;
mod state;
#[cfg(test)]
mod test_support;
mod types;

pub use state::{HistoryRepositoryState, PreparedBackupImportState};
pub use types::{
    BackupManifest, BackupManifestCounts, BackupManifestScopes, ExportBackupArchiveRequest,
    HistoryDraftSource, HistoryItemKind, HistoryItemRecord, HistoryItemStatus,
    LiveRecordingDraftResult, PreparedBackupImport, TranscriptSnapshotMetadata,
    TranscriptSnapshotReason, TranscriptSnapshotRecord,
};

pub(super) const HISTORY_DIR_NAME: &str = "history";
pub(super) const HISTORY_INDEX_FILE_NAME: &str = "index.json";
pub(super) const SUMMARY_FILE_SUFFIX: &str = ".summary.json";
pub(super) const HISTORY_VERSIONS_DIR_NAME: &str = "versions";
pub(super) const TRANSCRIPT_SNAPSHOT_RETENTION_LIMIT: usize = 20;

pub(super) const CONFIG_DIR_NAME: &str = "config";
pub(super) const CONFIG_FILE_NAME: &str = "sona-config.json";
pub(super) const PROJECTS_DIR_NAME: &str = "projects";
pub(super) const PROJECTS_INDEX_FILE_NAME: &str = "index.json";
pub(super) const AUTOMATION_DIR_NAME: &str = "automation";
pub(super) const AUTOMATION_RULES_FILE_NAME: &str = "rules.json";
pub(super) const AUTOMATION_PROCESSED_FILE_NAME: &str = "processed.json";
pub(super) const ANALYTICS_DIR_NAME: &str = "analytics";
pub(super) const ANALYTICS_USAGE_FILE_NAME: &str = "llm-usage.json";

pub(super) const BACKUP_SCHEMA_VERSION: u64 = 1;
pub(super) const BACKUP_HISTORY_MODE: &str = "light";
