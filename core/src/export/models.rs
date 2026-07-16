use serde::{Deserialize, Serialize};

use crate::transcription::transcript::TranscriptSegment;

use super::{ExportFormat, ExportMode};

#[derive(Debug, Clone, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ExportTranscriptFileRequest {
    pub segments: Vec<TranscriptSegment>,
    pub format: ExportFormat,
    pub mode: ExportMode,
    pub output_path: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ExportTranscriptFileResult {
    pub output_path: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub bytes_written: u64,
}
