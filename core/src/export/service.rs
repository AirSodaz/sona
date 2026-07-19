use super::{
    ExportError, ExportTranscriptFileRequest, ExportTranscriptFileResult,
    TranscriptExportRepository, export_segments_with_mode,
};

pub struct ExportService<R> {
    repository: R,
}

impl<R> ExportService<R>
where
    R: TranscriptExportRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub fn export_transcript_file(
        &self,
        request: ExportTranscriptFileRequest,
    ) -> Result<ExportTranscriptFileResult, ExportError> {
        let content = export_segments_with_mode(&request.segments, request.format, request.mode)?;
        self.repository
            .write_export(&request.output_path, &content)?;

        Ok(ExportTranscriptFileResult {
            output_path: request.output_path,
            bytes_written: content.len() as u64,
        })
    }
}
