use crate::asr::TranscriptSegment;
use crate::speaker::SpeakerTag;
use std::fs;
use std::path::Path;

/// Supported transcript export formats for the CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Json,
    Txt,
    Srt,
    Vtt,
    Md,
}

impl ExportFormat {
    /// Parses a format string such as `json` or `srt`.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "json" => Ok(Self::Json),
            "txt" => Ok(Self::Txt),
            "srt" => Ok(Self::Srt),
            "vtt" => Ok(Self::Vtt),
            "md" => Ok(Self::Md),
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

/// Selects which transcript text fields are included in an export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportMode {
    Original,
    Translation,
    Bilingual,
}

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

#[derive(serde::Serialize)]
struct ExportJsonSegment<'a> {
    start: f64,
    end: f64,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    translation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    speaker: Option<&'a SpeakerTag>,
}

/// Serializes transcript segments into the requested export format.
pub fn export_segments(
    segments: &[TranscriptSegment],
    format: ExportFormat,
) -> Result<String, String> {
    export_segments_with_mode(segments, format, ExportMode::Original)
}

pub fn export_segments_with_mode(
    segments: &[TranscriptSegment],
    format: ExportFormat,
    mode: ExportMode,
) -> Result<String, String> {
    match format {
        ExportFormat::Json => export_json(segments, mode).map_err(|error| error.to_string()),
        ExportFormat::Txt => Ok(export_txt(segments, mode)),
        ExportFormat::Srt => Ok(export_srt(segments, mode)),
        ExportFormat::Vtt => Ok(export_vtt(segments, mode)),
        ExportFormat::Md => Ok(export_md(segments, mode)),
    }
}

#[tauri::command]
pub async fn export_transcript_file(
    segments: Vec<TranscriptSegment>,
    format: ExportFormat,
    mode: ExportMode,
    output_path: String,
) -> Result<ExportTranscriptFileResult, String> {
    export_transcript_file_inner(ExportTranscriptFileRequest {
        segments,
        format,
        mode,
        output_path,
    })
}

fn export_transcript_file_inner(
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

fn export_json(
    segments: &[TranscriptSegment],
    mode: ExportMode,
) -> Result<String, serde_json::Error> {
    let payload: Vec<ExportJsonSegment<'_>> = segments
        .iter()
        .filter(|segment| segment.is_final)
        .map(|segment| ExportJsonSegment {
            start: segment.start,
            end: segment.end,
            text: json_text(segment, mode),
            translation: json_translation(segment, mode),
            speaker: segment.speaker.as_ref(),
        })
        .filter(|segment| !segment.text.is_empty())
        .collect();
    serde_json::to_string_pretty(&payload)
}

fn export_txt(segments: &[TranscriptSegment], mode: ExportMode) -> String {
    segments
        .iter()
        .filter(|segment| segment.is_final)
        .map(|segment| segment_export_text_with_speaker(segment, mode, false, false))
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn export_md(segments: &[TranscriptSegment], mode: ExportMode) -> String {
    segments
        .iter()
        .filter(|segment| segment.is_final)
        .map(|segment| {
            let text = segment_export_text(segment, mode, false, false);
            prefix_md_speaker_label(segment, &text)
        })
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn export_srt(segments: &[TranscriptSegment], mode: ExportMode) -> String {
    segments
        .iter()
        .filter(|segment| segment.is_final)
        .filter_map(|segment| {
            let text = segment_export_text_with_speaker(segment, mode, true, true);
            if text.is_empty() {
                return None;
            }
            Some((segment.start, segment.end, text))
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

fn export_vtt(segments: &[TranscriptSegment], mode: ExportMode) -> String {
    let body = segments
        .iter()
        .filter(|segment| segment.is_final)
        .filter_map(|segment| {
            let text = segment_export_text_with_speaker(segment, mode, true, true);
            if text.is_empty() {
                return None;
            }
            Some(format!(
                "{} --> {}\n{}\n",
                format_timestamp(segment.start, "."),
                format_timestamp(segment.end, "."),
                text
            ))
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!("WEBVTT\n\n{body}")
}

fn json_text(segment: &TranscriptSegment, mode: ExportMode) -> String {
    match mode {
        ExportMode::Translation => segment
            .translation
            .as_deref()
            .unwrap_or("")
            .trim()
            .to_string(),
        ExportMode::Bilingual | ExportMode::Original => html_to_formatted_text(segment.text.trim()),
    }
}

fn json_translation(segment: &TranscriptSegment, mode: ExportMode) -> Option<String> {
    if mode != ExportMode::Bilingual {
        return None;
    }

    let translation = segment.translation.as_deref().unwrap_or("").trim();
    if translation.is_empty() {
        None
    } else {
        Some(html_to_formatted_text(translation))
    }
}

fn segment_export_text(
    segment: &TranscriptSegment,
    mode: ExportMode,
    is_html_formatted: bool,
    is_subtitle: bool,
) -> String {
    let original = if is_html_formatted {
        html_to_formatted_text(segment.text.trim())
    } else {
        html_to_plain_text(segment.text.trim())
    };
    let translation = if is_html_formatted {
        html_to_formatted_text(segment.translation.as_deref().unwrap_or("").trim())
    } else {
        html_to_plain_text(segment.translation.as_deref().unwrap_or("").trim())
    };

    match mode {
        ExportMode::Translation => translation,
        ExportMode::Bilingual if is_subtitle => format!("{translation}\n{original}"),
        ExportMode::Bilingual => format!("{original}\n{translation}"),
        ExportMode::Original => original,
    }
}

fn segment_export_text_with_speaker(
    segment: &TranscriptSegment,
    mode: ExportMode,
    is_html_formatted: bool,
    is_subtitle: bool,
) -> String {
    let text = segment_export_text(segment, mode, is_html_formatted, is_subtitle);
    prefix_speaker_label(segment, &text)
}

fn html_to_plain_text(html: &str) -> String {
    decode_html_entities(&strip_html_tags(&replace_structural_tags(html)))
}

fn html_to_formatted_text(html: &str) -> String {
    decode_html_entities(&strip_html_tags_except_formatting(
        &replace_structural_tags(html),
    ))
}

fn replace_structural_tags(html: &str) -> String {
    let mut output = String::with_capacity(html.len());
    let mut index = 0;
    let bytes = html.as_bytes();

    while index < bytes.len() {
        if bytes[index] == b'<' {
            if let Some(close_offset) = html[index..].find('>') {
                let end = index + close_offset + 1;
                let tag = &html[index + 1..end - 1];
                let tag_name = tag
                    .trim()
                    .trim_start_matches('/')
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .trim_end_matches('/')
                    .to_ascii_lowercase();

                match tag_name.as_str() {
                    "br" => output.push('\n'),
                    "div" | "p" if tag.trim_start().starts_with('/') => output.push('\n'),
                    "div" | "p" => {}
                    _ => output.push_str(&html[index..end]),
                }
                index = end;
                continue;
            }
        }

        if let Some(ch) = html[index..].chars().next() {
            output.push(ch);
            index += ch.len_utf8();
        } else {
            break;
        }
    }

    output
}

fn strip_html_tags(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut in_tag = false;

    for ch in text.chars() {
        match ch {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => output.push(ch),
            _ => {}
        }
    }

    output
}

fn strip_html_tags_except_formatting(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut index = 0;
    let bytes = text.as_bytes();

    while index < bytes.len() {
        if bytes[index] == b'<' {
            if let Some(close_offset) = text[index..].find('>') {
                let end = index + close_offset + 1;
                let tag_body = text[index + 1..end - 1].trim();
                let normalized = tag_body.trim_start_matches('/');
                let tag_name = normalized
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .trim_end_matches('/')
                    .to_ascii_lowercase();

                if matches!(tag_name.as_str(), "b" | "i" | "u") {
                    output.push_str(&text[index..end]);
                }

                index = end;
                continue;
            }
        }

        if let Some(ch) = text[index..].chars().next() {
            output.push(ch);
            index += ch.len_utf8();
        } else {
            break;
        }
    }

    output
}

fn decode_html_entities(text: &str) -> String {
    text.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
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

fn prefix_md_speaker_label(segment: &TranscriptSegment, text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    match segment.speaker.as_ref() {
        Some(speaker) if !speaker.label.trim().is_empty() => {
            format!("**{}**: {}", speaker.label.trim(), trimmed)
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
    fn parses_export_formats() {
        assert_eq!(ExportFormat::parse("json").unwrap(), ExportFormat::Json);
        assert_eq!(ExportFormat::parse("SRT").unwrap(), ExportFormat::Srt);
        assert_eq!(ExportFormat::parse("md").unwrap(), ExportFormat::Md);
    }

    #[test]
    fn exports_md_segments() {
        let output = export_segments(&sample_segments(), ExportFormat::Md).unwrap();
        assert_eq!(output, "**Alice**: Hello\n\nWorld");
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

    #[test]
    fn exports_bilingual_subtitles_translation_before_original() {
        let mut segments = sample_segments();
        segments[0].translation = Some("Bonjour".to_string());

        let output =
            export_segments_with_mode(&segments, ExportFormat::Srt, ExportMode::Bilingual).unwrap();

        assert!(output.contains("Alice: Bonjour\nHello"));
    }

    #[test]
    fn exports_txt_strips_tags_and_keeps_original_before_translation() {
        let mut segments = sample_segments();
        segments[0].text = "Hello <b>World</b><br>Again".to_string();
        segments[0].translation = Some("Bonjour <i>Monde</i>".to_string());

        let output =
            export_segments_with_mode(&segments, ExportFormat::Txt, ExportMode::Bilingual).unwrap();

        assert_eq!(output, "Alice: Hello World\nAgain\nBonjour Monde\n\nWorld");
    }

    #[test]
    fn export_transcript_file_writes_content_and_reports_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let output_path = dir.path().join("sample.vtt");
        let mut segments = sample_segments();
        segments[0].translation = Some("Bonjour".to_string());

        let result = export_transcript_file_inner(ExportTranscriptFileRequest {
            segments,
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
