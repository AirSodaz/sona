use crate::integrations::asr::TranscriptSegment;
use crate::repositories::history::{
    HistoryBackupSnapshot, HistoryCreateLiveDraftRequest, HistoryItemRecord, HistoryListOptions,
    HistorySaveImportedFileRequest, HistorySaveRecordingRequest, HistoryWorkspaceQueryRequest,
    HistoryWorkspaceQueryResult, LiveRecordingDraftResult, TranscriptSnapshotMetadata,
    TranscriptSnapshotReason, TranscriptSnapshotRecord,
};
use serde_json::Value;

pub trait HistoryStore: Send + Sync {
    fn ensure_ready(&self) -> Result<(), String>;

    fn list_items(&self) -> Result<Vec<HistoryItemRecord>, String>;

    fn list_items_with_reconciled_live_drafts(&self) -> Result<Vec<HistoryItemRecord>, String>;

    fn list_items_paginated(
        &self,
        opts: HistoryListOptions,
    ) -> Result<Vec<HistoryItemRecord>, String>;

    fn list_items_with_reconciled_live_drafts_paginated(
        &self,
        opts: HistoryListOptions,
    ) -> Result<Vec<HistoryItemRecord>, String>;

    fn query_workspace(
        &self,
        request: HistoryWorkspaceQueryRequest,
    ) -> Result<HistoryWorkspaceQueryResult, String>;

    fn create_live_draft(
        &self,
        request: HistoryCreateLiveDraftRequest,
    ) -> Result<LiveRecordingDraftResult, String>;

    fn complete_live_draft(
        &self,
        history_id: &str,
        segments: Value,
        duration: f64,
    ) -> Result<HistoryItemRecord, String>;

    fn save_recording(
        &self,
        request: HistorySaveRecordingRequest,
    ) -> Result<HistoryItemRecord, String>;

    fn save_imported_file(
        &self,
        request: HistorySaveImportedFileRequest,
    ) -> Result<HistoryItemRecord, String>;

    fn delete_items(&self, ids: &[String]) -> Result<(), String>;

    fn load_transcript(&self, history_id: &str) -> Result<Option<Vec<TranscriptSegment>>, String>;

    fn update_transcript(
        &self,
        history_id: &str,
        segments: Value,
    ) -> Result<HistoryItemRecord, String>;

    fn create_transcript_snapshot(
        &self,
        history_id: &str,
        reason: TranscriptSnapshotReason,
        segments: Value,
    ) -> Result<TranscriptSnapshotMetadata, String>;

    fn list_transcript_snapshots(
        &self,
        history_id: &str,
    ) -> Result<Vec<TranscriptSnapshotMetadata>, String>;

    fn load_transcript_snapshot(
        &self,
        history_id: &str,
        snapshot_id: &str,
    ) -> Result<Option<TranscriptSnapshotRecord>, String>;

    fn update_item_meta(&self, history_id: &str, updates: Value) -> Result<(), String>;

    fn update_project_assignments(
        &self,
        ids: &[String],
        project_id: Option<String>,
    ) -> Result<(), String>;

    fn reassign_project(
        &self,
        current_project_id: String,
        next_project_id: Option<String>,
    ) -> Result<(), String>;

    fn load_summary(&self, history_id: &str) -> Result<Option<Value>, String>;

    fn save_summary(&self, history_id: &str, summary_payload: Value) -> Result<(), String>;

    fn delete_summary(&self, history_id: &str) -> Result<(), String>;

    fn resolve_audio_path(&self, history_id: &str) -> Result<Option<String>, String>;

    fn history_snapshot_for_backup(&self) -> Result<HistoryBackupSnapshot, String>;
}
