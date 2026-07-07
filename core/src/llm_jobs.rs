use std::collections::HashMap;

use crate::llm_tasks::{LlmSegmentInput, PolishedSegment, SummarySegmentInput, TranslatedSegment};
use crate::transcript::TranscriptSegment;

pub fn normalized_job_history_id(job_history_id: Option<&str>) -> Option<String> {
    job_history_id
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "current")
        .map(str::to_string)
}

pub fn segment_inputs_from_transcript(segments: &[TranscriptSegment]) -> Vec<LlmSegmentInput> {
    segments
        .iter()
        .map(|segment| LlmSegmentInput {
            id: segment.id.clone(),
            text: segment.text.clone(),
        })
        .collect()
}

pub fn summary_inputs_from_transcript(segments: &[TranscriptSegment]) -> Vec<SummarySegmentInput> {
    segments
        .iter()
        .map(|segment| {
            let text = segment
                .speaker
                .as_ref()
                .map(|speaker| format!("{}: {}", speaker.label, segment.text))
                .unwrap_or_else(|| segment.text.clone());
            SummarySegmentInput {
                id: segment.id.clone(),
                text,
                start: segment.start as f32,
                end: segment.end as f32,
                is_final: segment.is_final,
            }
        })
        .collect()
}

pub fn merge_translated_items_into_segments(
    mut segments: Vec<TranscriptSegment>,
    items: &[TranslatedSegment],
) -> Vec<TranscriptSegment> {
    let items_by_id: HashMap<&str, &TranslatedSegment> =
        items.iter().map(|item| (item.id.as_str(), item)).collect();
    for segment in &mut segments {
        if let Some(item) = items_by_id.get(segment.id.as_str()) {
            segment.translation = Some(item.translation.clone());
        }
    }
    segments
}

pub fn merge_polished_items_into_segments(
    mut segments: Vec<TranscriptSegment>,
    items: &[PolishedSegment],
) -> Vec<TranscriptSegment> {
    let items_by_id: HashMap<&str, &PolishedSegment> =
        items.iter().map(|item| (item.id.as_str(), item)).collect();
    for segment in &mut segments {
        if let Some(item) = items_by_id.get(segment.id.as_str()) {
            segment.text = item.text.clone();
        }
    }
    segments
}

pub fn compute_summary_source_fingerprint(segments: &[TranscriptSegment]) -> String {
    segments
        .iter()
        .map(|segment| {
            let (speaker_id, speaker_label, speaker_kind, speaker_score) = segment
                .speaker
                .as_ref()
                .map_or(("", "", "", String::new()), |speaker| {
                    (
                        speaker.id.as_str(),
                        speaker.label.as_str(),
                        speaker.kind.as_str(),
                        speaker
                            .score
                            .map(|score| score.to_string())
                            .unwrap_or_default(),
                    )
                });
            format!(
                "{}:{}:{}:{}:{}:{}:{}:{}:{}",
                segment.id,
                segment.text,
                segment.start,
                segment.end,
                segment.is_final,
                speaker_id,
                speaker_label,
                speaker_kind,
                speaker_score
            )
        })
        .collect::<Vec<_>>()
        .join("|")
}
