use sona_application::export::ExportService;
use sona_core::export::{ExportError, TranscriptExportRepository};

pub use sona_core::export::{ExportTranscriptFileRequest, ExportTranscriptFileResult};

#[derive(Debug, Clone, Copy, Default)]
pub struct FsTranscriptExportRepository;

impl TranscriptExportRepository for FsTranscriptExportRepository {
    fn write_export(&self, output_path: &str, content: &str) -> Result<(), ExportError> {
        std::fs::write(output_path, content).map_err(|error| {
            log::error!(
                "[Export] Failed to write transcript export (size_bytes={}): {error}",
                content.len()
            );
            ExportError::Repository {
                reason: format!(
                    "Failed to write transcript export to {}: {error}",
                    std::path::Path::new(output_path).display()
                ),
            }
        })
    }
}

pub fn export_transcript_file(
    request: ExportTranscriptFileRequest,
) -> Result<ExportTranscriptFileResult, ExportError> {
    log::info!(
        "[Export] Exporting transcript format={:?} segments={}",
        request.format,
        request.segments.len()
    );
    let result = ExportService::new(FsTranscriptExportRepository).export_transcript_file(request);
    if let Ok(ref res) = result {
        log::info!(
            "[Export] Transcript export completed successfully: bytes_written={}",
            res.bytes_written
        );
    }
    result
}
