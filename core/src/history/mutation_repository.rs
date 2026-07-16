use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::history::{
    HistoryAudioStatus, HistoryCreateLiveDraftRequest, HistoryDraftSource, HistoryItemKind,
    HistoryItemRecord, HistoryItemStatus, HistorySaveImportedFileRequest,
    HistorySaveRecordingRequest, LiveRecordingDraftResult, TranscriptSnapshotMetadata,
    TranscriptSnapshotReason,
};
use crate::transcription::transcript::TranscriptSegment;

#[cfg(feature = "specta")]
use specta::Type;

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

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct HistoryCompleteLiveDraftRequest {
    pub history_id: String,
    pub segments: Vec<TranscriptSegment>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub duration: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct HistoryDeleteItemsRequest {
    pub ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct HistoryUpdateTranscriptRequest {
    pub history_id: String,
    pub segments: Vec<TranscriptSegment>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct HistoryCreateTranscriptSnapshotRequest {
    pub history_id: String,
    pub reason: TranscriptSnapshotReason,
    pub segments: Vec<TranscriptSegment>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistoryItemMetaPatch {
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Number>)
    )]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Number>)
    )]
    pub duration: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_status: Option<HistoryAudioStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcript_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_text: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    #[cfg_attr(feature = "specta", specta(type = Option<Option<String>>))]
    pub icon: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "type")]
    pub kind: Option<HistoryItemKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_content: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    #[cfg_attr(feature = "specta", specta(type = Option<Option<String>>))]
    pub project_id: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<HistoryItemStatus>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<Option<HistoryDraftSource>>)
    )]
    pub draft_source: Option<Option<HistoryDraftSource>>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct HistoryUpdateItemMetaRequest {
    pub history_id: String,
    pub updates: HistoryItemMetaPatch,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct HistoryUpdateProjectAssignmentsRequest {
    pub ids: Vec<String>,
    pub project_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
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
