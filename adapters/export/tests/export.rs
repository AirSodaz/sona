use sona_core::export::{ExportFormat, ExportMode};
use sona_core::transcript::TranscriptSegment;

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

    let result = sona_export::export_transcript_file(sona_export::ExportTranscriptFileRequest {
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
