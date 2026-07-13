use super::ExportError;

pub trait TranscriptExportRepository: Send + Sync {
    fn write_export(&self, output_path: &str, content: &str) -> Result<(), ExportError>;
}
