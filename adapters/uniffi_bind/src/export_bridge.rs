use crate::{SonaCoreBindingError, SonaCoreBindingResult};
use sona_core::export::{ExportService, ExportTranscriptFileRequest};
use sona_export::FsTranscriptExportRepository;

pub(crate) async fn export_transcript_file_json(
    input_json: String,
) -> SonaCoreBindingResult<String> {
    tokio::task::spawn_blocking(move || build_export_transcript_file_json(input_json))
        .await
        .map_err(export_error)?
}

fn build_export_transcript_file_json(input_json: String) -> SonaCoreBindingResult<String> {
    let request: ExportTranscriptFileRequest =
        serde_json::from_str(&input_json).map_err(export_error)?;
    let result = ExportService::new(FsTranscriptExportRepository)
        .export_transcript_file(request)
        .map_err(export_error)?;
    let canonical = serde_json::to_value(result).map_err(export_error)?;
    serde_json::to_string(&canonical).map_err(export_error)
}

fn export_error(reason: impl ToString) -> SonaCoreBindingError {
    SonaCoreBindingError::Export {
        reason: reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::export_transcript_file_json;
    use crate::SonaCoreBindingError;
    use serde_json::{Value, json};
    use std::fs;

    fn request_json(output_path: &std::path::Path) -> String {
        serde_json::to_string(&json!({
            "segments": [{
                "id": "segment-1",
                "text": "Hello",
                "start": 0.0,
                "end": 1.25,
                "isFinal": true,
                "translation": "Bonjour"
            }],
            "format": "vtt",
            "mode": "bilingual",
            "outputPath": output_path
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn export_writes_file_and_returns_canonical_json() {
        let dir = tempfile::tempdir().unwrap();
        let output_path = dir.path().join("移动端导出.vtt");

        let output = export_transcript_file_json(request_json(&output_path))
            .await
            .unwrap();
        let result: Value = serde_json::from_str(&output).unwrap();
        let content = fs::read_to_string(&output_path).unwrap();

        assert_eq!(serde_json::to_string(&result).unwrap(), output);
        assert_eq!(result["outputPath"], output_path.to_string_lossy().as_ref());
        assert_eq!(result["bytesWritten"], content.len() as u64);
        assert!(content.starts_with("WEBVTT"));
        assert!(content.contains("Bonjour\nHello"));
    }

    #[tokio::test]
    async fn invalid_json_uses_export_error_without_writing_files() {
        let error = export_transcript_file_json("{".to_string())
            .await
            .unwrap_err();

        assert!(matches!(error, SonaCoreBindingError::Export { .. }));
    }

    #[tokio::test]
    async fn repository_failure_uses_export_error() {
        let dir = tempfile::tempdir().unwrap();
        let output_path = dir.path().join("missing").join("transcript.txt");

        let error = export_transcript_file_json(request_json(&output_path))
            .await
            .unwrap_err();

        assert!(matches!(error, SonaCoreBindingError::Export { .. }));
        assert!(!output_path.exists());
    }
}
