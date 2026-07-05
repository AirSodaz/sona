use sona_core::export::{ExportFormat, ExportMode, export_segments, export_segments_with_mode};
use sona_core::transcript::{SpeakerTag, TranscriptSegment};

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
