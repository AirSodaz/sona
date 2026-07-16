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
    let request = ExportTranscriptFileRequest {
        segments,
        format,
        mode,
        output_path,
    };
    sona_ts_bind::validate_export_transcript_request_for_typescript(&request)?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        export_transcript_file_with_fs(request).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())??;
    sona_ts_bind::validate_export_transcript_result_for_typescript(&result)?;
    Ok(result)
}
