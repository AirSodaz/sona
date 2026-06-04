mod backup;
pub(crate) mod commands;
pub(crate) mod fs_utils;
mod item_factory;
pub(crate) mod repository;
mod state;
pub use repository::HistoryRepository;
#[cfg(test)]
mod test_support;
mod transcript_diff;
mod transcript_payload;
mod types;
mod workspace_query;

pub use state::{HistoryRepositoryState, PreparedBackupImportState};
pub use types::{
    BackupManifest, BackupManifestCounts, BackupManifestScopes, ExportBackupArchiveRequest,
    HistoryCreateLiveDraftRequest, HistoryDraftSource, HistoryItemKind, HistoryItemRecord,
    HistoryItemStatus, HistorySaveImportedFileRequest, HistorySaveRecordingRequest,
    HistoryWorkspaceDateFilter, HistoryWorkspaceFilterType, HistoryWorkspaceItemCounts,
    HistoryWorkspaceItemSearchMatch, HistoryWorkspaceQueryRequest, HistoryWorkspaceQueryResult,
    HistoryWorkspaceScope, HistoryWorkspaceSearchRange, HistoryWorkspaceSearchSnippet,
    HistoryWorkspaceSortOrder, HistoryWorkspaceSummary, LiveRecordingDraftResult,
    PreparedBackupImport, TranscriptDiffResult, TranscriptDiffRow, TranscriptDiffStatus,
    TranscriptSnapshotMetadata, TranscriptSnapshotReason, TranscriptSnapshotRecord,
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

#[async_trait::async_trait]
impl crate::core::dashboard::ports::HistoryRepository for HistoryRepository {
    async fn read_history_items(
        &self,
    ) -> Result<
        Vec<crate::repositories::history::HistoryItemRecord>,
        crate::core::dashboard::error::DashboardServiceError,
    > {
        let items = self.list_items().map_err(|e| {
            crate::core::dashboard::error::DashboardServiceError::HistoryRepository(e)
        })?;
        Ok(items)
    }

    async fn read_transcript_segments(
        &self,
        transcript_path: &str,
    ) -> Result<
        Vec<crate::core::dashboard::models::ParsedTranscriptSegment>,
        crate::core::dashboard::error::DashboardServiceError,
    > {
        let segments = self.load_transcript(transcript_path).map_err(|e| {
            crate::core::dashboard::error::DashboardServiceError::HistoryRepository(e)
        })?;

        let Some(segments) = segments else {
            return Ok(Vec::new());
        };

        let parsed = segments
            .into_iter()
            .map(|s| {
                let speaker = s
                    .speaker
                    .map(|sp| crate::core::dashboard::models::SpeakerTag {
                        id: sp.id.clone(),
                        label: sp.label.clone(),
                        kind: if sp.kind == "identified" {
                            crate::core::dashboard::models::SpeakerKind::Identified
                        } else {
                            crate::core::dashboard::models::SpeakerKind::Anonymous
                        },
                    });
                crate::core::dashboard::models::ParsedTranscriptSegment {
                    text: s.text,
                    duration_seconds: (s.end - s.start).max(0.0) as f64,
                    speaker,
                }
            })
            .collect();

        Ok(parsed)
    }
}
