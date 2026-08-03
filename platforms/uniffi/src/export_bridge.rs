use crate::mapper::export_request_from_ffi;
use crate::{
    FfiExportTranscriptFileRequestV1, FfiExportTranscriptFileResultV1, SonaCoreBindingError,
    SonaCoreBindingResult,
};
use sona_core::export::ExportTranscriptFileRequest;
use sona_export::export_transcript_file;

pub(crate) async fn export_transcript_file_json(
    input_json: String,
) -> SonaCoreBindingResult<String> {
    tokio::task::spawn_blocking(move || build_export_transcript_file_json(input_json))
        .await
        .map_err(export_error)?
}

pub(crate) async fn export_transcript_file_v1(
    request: FfiExportTranscriptFileRequestV1,
) -> SonaCoreBindingResult<FfiExportTranscriptFileResultV1> {
    // Validate the typed request before handing work to the blocking pool so a
    // malformed segment never reaches the filesystem.
    let request = export_request_from_ffi(request).map_err(export_error)?;
    tokio::task::spawn_blocking(move || export_transcript_file(request))
        .await
        .map_err(export_error)?
        .map(Into::into)
        .map_err(export_error)
}

fn build_export_transcript_file_json(input_json: String) -> SonaCoreBindingResult<String> {
    let request: ExportTranscriptFileRequest =
        serde_json::from_str(&input_json).map_err(export_error)?;
    let result = export_transcript_file(request).map_err(export_error)?;
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
    use super::{export_transcript_file_json, export_transcript_file_v1};
    use crate::{
        FfiExportFormatV1, FfiExportModeV1, FfiExportTranscriptFileRequestV1, FfiTranscriptSegment,
        SonaCoreBindingError,
    };
    use serde_json::{Value, json};
    use sona_core::export::ExportTranscriptFileResult;
    use std::fs;

    fn typed_request(output_path: &std::path::Path) -> FfiExportTranscriptFileRequestV1 {
        FfiExportTranscriptFileRequestV1 {
            segments: vec![FfiTranscriptSegment {
                id: "segment-1".to_string(),
                text: "Hello".to_string(),
                start: 0.0,
                end: 1.25,
                is_final: true,
                timing: None,
                tokens: None,
                timestamps: None,
                durations: None,
                translation: Some("Bonjour".to_string()),
                speaker: None,
                speaker_attribution: None,
            }],
            format: FfiExportFormatV1::Vtt,
            mode: FfiExportModeV1::Bilingual,
            output_path: output_path.to_string_lossy().into_owned(),
        }
    }

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
        let result: ExportTranscriptFileResult = serde_json::from_str(&output).unwrap();
        let canonical: Value = serde_json::from_str(&output).unwrap();
        let content = fs::read_to_string(&output_path).unwrap();

        assert_eq!(serde_json::to_string(&canonical).unwrap(), output);
        assert_eq!(result.output_path, output_path.to_string_lossy().as_ref());
        assert_eq!(result.bytes_written, content.len() as u64);
        assert!(content.starts_with("WEBVTT"));
        assert!(content.contains("Bonjour\nHello"));
    }

    #[tokio::test]
    async fn typed_export_writes_the_same_file_as_the_json_surface() {
        let dir = tempfile::tempdir().unwrap();
        let typed_path = dir.path().join("typed.vtt");
        let json_path = dir.path().join("json.vtt");

        let typed = export_transcript_file_v1(typed_request(&typed_path))
            .await
            .unwrap();
        let json_output = export_transcript_file_json(request_json(&json_path))
            .await
            .unwrap();
        let json: ExportTranscriptFileResult = serde_json::from_str(&json_output).unwrap();

        assert_eq!(typed.bytes_written, json.bytes_written);
        assert_eq!(
            fs::read_to_string(&typed_path).unwrap(),
            fs::read_to_string(&json_path).unwrap()
        );
        assert_eq!(typed.output_path, typed_path.to_string_lossy().as_ref());
    }

    #[tokio::test]
    async fn typed_export_reports_export_errors_without_writing_files() {
        let dir = tempfile::tempdir().unwrap();
        let output_path = dir.path().join("missing").join("transcript.vtt");

        let error = export_transcript_file_v1(typed_request(&output_path))
            .await
            .unwrap_err();

        assert!(matches!(error, SonaCoreBindingError::Export { .. }));
        assert!(!output_path.exists());
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
