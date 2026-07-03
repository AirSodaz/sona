use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::integrations::asr::TranscriptSegment;

#[derive(Clone, Debug)]
pub struct PreparedBackupImportSnapshot {
    pub archive_path: String,
    pub extraction_dir: PathBuf,
}

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
}

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
    pub scoped_items: Vec<HistoryItemRecord>,
    pub scoped_item_ids: Vec<String>,
    pub search_match_by_item_id: BTreeMap<String, Option<HistoryWorkspaceItemSearchMatch>>,
    pub summary: HistoryWorkspaceSummary,
    pub item_counts: HistoryWorkspaceItemCounts,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LiveRecordingDraftResult {
    pub item: HistoryItemRecord,
    pub audio_absolute_path: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HistoryCreateLiveDraftRequest {
    pub id: Option<String>,
    pub audio_extension: String,
    pub project_id: Option<String>,
    pub icon: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HistorySaveRecordingRequest {
    pub segments: Value,
    pub duration: f64,
    pub project_id: Option<String>,
    pub audio_bytes: Option<Vec<u8>>,
    pub native_audio_path: Option<String>,
    pub audio_extension: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HistorySaveImportedFileRequest {
    pub id: Option<String>,
    pub source_path: String,
    pub segments: Value,
    pub duration: f64,
    pub project_id: Option<String>,
    pub converted_source_path: Option<String>,
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
    pub config: Value,
    pub projects: Vec<Value>,
    pub automation_rules: Vec<Value>,
    pub automation_processed_entries: Vec<Value>,
    pub analytics_content: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackupManifest {
    pub schema_version: u64,
    pub created_at: String,
    pub app_version: String,
    pub history_mode: String,
    pub scopes: BackupManifestScopes,
    pub counts: BackupManifestCounts,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackupManifestScopes {
    pub config: bool,
    pub workspace: bool,
    pub history: bool,
    pub automation: bool,
    pub analytics: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackupManifestCounts {
    pub projects: u64,
    pub history_items: u64,
    pub transcript_files: u64,
    pub summary_files: u64,
    pub automation_rules: u64,
    pub automation_processed_entries: u64,
    pub analytics_files: u64,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PreparedBackupImport {
    pub import_id: String,
    pub archive_path: String,
    pub manifest: BackupManifest,
    pub config: Value,
    pub projects: Vec<Value>,
    pub automation_rules: Vec<Value>,
    pub automation_processed_entries: Vec<Value>,
    pub analytics_content: String,
}

#[derive(Clone, Debug)]
pub struct HistoryBackupSnapshot {
    pub items: Vec<HistoryItemRecord>,
    pub transcript_files: Vec<(String, Value)>,
    pub summary_files: Vec<(String, Value)>,
    pub snapshot_files: Vec<(String, Value)>,
}
