use sona_core::export::{ExportFormat, ExportMode};
use sona_export::{
    ExportTranscriptFileRequest, ExportTranscriptFileResult,
    export_transcript_file as adapter_export_transcript_file,
};

#[tauri::command]
pub async fn export_transcript_file(
    segments: Vec<sona_core::transcription::transcript::TranscriptSegment>,
    format: ExportFormat,
    mode: ExportMode,
    output_path: String,
) -> Result<ExportTranscriptFileResult, String> {
    adapter_export_transcript_file(ExportTranscriptFileRequest {
        segments,
        format,
        mode,
        output_path,
    })
}
