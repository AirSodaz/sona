use sona_core::export::{
    ExportFormat, ExportMode, ExportTranscriptFileRequest, ExportTranscriptFileResult,
    export_transcript_file as core_export_transcript_file,
};

#[tauri::command]
pub async fn export_transcript_file(
    segments: Vec<sona_core::transcript::TranscriptSegment>,
    format: ExportFormat,
    mode: ExportMode,
    output_path: String,
) -> Result<ExportTranscriptFileResult, String> {
    core_export_transcript_file(ExportTranscriptFileRequest {
        segments,
        format,
        mode,
        output_path,
    })
}
