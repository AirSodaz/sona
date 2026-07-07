use crate::integrations::asr::TranscriptSegment;
use std::fs;
use std::path::Path;

pub use sona_core::export::{ExportFormat, ExportMode, export_segments, export_segments_with_mode};

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportTranscriptFileRequest {
    pub segments: Vec<TranscriptSegment>,
    pub format: ExportFormat,
    pub mode: ExportMode,
    pub output_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportTranscriptFileResult {
    pub output_path: String,
    pub bytes_written: u64,
}

pub(crate) fn export_transcript_file_inner(
    request: ExportTranscriptFileRequest,
) -> Result<ExportTranscriptFileResult, String> {
    let content = export_segments_with_mode(&request.segments, request.format, request.mode)?;
    let bytes_written = content.len() as u64;
    fs::write(&request.output_path, content).map_err(|error| {
        format!(
            "Failed to write transcript export to {}: {error}",
            Path::new(&request.output_path).display()
        )
    })?;

    Ok(ExportTranscriptFileResult {
        output_path: request.output_path,
        bytes_written,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sona_core::transcript::SpeakerTag;

    fn sample_segments() -> Vec<TranscriptSegment> {
        vec![
            TranscriptSegment {
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
                speaker: Some(SpeakerTag {
                    id: "speaker-1".to_string(),
                    label: "Alice".to_string(),
                    kind: "identified".to_string(),
                    score: Some(0.88),
                }),
                speaker_attribution: None,
            },
            TranscriptSegment {
                id: "2".to_string(),
                text: "World".to_string(),
                start: 1.25,
                end: 2.5,
                is_final: true,
                timing: None,
                tokens: None,
                timestamps: None,
                durations: None,
                translation: None,
                speaker: None,
                speaker_attribution: None,
            },
        ]
    }

    #[test]
    fn export_transcript_file_writes_core_content_and_reports_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let output_path = dir.path().join("sample.vtt");

        let result = export_transcript_file_inner(ExportTranscriptFileRequest {
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
        assert!(written.contains("Alice: Bonjour\nHello"));
    }
}
