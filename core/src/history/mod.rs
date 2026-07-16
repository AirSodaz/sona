use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

use crate::transcription::transcript::TranscriptSegment;

#[cfg(feature = "specta")]
use specta::Type;

pub use crate::backup::{
    BackupManifest, BackupManifestCounts, BackupManifestScopes, PreparedBackupImport,
};

#[derive(
    Clone, Copy, Debug, Deserialize, Serialize, PartialEq, strum::Display, strum::EnumString,
)]
#[cfg_attr(feature = "specta", derive(Type))]
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
#[cfg_attr(feature = "specta", derive(Type))]
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
#[cfg_attr(feature = "specta", derive(Type))]
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
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "snake_case")]
pub enum HistoryDraftSource {
    #[strum(serialize = "live_record")]
    LiveRecord,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct HistoryItemRecord {
    pub id: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub timestamp: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
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
    #[serde(default)]
    pub tag_ids: Vec<String>,
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Number>)
    )]
    #[serde(default)]
    pub deleted_at: Option<u64>,
    pub status: HistoryItemStatus,
    pub draft_source: Option<HistoryDraftSource>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum HistoryWorkspaceScope {
    All,
    Untagged,
    Tag {
        #[serde(rename = "tagId")]
        tag_id: String,
    },
    Trash,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "lowercase")]
pub enum HistoryWorkspaceFilterType {
    All,
    Recording,
    Batch,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "lowercase")]
pub enum HistoryWorkspaceDateFilter {
    All,
    Today,
    Week,
    Month,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "snake_case")]
pub enum HistoryWorkspaceSortOrder {
    Newest,
    Oldest,
    DurationDesc,
    DurationAsc,
    TitleAsc,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct HistoryWorkspaceQueryRequest {
    pub scope: HistoryWorkspaceScope,
    pub query: String,
    pub filter_type: HistoryWorkspaceFilterType,
    pub date_filter: HistoryWorkspaceDateFilter,
    pub sort_order: HistoryWorkspaceSortOrder,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub limit: usize,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub offset: usize,
}

pub const MAX_WORKSPACE_QUERY_LIMIT: usize = 200;

#[derive(Clone, Debug, Serialize, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct HistoryWorkspaceSearchRange {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub start: usize,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub end: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct HistoryWorkspaceSearchSnippet {
    pub text: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub highlight_start: usize,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub highlight_end: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
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
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct HistoryWorkspaceSummary {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub total_items: usize,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub total_duration: f64,
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Number>)
    )]
    pub latest_timestamp: Option<u64>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub recording_count: usize,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub batch_count: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct HistoryWorkspaceItemCounts {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub untagged: usize,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub trash: usize,
    #[cfg_attr(
        feature = "specta",
        specta(type = BTreeMap<String, specta_typescript::Number>)
    )]
    pub by_tag_id: BTreeMap<String, usize>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct HistoryWorkspaceQueryResult {
    pub filtered_items: Vec<HistoryItemRecord>,
    pub search_match_by_item_id: BTreeMap<String, Option<HistoryWorkspaceItemSearchMatch>>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub filtered_item_count: usize,
    pub has_more: bool,
    pub summary: HistoryWorkspaceSummary,
    pub item_counts: HistoryWorkspaceItemCounts,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct LiveRecordingDraftResult {
    pub item: HistoryItemRecord,
    pub audio_absolute_path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct HistoryCreateLiveDraftRequest {
    pub id: Option<String>,
    pub audio_extension: String,
    #[serde(default)]
    pub tag_ids: Vec<String>,
    pub icon: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct HistorySaveRecordingRequest {
    pub segments: Vec<TranscriptSegment>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub duration: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tag_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_bytes: Option<Vec<u8>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_audio_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_extension: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct HistorySaveImportedFileRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub source_path: String,
    pub segments: Vec<TranscriptSegment>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub duration: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tag_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub converted_source_path: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct HistoryAudioCleanupRequest {
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Number>)
    )]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retention_days: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclude_history_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct HistoryAudioCleanupReport {
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub eligible_count: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub removed_count: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub removed_bytes: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub missing_marked_count: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub failed_count: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub skipped_active_count: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "snake_case")]
pub enum TranscriptSnapshotReason {
    Polish,
    Translate,
    Retranscribe,
    Restore,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSnapshotMetadata {
    pub id: String,
    pub history_id: String,
    pub reason: TranscriptSnapshotReason,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub created_at: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub segment_count: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSnapshotRecord {
    pub metadata: TranscriptSnapshotMetadata,
    pub segments: Vec<TranscriptSegment>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "lowercase")]
pub enum TranscriptDiffStatus {
    Unchanged,
    Modified,
    Added,
    Removed,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct TranscriptDiffRow {
    pub id: String,
    pub status: TranscriptDiffStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot_segment: Option<TranscriptSegment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_segment: Option<TranscriptSegment>,
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Number>)
    )]
    pub snapshot_index: Option<usize>,
    #[cfg_attr(
        feature = "specta",
        specta(type = Option<specta_typescript::Number>)
    )]
    pub current_index: Option<usize>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct TranscriptDiffResult {
    pub rows: Vec<TranscriptDiffRow>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub changed_count: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSummaryRecordPayload {
    pub template_id: String,
    pub content: String,
    pub generated_at: String,
    pub source_fingerprint: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct HistorySummaryPayload {
    pub active_template_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record: Option<TranscriptSummaryRecordPayload>,
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
