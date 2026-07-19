use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::transcription::transcript::{SpeakerTag, TranscriptSegment};

mod error;
mod models;
mod ports;
mod service;

pub use error::ExportError;
pub use models::{ExportTranscriptFileRequest, ExportTranscriptFileResult};
pub use ports::TranscriptExportRepository;
pub use service::ExportService;

/// Supported transcript export formats for every Sona frontend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
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
    pub fn parse(value: &str) -> Result<Self, ExportError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "json" => Ok(Self::Json),
            "txt" => Ok(Self::Txt),
            "srt" => Ok(Self::Srt),
            "vtt" => Ok(Self::Vtt),
            "md" => Ok(Self::Md),
            other => Err(ExportError::InvalidFormat {
                value: other.to_string(),
            }),
        }
    }

    /// Infers an export format from a file extension.
    pub fn from_output_path(path: &Path) -> Result<Self, ExportError> {
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .ok_or_else(|| ExportError::MissingFormatExtension {
                path: path.to_path_buf(),
            })?;
        Self::parse(extension)
    }
}

/// Selects which transcript text fields are included in an export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "lowercase")]
pub enum ExportMode {
    Original,
    Translation,
    Bilingual,
}

impl ExportMode {
    pub fn parse(value: &str) -> Result<Self, ExportError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "original" => Ok(Self::Original),
            "translation" => Ok(Self::Translation),
            "bilingual" => Ok(Self::Bilingual),
            other => Err(ExportError::InvalidMode {
                value: other.to_string(),
            }),
        }
    }
}

#[derive(Serialize)]
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
) -> Result<String, ExportError> {
    export_segments_with_mode(segments, format, ExportMode::Original)
}

pub fn export_segments_with_mode(
    segments: &[TranscriptSegment],
    format: ExportFormat,
    mode: ExportMode,
) -> Result<String, ExportError> {
    match format {
        ExportFormat::Json => export_json(segments, mode).map_err(|error| ExportError::Render {
            reason: error.to_string(),
        }),
        ExportFormat::Txt => Ok(export_txt(segments, mode)),
        ExportFormat::Srt => Ok(export_srt(segments, mode)),
        ExportFormat::Vtt => Ok(export_vtt(segments, mode)),
        ExportFormat::Md => Ok(export_md(segments, mode)),
    }
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
        if bytes[index] == b'<'
            && let Some(close_offset) = html[index..].find('>')
        {
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
        if bytes[index] == b'<'
            && let Some(close_offset) = text[index..].find('>')
        {
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
