use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptTimingLevel {
    Token,
    Segment,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptTimingSource {
    Model,
    Derived,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptTimingUnit {
    pub text: String,
    pub start: f64,
    pub end: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptTiming {
    pub level: TranscriptTimingLevel,
    pub source: TranscriptTimingSource,
    pub units: Vec<TranscriptTimingUnit>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerTag {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub score: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerCandidate {
    pub profile_id: String,
    pub profile_name: String,
    pub score: f32,
    pub rank: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerAttribution {
    pub group_id: String,
    pub anonymous_label: String,
    pub state: String,
    pub source: String,
    pub confidence: String,
    pub candidates: Vec<SpeakerCandidate>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSegment {
    pub id: String,
    pub text: String,
    pub start: f64,
    pub end: f64,
    pub is_final: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timing: Option<TranscriptTiming>,
    // Legacy raw fields are still written for compatibility with older
    // persisted transcript records and upgrade paths.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamps: Option<Vec<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub durations: Option<Vec<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker: Option<SpeakerTag>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker_attribution: Option<SpeakerAttribution>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptUpdate {
    pub remove_ids: Vec<String>,
    pub upsert_segments: Vec<TranscriptSegment>,
}

pub fn ensure_transcript_segment_timing(segment: &mut TranscriptSegment) {
    segment.start = segment.start.max(0.0);
    segment.end = segment.end.max(segment.start);

    let timing = segment
        .timing
        .clone()
        .map(|timing| TranscriptTiming {
            level: timing.level,
            source: timing.source,
            units: normalize_timing_units(timing.units, segment.start, segment.end),
        })
        .filter(|timing| !timing.units.is_empty())
        .or_else(|| build_timing_from_legacy(segment))
        .unwrap_or_else(|| build_segment_level_timing(segment, TranscriptTimingSource::Derived));

    segment.timing = Some(timing);
}

fn normalize_timing_units(
    units: Vec<TranscriptTimingUnit>,
    segment_start: f64,
    segment_end: f64,
) -> Vec<TranscriptTimingUnit> {
    let safe_start = segment_start.max(0.0);
    let safe_end = segment_end.max(safe_start);

    let unit_count = units.len();

    units
        .into_iter()
        .enumerate()
        .filter_map(|(index, unit)| {
            if unit.text.is_empty() {
                return None;
            }

            let start = unit.start.max(safe_start).min(safe_end);
            let fallback_end = if index + 1 == unit_count {
                safe_end
            } else {
                start
            };
            let end = unit.end.max(fallback_end).min(safe_end).max(start);

            Some(TranscriptTimingUnit {
                text: unit.text,
                start,
                end,
            })
        })
        .collect()
}

fn build_segment_level_timing(
    segment: &TranscriptSegment,
    source: TranscriptTimingSource,
) -> TranscriptTiming {
    TranscriptTiming {
        level: TranscriptTimingLevel::Segment,
        source,
        units: vec![TranscriptTimingUnit {
            text: segment.text.clone(),
            start: segment.start,
            end: segment.end,
        }],
    }
}

fn build_token_windows(
    timestamps: &[f32],
    durations: Option<&[f32]>,
    segment_end: f64,
) -> Vec<(f64, f64)> {
    timestamps
        .iter()
        .enumerate()
        .map(|(index, timestamp)| {
            let start = *timestamp as f64;
            let explicit_end = durations
                .and_then(|values| values.get(index))
                .map(|value| start + (*value as f64).max(0.0));
            let next_start = timestamps.get(index + 1).map(|value| *value as f64);
            let end = explicit_end
                .or(next_start)
                .unwrap_or(segment_end)
                .max(start);
            (start, end)
        })
        .collect()
}

fn build_aligned_timing_units(
    text: &str,
    tokens: &[String],
    timestamps: &[f32],
    durations: Option<&[f32]>,
    segment_end: f64,
) -> Option<Vec<TranscriptTimingUnit>> {
    if tokens.is_empty() || tokens.len() != timestamps.len() {
        return None;
    }

    let windows = build_token_windows(timestamps, durations, segment_end);
    let aligned_units = crate::text_alignment::align_text_units_to_tokens(text, tokens)?;

    Some(
        aligned_units
            .into_iter()
            .map(|unit| {
                let (start, end) = windows
                    .get(unit.token_index)
                    .copied()
                    .unwrap_or((segment_end, segment_end));
                TranscriptTimingUnit {
                    text: unit.text,
                    start,
                    end,
                }
            })
            .collect(),
    )
}

fn build_timing_from_legacy(segment: &TranscriptSegment) -> Option<TranscriptTiming> {
    let tokens = segment.tokens.as_ref()?;
    let timestamps = segment.timestamps.as_ref()?;
    if tokens.is_empty() || tokens.len() != timestamps.len() {
        return None;
    }

    let durations = segment
        .durations
        .as_ref()
        .filter(|values| values.len() == tokens.len())
        .map(|values| values.as_slice());
    let units =
        build_aligned_timing_units(&segment.text, tokens, timestamps, durations, segment.end)?;
    let units = normalize_timing_units(units, segment.start, segment.end);
    if units.is_empty() {
        return None;
    }

    Some(TranscriptTiming {
        level: TranscriptTimingLevel::Token,
        source: TranscriptTimingSource::Model,
        units,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcript_segment_keeps_camel_case_transport_shape() {
        let segment = TranscriptSegment {
            id: "seg-1".to_string(),
            text: "hello".to_string(),
            start: 0.0,
            end: 1.0,
            is_final: true,
            timing: None,
            tokens: None,
            timestamps: None,
            durations: None,
            translation: None,
            speaker: None,
            speaker_attribution: None,
        };

        let value = serde_json::to_value(segment).unwrap();

        assert_eq!(value["isFinal"], true);
        assert!(value.get("is_final").is_none());
    }

    fn sample_segment(text: &str, start: f64, end: f64) -> TranscriptSegment {
        TranscriptSegment {
            id: "segment-1".to_string(),
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
    fn ensure_transcript_segment_timing_builds_token_level_units_from_legacy_fields() {
        let mut segment = TranscriptSegment {
            text: "你好世界".to_string(),
            tokens: Some(vec![
                "你".to_string(),
                "好".to_string(),
                "世".to_string(),
                "界".to_string(),
            ]),
            timestamps: Some(vec![0.0, 0.25, 0.5, 0.75]),
            durations: Some(vec![0.25, 0.25, 0.25, 0.25]),
            ..sample_segment("你好世界", 0.0, 1.0)
        };

        ensure_transcript_segment_timing(&mut segment);

        let timing = segment.timing.expect("timing should exist");
        assert_eq!(timing.level, TranscriptTimingLevel::Token);
        assert_eq!(timing.source, TranscriptTimingSource::Model);
        assert_eq!(timing.units.len(), 4);
        assert_eq!(timing.units[0].text, "你");
        assert_eq!(timing.units[0].start, 0.0);
        assert_eq!(timing.units[3].text, "界");
        assert_eq!(timing.units[3].end, 1.0);
    }

    #[test]
    fn ensure_transcript_segment_timing_falls_back_to_segment_level_without_token_timestamps() {
        let mut segment = TranscriptSegment {
            tokens: Some(vec!["Hello".to_string(), "world".to_string()]),
            ..sample_segment("Hello world", 1.0, 3.0)
        };

        ensure_transcript_segment_timing(&mut segment);

        let timing = segment.timing.expect("timing should exist");
        assert_eq!(timing.level, TranscriptTimingLevel::Segment);
        assert_eq!(timing.source, TranscriptTimingSource::Derived);
        assert_eq!(
            timing.units,
            vec![TranscriptTimingUnit {
                text: "Hello world".to_string(),
                start: 1.0,
                end: 3.0,
            }]
        );
    }
}
