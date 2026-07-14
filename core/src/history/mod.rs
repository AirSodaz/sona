use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

use crate::transcription::transcript::TranscriptSegment;

pub use crate::backup::{
    BackupManifest, BackupManifestCounts, BackupManifestScopes, PreparedBackupImport,
};

#[derive(
    Clone, Copy, Debug, Deserialize, Serialize, PartialEq, strum::Display, strum::EnumString,
)]
#[serde(rename_all = "lowercase")]
pub enum HistoryItemKind {
    #[strum(serialize = "batch")]
    Batch,
    #[strum(serialize = "recording")]
    Recording,
}

#[derive(
    Clone, Copy, Debug, Deserialize, Serialize, PartialEq, strum::Display, strum::EnumString,
)]
#[serde(rename_all = "lowercase")]
pub enum HistoryItemStatus {
    #[strum(serialize = "draft")]
    Draft,
    #[strum(serialize = "complete")]
    Complete,
}

#[derive(
    Clone, Copy, Debug, Deserialize, Serialize, PartialEq, strum::Display, strum::EnumString,
)]
#[serde(rename_all = "lowercase")]
pub enum HistoryAudioStatus {
    #[strum(serialize = "available")]
    Available,
    #[strum(serialize = "missing")]
    Missing,
    #[strum(serialize = "removed")]
    Removed,
}

#[derive(
    Clone, Copy, Debug, Deserialize, Serialize, PartialEq, strum::Display, strum::EnumString,
)]
#[serde(rename_all = "snake_case")]
pub enum HistoryDraftSource {
    #[strum(serialize = "live_record")]
    LiveRecord,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItemRecord {
    pub id: String,
    pub timestamp: u64,
    pub duration: f64,
    pub audio_path: String,
    pub audio_status: HistoryAudioStatus,
    pub transcript_path: String,
    pub title: String,
    pub preview_text: String,
    pub icon: Option<String>,
    #[serde(rename = "type")]
    pub kind: HistoryItemKind,
    pub search_content: String,
    pub project_id: Option<String>,
    pub status: HistoryItemStatus,
    pub draft_source: Option<HistoryDraftSource>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum HistoryWorkspaceScope {
    All,
    Inbox,
    Project {
        #[serde(rename = "projectId")]
        project_id: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HistoryWorkspaceFilterType {
    All,
    Recording,
    Batch,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HistoryWorkspaceDateFilter {
    All,
    Today,
    Week,
    Month,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HistoryWorkspaceSortOrder {
    Newest,
    Oldest,
    DurationDesc,
    DurationAsc,
    TitleAsc,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryWorkspaceQueryRequest {
    pub scope: HistoryWorkspaceScope,
    pub query: String,
    pub filter_type: HistoryWorkspaceFilterType,
    pub date_filter: HistoryWorkspaceDateFilter,
    pub sort_order: HistoryWorkspaceSortOrder,
    pub limit: usize,
    pub offset: usize,
}

pub const MAX_WORKSPACE_QUERY_LIMIT: usize = 200;

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryWorkspaceSearchRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryWorkspaceSearchSnippet {
    pub text: String,
    pub highlight_start: usize,
    pub highlight_end: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryWorkspaceItemSearchMatch {
    pub matched_field: String,
    pub title_match: Option<HistoryWorkspaceSearchRange>,
    pub display_snippet: HistoryWorkspaceSearchSnippet,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct HistoryListOptions {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryWorkspaceSummary {
    pub total_items: usize,
    pub total_duration: f64,
    pub latest_timestamp: Option<u64>,
    pub recording_count: usize,
    pub batch_count: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryWorkspaceItemCounts {
    pub inbox: usize,
    pub by_project_id: BTreeMap<String, usize>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryWorkspaceQueryResult {
    pub filtered_items: Vec<HistoryItemRecord>,
    pub search_match_by_item_id: BTreeMap<String, Option<HistoryWorkspaceItemSearchMatch>>,
    pub filtered_item_count: usize,
    pub has_more: bool,
    pub summary: HistoryWorkspaceSummary,
    pub item_counts: HistoryWorkspaceItemCounts,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LiveRecordingDraftResult {
    pub item: HistoryItemRecord,
    pub audio_absolute_path: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryCreateLiveDraftRequest {
    pub id: Option<String>,
    pub audio_extension: String,
    pub project_id: Option<String>,
    pub icon: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistorySaveRecordingRequest {
    pub segments: Value,
    pub duration: f64,
    pub project_id: Option<String>,
    pub audio_bytes: Option<Vec<u8>>,
    pub native_audio_path: Option<String>,
    pub audio_extension: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistorySaveImportedFileRequest {
    pub id: Option<String>,
    pub source_path: String,
    pub segments: Value,
    pub duration: f64,
    pub project_id: Option<String>,
    pub converted_source_path: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryAudioCleanupRequest {
    pub retention_days: Option<u64>,
    pub exclude_history_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HistoryAudioCleanupReport {
    pub eligible_count: u64,
    pub removed_count: u64,
    pub removed_bytes: u64,
    pub missing_marked_count: u64,
    pub failed_count: u64,
    pub skipped_active_count: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptSnapshotReason {
    Polish,
    Translate,
    Retranscribe,
    Restore,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSnapshotMetadata {
    pub id: String,
    pub history_id: String,
    pub reason: TranscriptSnapshotReason,
    pub created_at: u64,
    pub segment_count: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSnapshotRecord {
    pub metadata: TranscriptSnapshotMetadata,
    pub segments: Vec<TranscriptSegment>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptDiffStatus {
    Unchanged,
    Modified,
    Added,
    Removed,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptDiffRow {
    pub id: String,
    pub status: TranscriptDiffStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot_segment: Option<TranscriptSegment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_segment: Option<TranscriptSegment>,
    pub snapshot_index: Option<usize>,
    pub current_index: Option<usize>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptDiffResult {
    pub rows: Vec<TranscriptDiffRow>,
    pub changed_count: usize,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportBackupArchiveRequest {
    pub archive_path: String,
    pub app_version: String,
    #[serde(default)]
    pub config: Value,
    #[serde(default)]
    pub projects: Vec<Value>,
    #[serde(default)]
    pub automation_rules: Vec<Value>,
    #[serde(default)]
    pub automation_processed_entries: Vec<Value>,
    #[serde(default)]
    pub analytics_content: String,
}

#[derive(Clone, Debug)]
pub struct HistoryBackupSnapshot {
    pub items: Vec<HistoryItemRecord>,
    pub transcript_files: Vec<(String, Value)>,
    pub summary_files: Vec<(String, Value)>,
    pub snapshot_files: Vec<(String, Value)>,
}

pub mod item_factory;
pub mod mutation_repository;
pub mod mutation_service;
pub mod query_repository;
pub mod query_service;
pub mod transcript_diff;
pub mod transcript_payload;
pub mod workspace_query;
