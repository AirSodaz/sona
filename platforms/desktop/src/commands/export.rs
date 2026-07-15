use sona_core::export::{
    ExportFormat, ExportMode, ExportTranscriptFileRequest, ExportTranscriptFileResult,
};
use sona_export::export_transcript_file as export_transcript_file_with_fs;

#[tauri::command]
pub async fn export_transcript_file(
    segments: Vec<sona_core::transcription::transcript::TranscriptSegment>,
    format: ExportFormat,
    mode: ExportMode,
    output_path: String,
) -> Result<ExportTranscriptFileResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        export_transcript_file_with_fs(ExportTranscriptFileRequest {
            segments,
            format,
            mode,
            output_path,
        })
        .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}
