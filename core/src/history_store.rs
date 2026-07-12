use crate::history::{
    HistoryAudioCleanupReport, HistoryAudioCleanupRequest, HistoryBackupSnapshot,
    HistoryCreateLiveDraftRequest, HistoryItemRecord, HistoryListOptions,
    HistorySaveImportedFileRequest, HistorySaveRecordingRequest, HistoryWorkspaceQueryRequest,
    HistoryWorkspaceQueryResult, LiveRecordingDraftResult, TranscriptSnapshotMetadata,
    TranscriptSnapshotReason, TranscriptSnapshotRecord,
};
use crate::transcription::transcript::TranscriptSegment;
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HistoryStoreError {
    #[error("Invalid history query: {0}")]
    InvalidRequest(String),
    #[error("Database error: {0}")]
    Database(String),
    #[error("{0}")]
    Internal(String),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub trait HistoryStore: Send + Sync {
    fn ensure_ready(&self) -> Result<(), HistoryStoreError>;
    fn list_items(&self) -> Result<Vec<HistoryItemRecord>, HistoryStoreError>;
    fn list_items_with_reconciled_live_drafts(
        &self,
    ) -> Result<Vec<HistoryItemRecord>, HistoryStoreError>;
    fn list_items_paginated(
        &self,
        opts: HistoryListOptions,
    ) -> Result<Vec<HistoryItemRecord>, HistoryStoreError>;
    fn list_items_with_reconciled_live_drafts_paginated(
        &self,
        opts: HistoryListOptions,
    ) -> Result<Vec<HistoryItemRecord>, HistoryStoreError>;
    fn query_workspace(
        &self,
        request: HistoryWorkspaceQueryRequest,
    ) -> Result<HistoryWorkspaceQueryResult, HistoryStoreError>;
    fn create_live_draft(
        &self,
        request: HistoryCreateLiveDraftRequest,
    ) -> Result<LiveRecordingDraftResult, HistoryStoreError>;
    fn complete_live_draft(
        &self,
        history_id: &str,
        segments: Value,
        duration: f64,
    ) -> Result<HistoryItemRecord, HistoryStoreError>;
    fn save_recording(
        &self,
        request: HistorySaveRecordingRequest,
    ) -> Result<HistoryItemRecord, HistoryStoreError>;
    fn save_imported_file(
        &self,
        request: HistorySaveImportedFileRequest,
    ) -> Result<HistoryItemRecord, HistoryStoreError>;
    fn delete_items(&self, ids: &[String]) -> Result<(), HistoryStoreError>;
    fn load_transcript(
        &self,
        history_id: &str,
    ) -> Result<Option<Vec<TranscriptSegment>>, HistoryStoreError>;
    fn update_transcript(
        &self,
        history_id: &str,
        segments: Value,
    ) -> Result<HistoryItemRecord, HistoryStoreError>;
    fn create_transcript_snapshot(
        &self,
        history_id: &str,
        reason: TranscriptSnapshotReason,
        segments: Value,
    ) -> Result<TranscriptSnapshotMetadata, HistoryStoreError>;
    fn list_transcript_snapshots(
        &self,
        history_id: &str,
    ) -> Result<Vec<TranscriptSnapshotMetadata>, HistoryStoreError>;
    fn load_transcript_snapshot(
        &self,
        history_id: &str,
        snapshot_id: &str,
    ) -> Result<Option<TranscriptSnapshotRecord>, HistoryStoreError>;
    fn update_item_meta(&self, history_id: &str, updates: Value) -> Result<(), HistoryStoreError>;
    fn update_project_assignments(
        &self,
        ids: &[String],
        project_id: Option<String>,
    ) -> Result<(), HistoryStoreError>;
    fn reassign_project(
        &self,
        current_project_id: String,
        next_project_id: Option<String>,
    ) -> Result<(), HistoryStoreError>;
    fn load_summary(&self, history_id: &str) -> Result<Option<Value>, HistoryStoreError>;
    fn save_summary(
        &self,
        history_id: &str,
        summary_payload: Value,
    ) -> Result<(), HistoryStoreError>;
    fn delete_summary(&self, history_id: &str) -> Result<(), HistoryStoreError>;
    fn resolve_audio_path(&self, history_id: &str) -> Result<Option<String>, HistoryStoreError>;
    fn preview_audio_cleanup(
        &self,
        request: HistoryAudioCleanupRequest,
    ) -> Result<HistoryAudioCleanupReport, HistoryStoreError>;
    fn cleanup_audio(
        &self,
        request: HistoryAudioCleanupRequest,
    ) -> Result<HistoryAudioCleanupReport, HistoryStoreError>;
    fn history_snapshot_for_backup(&self) -> Result<HistoryBackupSnapshot, HistoryStoreError>;
}
