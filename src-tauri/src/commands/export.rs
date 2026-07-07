use crate::integrations::asr::TranscriptSegment;
use crate::platform::export_files::{
    ExportFormat, ExportMode, ExportTranscriptFileRequest, ExportTranscriptFileResult,
    export_transcript_file_inner,
};

#[tauri::command]
pub async fn export_transcript_file(
    segments: Vec<TranscriptSegment>,
    format: ExportFormat,
    mode: ExportMode,
    output_path: String,
) -> Result<ExportTranscriptFileResult, String> {
    export_transcript_file_inner(ExportTranscriptFileRequest {
        segments,
        format,
        mode,
        output_path,
    })
}
