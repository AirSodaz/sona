use serde::{Deserialize, Serialize};

use crate::transcription::transcript::TranscriptSegment;

use super::{ExportFormat, ExportMode};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportTranscriptFileRequest {
    pub segments: Vec<TranscriptSegment>,
    pub format: ExportFormat,
    pub mode: ExportMode,
    pub output_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportTranscriptFileResult {
    pub output_path: String,
    pub bytes_written: u64,
}
