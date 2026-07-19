use crate::history::{
    HistoryItemRecord, HistoryListOptions, HistoryWorkspaceQueryRequest,
    HistoryWorkspaceQueryResult, TranscriptSnapshotMetadata, TranscriptSnapshotRecord,
};
use crate::transcription::transcript::TranscriptSegment;
use thiserror::Error;

use crate::ports::fs::FileSystemError;
use crate::ports::time::ClockError;

#[derive(Debug, Error)]
pub enum HistoryQueryError {
    #[error("Invalid history query: {0}")]
    InvalidRequest(String),
    #[error("Database error: {0}")]
    Database(String),
    #[error("{0}")]
    Internal(String),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error(transparent)]
    Clock(#[from] ClockError),
    #[error(transparent)]
    FileSystem(#[from] FileSystemError),
}

pub trait HistoryQueryRepository: Send + Sync {
    fn list_items(&self) -> Result<Vec<HistoryItemRecord>, HistoryQueryError>;

    fn list_items_with_reconciled_live_drafts(
        &self,
    ) -> Result<Vec<HistoryItemRecord>, HistoryQueryError>;

    fn list_items_paginated(
        &self,
        opts: HistoryListOptions,
    ) -> Result<Vec<HistoryItemRecord>, HistoryQueryError>;

    fn list_items_with_reconciled_live_drafts_paginated(
        &self,
        opts: HistoryListOptions,
    ) -> Result<Vec<HistoryItemRecord>, HistoryQueryError>;

    fn query_workspace(
        &self,
        request: HistoryWorkspaceQueryRequest,
    ) -> Result<HistoryWorkspaceQueryResult, HistoryQueryError>;

    fn load_transcript(
        &self,
        history_id: &str,
    ) -> Result<Option<Vec<TranscriptSegment>>, HistoryQueryError>;

    fn list_transcript_snapshots(
        &self,
        history_id: &str,
    ) -> Result<Vec<TranscriptSnapshotMetadata>, HistoryQueryError>;

    fn load_transcript_snapshot(
        &self,
        history_id: &str,
        snapshot_id: &str,
    ) -> Result<Option<TranscriptSnapshotRecord>, HistoryQueryError>;
}
