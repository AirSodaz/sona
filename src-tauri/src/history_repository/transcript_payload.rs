use serde_json::{Map, Value, from_value};

use super::fs_utils::ensure_json_array_value;
use crate::asr::{TranscriptSegment, ensure_transcript_segment_timing};

pub(super) struct NormalizedHistoryTranscript {
    pub(super) segments: Vec<TranscriptSegment>,
    pub(super) preview_text: String,
    pub(super) search_content: String,
}

pub(super) fn normalize_history_transcript_segments(
    segments: Value,
) -> Result<NormalizedHistoryTranscript, String> {
    let segments = ensure_json_array_value(segments, "History transcript segments")?;
    let parsed_segments = segments
        .as_array()
        .ok_or_else(|| "History transcript segments must be an array.".to_string())?;
    let mut parsed_segments: Vec<TranscriptSegment> = parsed_segments
        .iter()
        .enumerate()
        .map(|(index, segment)| parse_history_transcript_segment(segment, index))
        .collect::<Result<Vec<_>, _>>()?;

    for segment in &mut parsed_segments {
        ensure_transcript_segment_timing(segment);
    }

    let search_content = parsed_segments
        .iter()
        .map(|segment| segment.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let preview_text =
        preview_text_from_search_content(&search_content, !parsed_segments.is_empty());
    Ok(NormalizedHistoryTranscript {
        segments: parsed_segments,
        preview_text,
        search_content,
    })
}

fn parse_history_transcript_segment(
    segment: &Value,
    index: usize,
) -> Result<TranscriptSegment, String> {
    match from_value::<TranscriptSegment>(segment.clone()) {
        Ok(segment) => Ok(segment),
        Err(strict_error) => {
            let Some(source) = segment.as_object() else {
                return Err(format!(
                    "History transcript segment at index {index} must be an object: {strict_error}"
                ));
            };

            let mut normalized = source.clone();
            ensure_string_field(&mut normalized, "id", &format!("segment-{index}"));
            ensure_string_field(&mut normalized, "text", "");
            ensure_number_field(&mut normalized, "start", 0.0);
            let start = normalized
                .get("start")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            ensure_number_field(&mut normalized, "end", start);
            if normalized.get("isFinal").and_then(Value::as_bool).is_none() {
                normalized.insert("isFinal".to_string(), Value::Bool(true));
            }

            from_value(Value::Object(normalized)).map_err(|error| {
                format!("History transcript segment at index {index} must match transcript schema: {error}")
            })
        }
    }
}

fn ensure_string_field(source: &mut Map<String, Value>, key: &str, fallback: &str) {
    if source.get(key).and_then(Value::as_str).is_some() {
        return;
    }

    source.insert(key.to_string(), Value::String(fallback.to_string()));
}

fn ensure_number_field(source: &mut Map<String, Value>, key: &str, fallback: f64) {
    if source.get(key).and_then(Value::as_f64).is_some() {
        return;
    }

    source.insert(key.to_string(), Value::from(fallback));
}

fn preview_text_from_search_content(search_content: &str, has_segments: bool) -> String {
    let mut preview_text = search_content.chars().take(100).collect::<String>();
    if has_segments {
        preview_text.push_str("...");
    }
    preview_text
}
