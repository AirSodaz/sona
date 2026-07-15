use sona_core::export::{ExportError, ExportFormat, ExportMode, ExportTranscriptFileRequest};
use sona_core::transcription::transcript::TranscriptSegment;
use sona_export::export_transcript_file;

fn sample_segments() -> Vec<TranscriptSegment> {
    vec![TranscriptSegment {
        id: "1".to_string(),
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
    }]
}

#[test]
fn export_transcript_file_writes_core_content_and_reports_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let output_path = dir.path().join("sample.vtt");

    let result = export_transcript_file(ExportTranscriptFileRequest {
        segments: sample_segments(),
        format: ExportFormat::Vtt,
        mode: ExportMode::Bilingual,
        output_path: output_path.to_string_lossy().into_owned(),
    })
    .unwrap();

    let written = std::fs::read_to_string(&output_path).unwrap();
    assert_eq!(result.output_path, output_path.to_string_lossy());
    assert_eq!(result.bytes_written, written.len() as u64);
    assert!(written.starts_with("WEBVTT"));
    assert!(written.contains("Bonjour\nHello"));
}

#[test]
fn export_adapter_entrypoint_preserves_typed_repository_errors() {
    let dir = tempfile::tempdir().unwrap();
    let output_path = dir.path().join("missing").join("sample.vtt");

    let error = export_transcript_file(ExportTranscriptFileRequest {
        segments: sample_segments(),
        format: ExportFormat::Vtt,
        mode: ExportMode::Original,
        output_path: output_path.to_string_lossy().into_owned(),
    })
    .unwrap_err();

    assert!(
        matches!(error, ExportError::Repository { ref reason } if reason.contains("Failed to write transcript export")),
        "unexpected adapter error: {error:?}"
    );
    assert!(!output_path.exists());
}
