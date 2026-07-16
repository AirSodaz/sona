use std::sync::{Arc, Mutex};

use sona_core::export::{
    ExportError, ExportFormat, ExportMode, ExportService, ExportTranscriptFileRequest,
    ExportTranscriptFileResult, TranscriptExportRepository, export_segments,
    export_segments_with_mode,
};
use sona_core::transcription::transcript::{SpeakerTag, TranscriptSegment};

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
            translation: None,
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
fn export_format_parses_and_infers_without_adapter_dependencies() {
    assert_eq!(ExportFormat::parse("json").unwrap(), ExportFormat::Json);
    assert_eq!(ExportFormat::parse("SRT").unwrap(), ExportFormat::Srt);
    assert_eq!(ExportFormat::parse("md").unwrap(), ExportFormat::Md);

    let format = ExportFormat::from_output_path(std::path::Path::new("sample.vtt")).unwrap();
    assert_eq!(format, ExportFormat::Vtt);
    assert_eq!(ExportMode::parse("original").unwrap(), ExportMode::Original);
    assert_eq!(
        ExportMode::parse("BILINGUAL").unwrap(),
        ExportMode::Bilingual
    );
}

#[test]
fn exports_plain_and_subtitle_formats_from_core_segments() {
    let segments = sample_segments();

    assert_eq!(
        export_segments(&segments, ExportFormat::Txt).unwrap(),
        "Alice: Hello\n\nWorld"
    );
    assert_eq!(
        export_segments(&segments, ExportFormat::Md).unwrap(),
        "**Alice**: Hello\n\nWorld"
    );

    let srt = export_segments(&segments, ExportFormat::Srt).unwrap();
    assert!(srt.contains("1\n00:00:00,000 --> 00:00:01,250\nAlice: Hello"));
    assert!(srt.contains("2\n00:00:01,250 --> 00:00:02,500\nWorld"));

    let vtt = export_segments(&segments, ExportFormat::Vtt).unwrap();
    assert!(vtt.starts_with("WEBVTT"));
    assert!(vtt.contains("00:00:00.000 --> 00:00:01.250"));
}

#[test]
fn exports_json_segments_with_speaker_shape() {
    let output = export_segments(&sample_segments(), ExportFormat::Json).unwrap();

    assert!(output.contains("\"start\": 0.0"));
    assert!(output.contains("\"text\": \"Hello\""));
    assert!(output.contains("\"label\": \"Alice\""));
}

#[test]
fn exports_bilingual_text_and_subtitles_with_existing_ordering() {
    let mut segments = sample_segments();
    segments[0].translation = Some("Bonjour".to_string());

    let srt =
        export_segments_with_mode(&segments, ExportFormat::Srt, ExportMode::Bilingual).unwrap();
    assert!(srt.contains("Alice: Bonjour\nHello"));

    segments[0].text = "Hello <b>World</b><br>Again".to_string();
    segments[0].translation = Some("Bonjour <i>Monde</i>".to_string());
    let txt =
        export_segments_with_mode(&segments, ExportFormat::Txt, ExportMode::Bilingual).unwrap();

    assert_eq!(txt, "Alice: Hello World\nAgain\nBonjour Monde\n\nWorld");
}

struct RecordingExportRepository {
    writes: Arc<Mutex<Vec<(String, String)>>>,
}

impl TranscriptExportRepository for RecordingExportRepository {
    fn write_export(&self, output_path: &str, content: &str) -> Result<(), ExportError> {
        self.writes
            .lock()
            .unwrap()
            .push((output_path.to_string(), content.to_string()));
        Ok(())
    }
}

#[test]
fn export_service_renders_through_core_and_persists_through_the_port() {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let repository = RecordingExportRepository {
        writes: Arc::clone(&writes),
    };
    let service = ExportService::new(repository);

    let result = service
        .export_transcript_file(ExportTranscriptFileRequest {
            segments: sample_segments(),
            format: ExportFormat::Vtt,
            mode: ExportMode::Original,
            output_path: "exports/sample.vtt".to_string(),
        })
        .unwrap();

    let writes = writes.lock().unwrap();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].0, "exports/sample.vtt");
    assert!(writes[0].1.starts_with("WEBVTT"));
    assert_eq!(result.output_path, "exports/sample.vtt");
    assert_eq!(result.bytes_written, writes[0].1.len() as u64);
}

struct FailingExportRepository;

impl TranscriptExportRepository for FailingExportRepository {
    fn write_export(&self, _output_path: &str, _content: &str) -> Result<(), ExportError> {
        Err(ExportError::Repository {
            reason: "disk full".to_string(),
        })
    }
}

#[test]
fn export_service_preserves_repository_failures() {
    let error = ExportService::new(FailingExportRepository)
        .export_transcript_file(ExportTranscriptFileRequest {
            segments: sample_segments(),
            format: ExportFormat::Txt,
            mode: ExportMode::Original,
            output_path: "exports/sample.txt".to_string(),
        })
        .unwrap_err();

    assert_eq!(
        error,
        ExportError::Repository {
            reason: "disk full".to_string(),
        }
    );
}

#[test]
fn export_file_contracts_round_trip_camel_case_json() {
    let request = ExportTranscriptFileRequest {
        segments: sample_segments(),
        format: ExportFormat::Vtt,
        mode: ExportMode::Bilingual,
        output_path: "C:/exports/transcript.vtt".to_string(),
    };

    let request_json = serde_json::to_value(&request).unwrap();
    assert_eq!(request_json["format"], "vtt");
    assert_eq!(request_json["mode"], "bilingual");
    assert_eq!(request_json["outputPath"], "C:/exports/transcript.vtt");

    let decoded_request: ExportTranscriptFileRequest =
        serde_json::from_value(request_json).unwrap();
    assert_eq!(decoded_request.segments, request.segments);
    assert_eq!(decoded_request.format, request.format);
    assert_eq!(decoded_request.mode, request.mode);
    assert_eq!(decoded_request.output_path, request.output_path);

    let result: ExportTranscriptFileResult = serde_json::from_value(serde_json::json!({
        "outputPath": "C:/exports/transcript.vtt",
        "bytesWritten": 42
    }))
    .unwrap();
    assert_eq!(result.output_path, "C:/exports/transcript.vtt");
    assert_eq!(result.bytes_written, 42);
}
