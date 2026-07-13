use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;

use crate::history::{
    HistoryCreateLiveDraftRequest, HistoryItemRecord, HistorySaveImportedFileRequest,
    HistorySaveRecordingRequest, LiveRecordingDraftResult, TranscriptSnapshotMetadata,
    TranscriptSnapshotReason,
};

#[derive(Debug, Error)]
pub enum HistoryMutationError {
    #[error("Invalid history mutation: {0}")]
    InvalidRequest(String),
    #[error("History record not found: {0}")]
    NotFound(String),
    #[error("Database error: {0}")]
    Database(String),
    #[error("{0}")]
    Internal(String),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryCompleteLiveDraftRequest {
    pub history_id: String,
    pub segments: Value,
    pub duration: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryDeleteItemsRequest {
    pub ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryUpdateTranscriptRequest {
    pub history_id: String,
    pub segments: Value,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryCreateTranscriptSnapshotRequest {
    pub history_id: String,
    pub reason: TranscriptSnapshotReason,
    pub segments: Value,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryUpdateItemMetaRequest {
    pub history_id: String,
    pub updates: Value,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryUpdateProjectAssignmentsRequest {
    pub ids: Vec<String>,
    pub project_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryReassignProjectRequest {
    pub current_project_id: String,
    pub next_project_id: Option<String>,
}

pub trait HistoryMutationRepository: Send + Sync {
    fn create_live_draft(
        &self,
        request: HistoryCreateLiveDraftRequest,
    ) -> Result<LiveRecordingDraftResult, HistoryMutationError>;

    fn complete_live_draft(
        &self,
        request: HistoryCompleteLiveDraftRequest,
    ) -> Result<HistoryItemRecord, HistoryMutationError>;

    fn save_recording(
        &self,
        request: HistorySaveRecordingRequest,
    ) -> Result<HistoryItemRecord, HistoryMutationError>;

    fn save_imported_file(
        &self,
        request: HistorySaveImportedFileRequest,
    ) -> Result<HistoryItemRecord, HistoryMutationError>;

    fn delete_items(&self, request: HistoryDeleteItemsRequest) -> Result<(), HistoryMutationError>;

    fn update_transcript(
        &self,
        request: HistoryUpdateTranscriptRequest,
    ) -> Result<HistoryItemRecord, HistoryMutationError>;

    fn create_transcript_snapshot(
        &self,
        request: HistoryCreateTranscriptSnapshotRequest,
    ) -> Result<TranscriptSnapshotMetadata, HistoryMutationError>;

    fn update_item_meta(
        &self,
        request: HistoryUpdateItemMetaRequest,
    ) -> Result<(), HistoryMutationError>;

    fn update_project_assignments(
        &self,
        request: HistoryUpdateProjectAssignmentsRequest,
    ) -> Result<(), HistoryMutationError>;

    fn reassign_project(
        &self,
        request: HistoryReassignProjectRequest,
    ) -> Result<(), HistoryMutationError>;
}
