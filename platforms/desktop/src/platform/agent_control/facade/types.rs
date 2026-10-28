use serde::{Deserialize, Serialize};
use sona_core::ports::asr::AsrTranscriptionRequest;
use sona_core::transcription::transcript::TranscriptSegment;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientStateResult {
    pub online: bool,
    pub is_recording: bool,
    #[serde(default)]
    pub is_paused: bool,
    pub active_project_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StartRecordingRequest {
    #[serde(default, alias = "projectId", alias = "project_id")]
    pub project_id: Option<String>,
    #[serde(default, alias = "deviceName", alias = "device_name")]
    pub device_name: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default, alias = "asrRequest", alias = "asr_request")]
    pub asr_request: Option<AsrTranscriptionRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRecordingResult {
    pub history_id: String,
    pub started_at: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StopRecordingRequest {
    #[serde(default)]
    pub discard: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StopRecordingResult {
    pub history_id: String,
    pub duration_seconds: f64,
    pub segment_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevicesResult {
    pub microphones: Vec<crate::integrations::audio::AudioDevice>,
    pub system_devices: Vec<crate::integrations::audio::AudioDevice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevertTranscriptResult {
    pub success: bool,
    pub snapshot_id: String,
    pub segment_count: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateHistoryMetaRequest {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default, alias = "projectId", alias = "project_id")]
    pub project_id: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueryHistoryRequest {
    #[serde(default)]
    pub query: String,
    #[serde(default, alias = "projectId", alias = "project_id")]
    pub project_id: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItemSummary {
    pub id: String,
    pub title: String,
    pub preview_text: String,
    pub timestamp: u64,
    pub duration: f64,
    pub project_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ReadTranscriptRequest {
    #[serde(alias = "historyId", alias = "history_id")]
    pub history_id: String,
    #[serde(default, alias = "startSeconds", alias = "start_seconds")]
    pub start_seconds: Option<f64>,
    #[serde(default, alias = "endSeconds", alias = "end_seconds")]
    pub end_seconds: Option<f64>,
    #[serde(default)]
    pub offset: Option<usize>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default, alias = "sinceSegmentId", alias = "since_segment_id")]
    pub since_segment_id: Option<String>,
}

impl From<String> for ReadTranscriptRequest {
    fn from(history_id: String) -> Self {
        Self {
            history_id,
            ..Default::default()
        }
    }
}

impl From<&str> for ReadTranscriptRequest {
    fn from(history_id: &str) -> Self {
        Self {
            history_id: history_id.to_string(),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadTranscriptResult {
    pub history_id: String,
    #[serde(default = "default_completed_status")]
    pub task_status: String,
    pub error_message: Option<String>,
    pub segments: Vec<TranscriptSegment>,
    pub text: String,
    #[serde(default, alias = "translation_text")]
    pub translation_text: Option<String>,
    #[serde(default)]
    pub total_segments_in_history: usize,
    #[serde(default)]
    pub returned_segments: usize,
    #[serde(default)]
    pub has_more: bool,
}

fn default_completed_status() -> String {
    "completed".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelaxedTranscriptSegmentInput {
    pub id: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub start: Option<f64>,
    #[serde(default)]
    pub end: Option<f64>,
    #[serde(default)]
    pub is_final: Option<bool>,
    #[serde(default)]
    pub translation: Option<String>,
}

impl From<TranscriptSegment> for RelaxedTranscriptSegmentInput {
    fn from(s: TranscriptSegment) -> Self {
        Self {
            id: Some(s.id),
            text: Some(s.text),
            start: Some(s.start),
            end: Some(s.end),
            is_final: Some(s.is_final),
            translation: s.translation,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditTranscriptRequest {
    #[serde(alias = "historyId", alias = "history_id")]
    pub history_id: String,
    pub segments: Vec<RelaxedTranscriptSegmentInput>,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditTranscriptResult {
    pub success: bool,
    pub snapshot_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SegmentPatchInput {
    pub id: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub start: Option<f64>,
    #[serde(default)]
    pub end: Option<f64>,
    #[serde(default)]
    pub translation: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchSegmentsRequest {
    #[serde(alias = "historyId", alias = "history_id")]
    pub history_id: String,
    pub segments: Vec<SegmentPatchInput>,
    #[serde(default, alias = "removeIds", alias = "remove_ids")]
    pub remove_ids: Option<Vec<String>>,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchSegmentsResult {
    pub success: bool,
    pub snapshot_id: String,
    pub updated_count: usize,
    pub total_segments: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SegmentTranslationInput {
    pub id: String,
    #[serde(default)]
    pub translation: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTranslationsRequest {
    #[serde(alias = "historyId", alias = "history_id")]
    pub history_id: String,
    pub translations: Vec<SegmentTranslationInput>,
    #[serde(default, alias = "clearAll", alias = "clear_all")]
    pub clear_all: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTranslationsResult {
    pub success: bool,
    pub snapshot_id: String,
    pub updated_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteHistoryRequest {
    #[serde(alias = "historyId", alias = "history_id")]
    pub history_id: String,
    #[serde(default)]
    pub permanent: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SetActiveProjectRequest {
    #[serde(default, alias = "projectId", alias = "project_id")]
    pub project_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GetSettingsRequest {
    pub key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSettingRequest {
    pub key: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TranscribeFileRequest {
    #[serde(alias = "filePath", alias = "file_path", alias = "input")]
    pub file_path: String,
    #[serde(default, alias = "projectId", alias = "project_id")]
    pub project_id: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default, alias = "saveToPath", alias = "save_to_path")]
    pub save_to_path: Option<String>,
    #[serde(
        default,
        alias = "instanceId",
        alias = "instance_id",
        alias = "taskId",
        alias = "task_id"
    )]
    pub instance_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscribeFileResult {
    pub history_id: String,
    pub task_id: String,
    pub status: String,
    pub is_stream: bool,
    pub message: String,
    #[serde(default)]
    pub duration_seconds: f64,
    #[serde(default)]
    pub segment_count: usize,
    #[serde(default)]
    pub text: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExportTranscriptRequest {
    #[serde(alias = "historyId", alias = "history_id")]
    pub history_id: String,
    pub format: String,
    #[serde(alias = "outputPath", alias = "output_path")]
    pub output_path: String,
    #[serde(default)]
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportTranscriptResult {
    pub success: bool,
    pub output_path: String,
    pub format: String,
    pub segment_count: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SaveSummaryRequest {
    #[serde(alias = "historyId", alias = "history_id")]
    pub history_id: String,
    pub content: String,
    #[serde(default, alias = "templateId", alias = "template_id")]
    pub template_id: Option<String>,
    #[serde(default)]
    pub thought: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DownloadPresetModelRequest {
    #[serde(alias = "modelId", alias = "model_id")]
    pub model_id: String,
    #[serde(default)]
    pub mirror: Option<String>,
    #[serde(default, alias = "downloadId", alias = "download_id")]
    pub download_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CreateProjectRequest {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct UpdateProjectRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct QueryTrashRequest {
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub offset: Option<usize>,
}
