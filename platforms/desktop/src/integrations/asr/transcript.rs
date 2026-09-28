use super::types::{TranscriptNormalizationOptions, TranscriptSegment};
#[cfg(test)]
use super::types::{TranscriptTimingLevel, TranscriptTimingSource};

fn new_transcript_segment_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub(crate) fn apply_timeline_normalization(
    segments: Vec<TranscriptSegment>,
    options: TranscriptNormalizationOptions,
) -> Vec<TranscriptSegment> {
    sona_core::transcription::transcript::apply_timeline_normalization_with_id_generator(
        segments,
        options,
        new_transcript_segment_id,
    )
}

#[cfg(test)]
mod tests {
    use super::super::types::TranscriptNormalizationOptions;
    use super::*;

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
    fn apply_timeline_normalization_marks_segment_level_splits_as_derived() {
        let results = apply_timeline_normalization(
            vec![sample_segment("Hello. World.", 0.0, 2.0)],
            TranscriptNormalizationOptions {
                enable_timeline: true,
            },
        );

        assert_eq!(results.len(), 2);
        assert_eq!(
            results[0].timing.as_ref().map(|timing| timing.level),
            Some(TranscriptTimingLevel::Segment)
        );
        assert_eq!(
            results[0].timing.as_ref().map(|timing| timing.source),
            Some(TranscriptTimingSource::Derived)
        );
        assert_eq!(
            results[1].timing.as_ref().map(|timing| timing.source),
            Some(TranscriptTimingSource::Derived)
        );
    }
}
