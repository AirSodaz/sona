use serde::{Deserialize, Serialize};

use crate::transcription::transcript::{TranscriptSegment, ensure_transcript_segment_timing};

use super::mutation_repository::HistoryMutationError;

#[cfg(feature = "specta")]
use specta::Type;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TranscriptEditOperation {
    UpdateText {
        segment_id: String,
        text: String,
    },
    UpdateTranslation {
        segment_id: String,
        translation: Option<String>,
    },
    Delete {
        segment_id: String,
    },
    MergeNext {
        segment_id: String,
    },
    Split {
        segment_id: String,
        new_segment_id: String,
        left_text: String,
        right_text: String,
        left_translation: Option<String>,
        right_translation: Option<String>,
    },
}

pub fn apply_transcript_edit(
    mut segments: Vec<TranscriptSegment>,
    operation: TranscriptEditOperation,
) -> Result<Vec<TranscriptSegment>, HistoryMutationError> {
    match operation {
        TranscriptEditOperation::UpdateText { segment_id, text } => {
            let segment = find_segment_mut(&mut segments, &segment_id)?;
            segment.text = required_text("Transcript text", text)?;
            reset_timing(segment);
        }
        TranscriptEditOperation::UpdateTranslation {
            segment_id,
            translation,
        } => {
            let segment = find_segment_mut(&mut segments, &segment_id)?;
            segment.translation = optional_text(translation);
        }
        TranscriptEditOperation::Delete { segment_id } => {
            let index = find_segment_index(&segments, &segment_id)?;
            segments.remove(index);
        }
        TranscriptEditOperation::MergeNext { segment_id } => {
            let index = find_segment_index(&segments, &segment_id)?;
            if index + 1 >= segments.len() {
                return Err(invalid(
                    "The final transcript segment cannot be merged forward.",
                ));
            }
            let first = &segments[index];
            let second = &segments[index + 1];
            if !first.is_final || !second.is_final {
                return Err(invalid("Only final transcript segments can be merged."));
            }
            if speaker_identity(first) != speaker_identity(second) {
                return Err(invalid(
                    "Transcript segments with different speakers cannot be merged.",
                ));
            }
            let mut merged = first.clone();
            merged.text = join_text(&first.text, &second.text);
            merged.translation =
                join_optional_text(first.translation.as_deref(), second.translation.as_deref());
            merged.start = first.start.min(second.start);
            merged.end = first.end.max(second.end);
            reset_timing(&mut merged);
            segments.splice(index..=index + 1, [merged]);
        }
        TranscriptEditOperation::Split {
            segment_id,
            new_segment_id,
            left_text,
            right_text,
            left_translation,
            right_translation,
        } => {
            validate_id("New segment ID", &new_segment_id)?;
            if segments.iter().any(|segment| segment.id == new_segment_id) {
                return Err(invalid("New transcript segment ID already exists."));
            }
            let index = find_segment_index(&segments, &segment_id)?;
            let original = segments[index].clone();
            if !original.is_final {
                return Err(invalid("Only final transcript segments can be split."));
            }
            let left_text = required_text("Left transcript text", left_text)?;
            let right_text = required_text("Right transcript text", right_text)?;
            let left_translation = optional_text(left_translation);
            let right_translation = optional_text(right_translation);
            if original
                .translation
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
                && (left_translation.is_none() || right_translation.is_none())
            {
                return Err(invalid(
                    "Both translated halves are required when splitting a translated segment.",
                ));
            }

            let boundary = split_boundary(&original, &left_text, &right_text);
            let mut left = original.clone();
            left.text = left_text;
            left.translation = left_translation;
            left.end = boundary;
            reset_timing(&mut left);

            let mut right = original;
            right.id = new_segment_id;
            right.text = right_text;
            right.translation = right_translation;
            right.start = boundary;
            reset_timing(&mut right);
            segments.splice(index..=index, [left, right]);
        }
    }
    Ok(segments)
}

fn find_segment_index(
    segments: &[TranscriptSegment],
    segment_id: &str,
) -> Result<usize, HistoryMutationError> {
    validate_id("Transcript segment ID", segment_id)?;
    segments
        .iter()
        .position(|segment| segment.id == segment_id)
        .ok_or_else(|| invalid("Transcript segment was not found."))
}

fn find_segment_mut<'a>(
    segments: &'a mut [TranscriptSegment],
    segment_id: &str,
) -> Result<&'a mut TranscriptSegment, HistoryMutationError> {
    let index = find_segment_index(segments, segment_id)?;
    Ok(&mut segments[index])
}

fn split_boundary(segment: &TranscriptSegment, left_text: &str, right_text: &str) -> f64 {
    let left_chars = left_text.chars().count();
    let total_chars = left_chars + right_text.chars().count();
    let ratio = if total_chars == 0 {
        0.5
    } else {
        left_chars as f64 / total_chars as f64
    };
    let target = segment.start + (segment.end - segment.start).max(0.0) * ratio;
    segment
        .timing
        .as_ref()
        .filter(|timing| timing.units.len() > 1)
        .and_then(|timing| {
            timing
                .units
                .iter()
                .take(timing.units.len() - 1)
                .min_by(|left, right| {
                    (left.end - target)
                        .abs()
                        .total_cmp(&(right.end - target).abs())
                })
                .map(|unit| unit.end)
        })
        .unwrap_or(target)
        .clamp(segment.start, segment.end.max(segment.start))
}

fn reset_timing(segment: &mut TranscriptSegment) {
    segment.timing = None;
    segment.tokens = None;
    segment.timestamps = None;
    segment.durations = None;
    ensure_transcript_segment_timing(segment);
}

fn speaker_identity(segment: &TranscriptSegment) -> Option<(&str, Option<&str>)> {
    segment.speaker.as_ref().map(|speaker| {
        (
            speaker.id.as_str(),
            segment
                .speaker_attribution
                .as_ref()
                .map(|attribution| attribution.group_id.as_str()),
        )
    })
}

fn join_text(left: &str, right: &str) -> String {
    format!("{} {}", left.trim(), right.trim())
        .trim()
        .to_string()
}

fn join_optional_text(left: Option<&str>, right: Option<&str>) -> Option<String> {
    let joined = join_text(left.unwrap_or_default(), right.unwrap_or_default());
    (!joined.is_empty()).then_some(joined)
}

fn required_text(label: &str, value: String) -> Result<String, HistoryMutationError> {
    let value = value.trim().to_string();
    if value.is_empty() {
        Err(invalid(&format!("{label} must not be empty.")))
    } else {
        Ok(value)
    }
}

fn optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    })
}

fn validate_id(label: &str, value: &str) -> Result<(), HistoryMutationError> {
    if value.trim().is_empty() {
        Err(invalid(&format!("{label} must not be empty.")))
    } else {
        Ok(())
    }
}

fn invalid(message: &str) -> HistoryMutationError {
    HistoryMutationError::InvalidRequest(message.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcription::transcript::{
        SpeakerTag, TranscriptTiming, TranscriptTimingLevel, TranscriptTimingSource,
        TranscriptTimingUnit,
    };

    fn segment(id: &str, text: &str, start: f64, end: f64) -> TranscriptSegment {
        TranscriptSegment {
            id: id.to_string(),
            text: text.to_string(),
            start,
            end,
            is_final: true,
            timing: None,
            tokens: None,
            timestamps: None,
            durations: None,
            translation: None,
            speaker: None,
            speaker_attribution: None,
        }
    }

    #[test]
    fn updating_original_text_clears_legacy_timing_and_rebuilds_derived_timing() {
        let mut original = segment("one", "before", 1.0, 3.0);
        original.tokens = Some(vec!["before".to_string()]);
        original.timestamps = Some(vec![1.0]);
        original.durations = Some(vec![2.0]);

        let changed = apply_transcript_edit(
            vec![original],
            TranscriptEditOperation::UpdateText {
                segment_id: "one".to_string(),
                text: "after".to_string(),
            },
        )
        .unwrap();

        assert_eq!(changed[0].text, "after");
        assert!(changed[0].tokens.is_none());
        assert!(changed[0].timestamps.is_none());
        assert!(changed[0].durations.is_none());
        assert_eq!(
            changed[0].timing.as_ref().unwrap().source,
            TranscriptTimingSource::Derived
        );
    }

    #[test]
    fn cjk_split_without_token_timing_uses_character_ratio() {
        let changed = apply_transcript_edit(
            vec![segment("one", "你好世界", 0.0, 8.0)],
            TranscriptEditOperation::Split {
                segment_id: "one".to_string(),
                new_segment_id: "two".to_string(),
                left_text: "你好".to_string(),
                right_text: "世界".to_string(),
                left_translation: None,
                right_translation: None,
            },
        )
        .unwrap();

        assert_eq!(changed.len(), 2);
        assert_eq!(changed[0].end, 4.0);
        assert_eq!(changed[1].start, 4.0);
    }

    #[test]
    fn split_prefers_nearest_token_timing_boundary() {
        let mut original = segment("one", "one two three", 0.0, 9.0);
        original.timing = Some(TranscriptTiming {
            level: TranscriptTimingLevel::Token,
            source: TranscriptTimingSource::Model,
            units: vec![
                TranscriptTimingUnit {
                    text: "one".into(),
                    start: 0.0,
                    end: 1.0,
                },
                TranscriptTimingUnit {
                    text: "two".into(),
                    start: 1.0,
                    end: 7.0,
                },
                TranscriptTimingUnit {
                    text: "three".into(),
                    start: 7.0,
                    end: 9.0,
                },
            ],
        });

        let changed = apply_transcript_edit(
            vec![original],
            TranscriptEditOperation::Split {
                segment_id: "one".into(),
                new_segment_id: "two".into(),
                left_text: "one two".into(),
                right_text: "three".into(),
                left_translation: None,
                right_translation: None,
            },
        )
        .unwrap();

        assert_eq!(changed[0].end, 7.0);
        assert_eq!(changed[1].start, 7.0);
    }

    #[test]
    fn translated_split_requires_both_translated_halves() {
        let mut original = segment("one", "hello world", 0.0, 2.0);
        original.translation = Some("你好世界".into());

        let result = apply_transcript_edit(
            vec![original],
            TranscriptEditOperation::Split {
                segment_id: "one".into(),
                new_segment_id: "two".into(),
                left_text: "hello".into(),
                right_text: "world".into(),
                left_translation: Some("你好".into()),
                right_translation: None,
            },
        );

        assert!(matches!(
            result,
            Err(HistoryMutationError::InvalidRequest(_))
        ));
    }

    #[test]
    fn merge_rejects_different_speakers_and_delete_allows_empty_transcript() {
        let mut first = segment("one", "first", 0.0, 1.0);
        first.speaker = Some(SpeakerTag {
            id: "a".into(),
            label: "A".into(),
            kind: "known".into(),
            score: None,
        });
        let mut second = segment("two", "second", 1.0, 2.0);
        second.speaker = Some(SpeakerTag {
            id: "b".into(),
            label: "B".into(),
            kind: "known".into(),
            score: None,
        });

        assert!(
            apply_transcript_edit(
                vec![first, second],
                TranscriptEditOperation::MergeNext {
                    segment_id: "one".into()
                },
            )
            .is_err()
        );

        let empty = apply_transcript_edit(
            vec![segment("last", "only", 0.0, 1.0)],
            TranscriptEditOperation::Delete {
                segment_id: "last".into(),
            },
        )
        .unwrap();
        assert!(empty.is_empty());
    }
}
