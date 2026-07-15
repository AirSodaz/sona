use sona_core::export::{ExportError, ExportService, TranscriptExportRepository};

pub use sona_core::export::{ExportTranscriptFileRequest, ExportTranscriptFileResult};

#[derive(Debug, Clone, Copy, Default)]
pub struct FsTranscriptExportRepository;

impl TranscriptExportRepository for FsTranscriptExportRepository {
    fn write_export(&self, output_path: &str, content: &str) -> Result<(), ExportError> {
        std::fs::write(output_path, content).map_err(|error| ExportError::Repository {
            reason: format!(
                "Failed to write transcript export to {}: {error}",
                std::path::Path::new(output_path).display()
            ),
        })
    }
}

pub fn export_transcript_file(
    request: ExportTranscriptFileRequest,
) -> Result<ExportTranscriptFileResult, ExportError> {
    ExportService::new(FsTranscriptExportRepository).export_transcript_file(request)
}
