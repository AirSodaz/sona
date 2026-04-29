use crate::sherpa::TranscriptSegment;
use crate::speaker::SpeakerTag;

/// Supported transcript export formats for the CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Json,
    Txt,
    Srt,
    Vtt,
}

impl ExportFormat {
    /// Parses a format string such as `json` or `srt`.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "json" => Ok(Self::Json),
            "txt" => Ok(Self::Txt),
            "srt" => Ok(Self::Srt),
            "vtt" => Ok(Self::Vtt),
            other => Err(format!("Unsupported export format: {other}")),
        }
    }

    /// Infers an export format from a file extension.
    pub fn from_output_path(path: &std::path::Path) -> Result<Self, String> {
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .ok_or_else(|| {
                format!(
                    "Unable to infer export format from path: {}",
                    path.display()
                )
            })?;
        Self::parse(extension)
    }
}

#[derive(serde::Serialize)]
struct ExportJsonSegment<'a> {
    start: f64,
    end: f64,
    text: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    speaker: Option<&'a SpeakerTag>,
}

/// Serializes transcript segments into the requested export format.
pub fn export_segments(
    segments: &[TranscriptSegment],
    format: ExportFormat,
) -> Result<String, String> {
    match format {
        ExportFormat::Json => export_json(segments).map_err(|error| error.to_string()),
        ExportFormat::Txt => Ok(export_txt(segments)),
        ExportFormat::Srt => Ok(export_srt(segments)),
        ExportFormat::Vtt => Ok(export_vtt(segments)),
    }
}

fn export_json(segments: &[TranscriptSegment]) -> Result<String, serde_json::Error> {
    let payload: Vec<ExportJsonSegment<'_>> = segments
        .iter()
        .filter(|segment| segment.is_final)
        .map(|segment| ExportJsonSegment {
            start: segment.start,
            end: segment.end,
            text: segment.text.trim(),
            speaker: segment.speaker.as_ref(),
        })
        .filter(|segment| !segment.text.is_empty())
        .collect();
    serde_json::to_string_pretty(&payload)
}

fn export_txt(segments: &[TranscriptSegment]) -> String {
    segments
        .iter()
        .filter(|segment| segment.is_final)
        .map(|segment| prefix_speaker_label(segment, segment.text.trim()))
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn export_srt(segments: &[TranscriptSegment]) -> String {
    segments
        .iter()
        .filter(|segment| segment.is_final)
        .filter_map(|segment| {
            let text = segment.text.trim();
            if text.is_empty() {
                return None;
            }
            Some((segment.start, segment.end, prefix_speaker_label(segment, text)))
        })
        .enumerate()
        .map(|(index, (start, end, text))| {
            format!(
                "{}\n{} --> {}\n{}\n",
                index + 1,
                format_timestamp(start, ","),
                format_timestamp(end, ","),
                text
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn export_vtt(segments: &[TranscriptSegment]) -> String {
    let body = segments
        .iter()
        .filter(|segment| segment.is_final)
        .filter_map(|segment| {
            let text = segment.text.trim();
            if text.is_empty() {
                return None;
            }
            Some(format!(
                "{} --> {}\n{}\n",
                format_timestamp(segment.start, "."),
                format_timestamp(segment.end, "."),
                prefix_speaker_label(segment, text)
            ))
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!("WEBVTT\n\n{body}")
}

fn format_timestamp(seconds: f64, separator: &str) -> String {
    let hours = (seconds / 3600.0).floor() as u64;
    let minutes = ((seconds % 3600.0) / 60.0).floor() as u64;
    let whole_seconds = (seconds % 60.0).floor() as u64;
    let millis = ((seconds.fract()) * 1000.0).floor() as u64;

    format!("{hours:02}:{minutes:02}:{whole_seconds:02}{separator}{millis:03}")
}

fn prefix_speaker_label(segment: &TranscriptSegment, text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    match segment.speaker.as_ref() {
        Some(speaker) if !speaker.label.trim().is_empty() => {
            format!("{}: {}", speaker.label.trim(), trimmed)
        }
        _ => trimmed.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            },
        ]
    }

    #[test]
    fn parses_export_formats() {
        assert_eq!(ExportFormat::parse("json").unwrap(), ExportFormat::Json);
        assert_eq!(ExportFormat::parse("SRT").unwrap(), ExportFormat::Srt);
        assert!(ExportFormat::parse("md").is_err());
    }

    #[test]
    fn infers_export_format_from_output_path() {
        let format = ExportFormat::from_output_path(std::path::Path::new("sample.vtt")).unwrap();
        assert_eq!(format, ExportFormat::Vtt);
    }

    #[test]
    fn exports_json_segments() {
        let output = export_segments(&sample_segments(), ExportFormat::Json).unwrap();
        assert!(output.contains("\"start\": 0.0"));
        assert!(output.contains("\"text\": \"Hello\""));
        assert!(output.contains("\"label\": \"Alice\""));
    }

    #[test]
    fn exports_txt_segments() {
        let output = export_segments(&sample_segments(), ExportFormat::Txt).unwrap();
        assert_eq!(output, "Alice: Hello\n\nWorld");
    }

    #[test]
    fn exports_srt_segments() {
        let output = export_segments(&sample_segments(), ExportFormat::Srt).unwrap();
        assert!(output.contains("00:00:00,000 --> 00:00:01,250"));
        assert!(output.contains("1\n00:00:00,000 --> 00:00:01,250\nAlice: Hello"));
        assert!(output.contains("2\n00:00:01,250 --> 00:00:02,500\nWorld"));
    }

    #[test]
    fn exports_vtt_segments() {
        let output = export_segments(&sample_segments(), ExportFormat::Vtt).unwrap();
        assert!(output.starts_with("WEBVTT"));
        assert!(output.contains("00:00:00.000 --> 00:00:01.250"));
    }
}
