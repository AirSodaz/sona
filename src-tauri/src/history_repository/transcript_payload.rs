use serde_json::{from_value, to_value, Value};

use super::fs_utils::ensure_json_array_value;
use crate::sherpa::{ensure_transcript_segment_timing, TranscriptSegment};

pub(super) struct NormalizedHistoryTranscript {
    pub(super) segments: Value,
    pub(super) preview_text: String,
    pub(super) search_content: String,
}

pub(super) fn normalize_history_transcript_segments(
    segments: Value,
) -> Result<NormalizedHistoryTranscript, String> {
    let segments = ensure_json_array_value(segments, "History transcript segments")?;
    let mut parsed_segments: Vec<TranscriptSegment> = from_value(segments).map_err(|error| {
        format!("History transcript segments must match transcript schema: {error}")
    })?;

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
    let segments = to_value(&parsed_segments).map_err(|error| error.to_string())?;

    Ok(NormalizedHistoryTranscript {
        segments,
        preview_text,
        search_content,
    })
}

fn preview_text_from_search_content(search_content: &str, has_segments: bool) -> String {
    let mut preview_text = search_content.chars().take(100).collect::<String>();
    if has_segments {
        preview_text.push_str("...");
    }
    preview_text
}
