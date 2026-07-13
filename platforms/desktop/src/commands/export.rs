use sona_core::export::{
    ExportFormat, ExportMode, ExportService, ExportTranscriptFileRequest,
    ExportTranscriptFileResult,
};
use sona_export::FsTranscriptExportRepository;

#[tauri::command]
pub async fn export_transcript_file(
    segments: Vec<sona_core::transcription::transcript::TranscriptSegment>,
    format: ExportFormat,
    mode: ExportMode,
    output_path: String,
) -> Result<ExportTranscriptFileResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        ExportService::new(FsTranscriptExportRepository)
            .export_transcript_file(ExportTranscriptFileRequest {
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
