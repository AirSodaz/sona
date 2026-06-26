use crate::core::history_store::HistoryStore;
use crate::integrations::asr::TranscriptSegment;
use crate::repositories::history::{
    HistoryBackupSnapshot, HistoryCreateLiveDraftRequest, HistoryItemRecord,
    HistorySaveImportedFileRequest, HistorySaveRecordingRequest, HistoryWorkspaceQueryRequest,
    HistoryWorkspaceQueryResult, LiveRecordingDraftResult, TranscriptSnapshotMetadata,
    TranscriptSnapshotReason, TranscriptSnapshotRecord,
};
use serde_json::Value;
use std::path::PathBuf;

use super::repository::HistoryRepository;

#[derive(Clone)]
pub struct FileHistoryStore {
    repo: HistoryRepository,
}

impl FileHistoryStore {
    pub fn new(app_local_data_dir: PathBuf) -> Self {
        Self {
            repo: HistoryRepository::new(app_local_data_dir),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn write_index(&self, items: &[HistoryItemRecord]) -> Result<(), String> {
        self.repo.write_index(items)
    }

    #[allow(dead_code)]
    pub(crate) fn history_dir(&self) -> PathBuf {
        self.repo.history_dir()
    }
}

impl HistoryStore for FileHistoryStore {
    fn ensure_ready(&self) -> Result<(), String> {
        self.repo.ensure_ready()
    }

    fn list_items(&self) -> Result<Vec<HistoryItemRecord>, String> {
        self.repo.list_items()
    }

    fn list_items_with_reconciled_live_drafts(&self) -> Result<Vec<HistoryItemRecord>, String> {
        self.repo.list_items_with_reconciled_live_drafts()
    }

    fn query_workspace(
        &self,
        request: HistoryWorkspaceQueryRequest,
    ) -> Result<HistoryWorkspaceQueryResult, String> {
        self.repo.query_workspace(request)
    }

    fn create_live_draft(
        &self,
        request: HistoryCreateLiveDraftRequest,
    ) -> Result<LiveRecordingDraftResult, String> {
        self.repo.create_live_draft(request)
    }

    fn complete_live_draft(
        &self,
        history_id: &str,
        segments: Value,
        duration: f64,
    ) -> Result<HistoryItemRecord, String> {
        self.repo
            .complete_live_draft(history_id, segments, duration)
    }

    fn save_recording(
        &self,
        request: HistorySaveRecordingRequest,
    ) -> Result<HistoryItemRecord, String> {
        self.repo.save_recording(request)
    }

    fn save_imported_file(
        &self,
        request: HistorySaveImportedFileRequest,
    ) -> Result<HistoryItemRecord, String> {
        self.repo.save_imported_file(request)
    }

    fn delete_items(&self, ids: &[String]) -> Result<(), String> {
        self.repo.delete_items(ids)
    }

    fn load_transcript(&self, history_id: &str) -> Result<Option<Vec<TranscriptSegment>>, String> {
        let items = self.repo.list_items()?;
        let item = items
            .iter()
            .find(|entry| entry.id == history_id || entry.transcript_path == history_id)
            .ok_or_else(|| format!("History item not found: {history_id}"))?;
        self.repo.load_transcript(&item.transcript_path)
    }

    fn update_transcript(
        &self,
        history_id: &str,
        segments: Value,
    ) -> Result<HistoryItemRecord, String> {
        self.repo.update_transcript(history_id, segments)
    }

    fn create_transcript_snapshot(
        &self,
        history_id: &str,
        reason: TranscriptSnapshotReason,
        segments: Value,
    ) -> Result<TranscriptSnapshotMetadata, String> {
        self.repo
            .create_transcript_snapshot(history_id, reason, segments)
    }

    fn list_transcript_snapshots(
        &self,
        history_id: &str,
    ) -> Result<Vec<TranscriptSnapshotMetadata>, String> {
        self.repo.list_transcript_snapshots(history_id)
    }

    fn load_transcript_snapshot(
        &self,
        history_id: &str,
        snapshot_id: &str,
    ) -> Result<Option<TranscriptSnapshotRecord>, String> {
        self.repo.load_transcript_snapshot(history_id, snapshot_id)
    }

    fn update_item_meta(&self, history_id: &str, updates: Value) -> Result<(), String> {
        self.repo.update_item_meta(history_id, updates)
    }

    fn update_project_assignments(
        &self,
        ids: &[String],
        project_id: Option<String>,
    ) -> Result<(), String> {
        self.repo.update_project_assignments(ids, project_id)
    }

    fn reassign_project(
        &self,
        current_project_id: String,
        next_project_id: Option<String>,
    ) -> Result<(), String> {
        self.repo
            .reassign_project(current_project_id, next_project_id)
    }

    fn load_summary(&self, history_id: &str) -> Result<Option<Value>, String> {
        self.repo.load_summary(history_id)
    }

    fn save_summary(&self, history_id: &str, summary_payload: Value) -> Result<(), String> {
        self.repo.save_summary(history_id, summary_payload)
    }

    fn delete_summary(&self, history_id: &str) -> Result<(), String> {
        self.repo.delete_summary(history_id)
    }

    fn resolve_audio_path(&self, history_id: &str) -> Result<Option<String>, String> {
        let items = self.repo.list_items()?;
        let item = items
            .iter()
            .find(|entry| entry.id == history_id || entry.audio_path == history_id)
            .ok_or_else(|| format!("History item not found: {history_id}"))?;
        self.repo.resolve_audio_path(&item.audio_path)
    }

    fn history_snapshot_for_backup(&self) -> Result<HistoryBackupSnapshot, String> {
        self.repo.history_snapshot_for_backup()
    }
}
