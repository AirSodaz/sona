use crate::core::database::DatabaseError;
use crate::core::history::{
    HistoryAudioCleanupReport, HistoryAudioCleanupRequest, HistoryBackupSnapshot,
    HistoryCreateLiveDraftRequest, HistoryItemRecord, HistoryListOptions,
    HistorySaveImportedFileRequest, HistorySaveRecordingRequest, HistoryWorkspaceQueryRequest,
    HistoryWorkspaceQueryResult, LiveRecordingDraftResult, TranscriptSnapshotMetadata,
    TranscriptSnapshotReason, TranscriptSnapshotRecord,
};
use crate::core::transcript::TranscriptSegment;
use serde_json::Value;

pub trait HistoryStore: Send + Sync {
    fn ensure_ready(&self) -> Result<(), DatabaseError>;

    fn list_items(&self) -> Result<Vec<HistoryItemRecord>, DatabaseError>;

    fn list_items_with_reconciled_live_drafts(
        &self,
    ) -> Result<Vec<HistoryItemRecord>, DatabaseError>;

    fn list_items_paginated(
        &self,
        opts: HistoryListOptions,
    ) -> Result<Vec<HistoryItemRecord>, DatabaseError>;

    fn list_items_with_reconciled_live_drafts_paginated(
        &self,
        opts: HistoryListOptions,
    ) -> Result<Vec<HistoryItemRecord>, DatabaseError>;

    fn query_workspace(
        &self,
        request: HistoryWorkspaceQueryRequest,
    ) -> Result<HistoryWorkspaceQueryResult, DatabaseError>;

    fn create_live_draft(
        &self,
        request: HistoryCreateLiveDraftRequest,
    ) -> Result<LiveRecordingDraftResult, DatabaseError>;

    fn complete_live_draft(
        &self,
        history_id: &str,
        segments: Value,
        duration: f64,
    ) -> Result<HistoryItemRecord, DatabaseError>;

    fn save_recording(
        &self,
        request: HistorySaveRecordingRequest,
    ) -> Result<HistoryItemRecord, DatabaseError>;

    fn save_imported_file(
        &self,
        request: HistorySaveImportedFileRequest,
    ) -> Result<HistoryItemRecord, DatabaseError>;

    fn delete_items(&self, ids: &[String]) -> Result<(), DatabaseError>;

    fn load_transcript(
        &self,
        history_id: &str,
    ) -> Result<Option<Vec<TranscriptSegment>>, DatabaseError>;

    fn update_transcript(
        &self,
        history_id: &str,
        segments: Value,
    ) -> Result<HistoryItemRecord, DatabaseError>;

    fn create_transcript_snapshot(
        &self,
        history_id: &str,
        reason: TranscriptSnapshotReason,
        segments: Value,
    ) -> Result<TranscriptSnapshotMetadata, DatabaseError>;

    fn list_transcript_snapshots(
        &self,
        history_id: &str,
    ) -> Result<Vec<TranscriptSnapshotMetadata>, DatabaseError>;

    fn load_transcript_snapshot(
        &self,
        history_id: &str,
        snapshot_id: &str,
    ) -> Result<Option<TranscriptSnapshotRecord>, DatabaseError>;

    fn update_item_meta(&self, history_id: &str, updates: Value) -> Result<(), DatabaseError>;

    fn update_project_assignments(
        &self,
        ids: &[String],
        project_id: Option<String>,
    ) -> Result<(), DatabaseError>;

    fn reassign_project(
        &self,
        current_project_id: String,
        next_project_id: Option<String>,
    ) -> Result<(), DatabaseError>;

    fn load_summary(&self, history_id: &str) -> Result<Option<Value>, DatabaseError>;

    fn save_summary(&self, history_id: &str, summary_payload: Value) -> Result<(), DatabaseError>;

    fn delete_summary(&self, history_id: &str) -> Result<(), DatabaseError>;

    fn resolve_audio_path(&self, history_id: &str) -> Result<Option<String>, DatabaseError>;

    fn preview_audio_cleanup(
        &self,
        request: HistoryAudioCleanupRequest,
    ) -> Result<HistoryAudioCleanupReport, DatabaseError>;

    fn cleanup_audio(
        &self,
        request: HistoryAudioCleanupRequest,
    ) -> Result<HistoryAudioCleanupReport, DatabaseError>;

    fn history_snapshot_for_backup(&self) -> Result<HistoryBackupSnapshot, DatabaseError>;
}
