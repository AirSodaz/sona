use sona_core::export::{
    ExportError, ExportTranscriptFileRequest, ExportTranscriptFileResult,
    TranscriptExportRepository, export_segments_with_order,
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
        let content = export_segments_with_order(
            &request.segments,
            request.format,
            request.mode,
            request.bilingual_order.as_deref(),
        )?;
        self.repository
            .write_export(&request.output_path, &content)?;

        Ok(ExportTranscriptFileResult {
            output_path: request.output_path,
            bytes_written: content.len() as u64,
        })
    }
}
