use sona_core::export::{ExportFormat, ExportMode};
use sona_core::transcript::TranscriptSegment;

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportTranscriptFileRequest {
    pub segments: Vec<TranscriptSegment>,
    pub format: ExportFormat,
    pub mode: ExportMode,
    pub output_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportTranscriptFileResult {
    pub output_path: String,
    pub bytes_written: u64,
}

pub fn export_transcript_file(
    request: ExportTranscriptFileRequest,
) -> Result<ExportTranscriptFileResult, String> {
    let content = sona_core::export::export_segments_with_mode(
        &request.segments,
        request.format,
        request.mode,
    )?;
    let bytes_written = content.len() as u64;
    std::fs::write(&request.output_path, content).map_err(|error| {
        format!(
            "Failed to write transcript export to {}: {error}",
            std::path::Path::new(&request.output_path).display()
        )
    })?;

    Ok(ExportTranscriptFileResult {
        output_path: request.output_path,
        bytes_written,
    })
}
