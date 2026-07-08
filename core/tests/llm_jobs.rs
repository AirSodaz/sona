use sona_core::llm::jobs::{
    compute_summary_source_fingerprint, merge_polished_items_into_segments,
    merge_translated_items_into_segments, normalized_job_history_id,
    segment_inputs_from_transcript, summary_inputs_from_transcript,
};
use sona_core::llm::tasks::{PolishedSegment, TranslatedSegment};
use sona_core::transcription::transcript::{SpeakerTag, TranscriptSegment};

fn sample_segment(id: &str, text: &str) -> TranscriptSegment {
    TranscriptSegment {
        id: id.to_string(),
        text: text.to_string(),
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
    }
}

#[test]
fn normalizes_current_or_empty_history_id_to_none() {
    assert_eq!(normalized_job_history_id(None), None);
    assert_eq!(normalized_job_history_id(Some("  ")), None);
    assert_eq!(normalized_job_history_id(Some(" current ")), None);
    assert_eq!(
        normalized_job_history_id(Some(" history-1 ")),
        Some("history-1".to_string())
    );
}

#[test]
fn builds_segment_and_summary_inputs_from_transcript_segments() {
    let mut segment = sample_segment("1", "Hello");
    segment.speaker = Some(SpeakerTag {
        id: "speaker-a".to_string(),
        label: "Alice".to_string(),
        kind: "identified".to_string(),
        score: Some(0.91),
    });

    let segment_inputs = segment_inputs_from_transcript(&[segment.clone()]);
    let summary_inputs = summary_inputs_from_transcript(&[segment]);

    assert_eq!(segment_inputs[0].id, "1");
    assert_eq!(segment_inputs[0].text, "Hello");
    assert_eq!(summary_inputs[0].text, "Alice: Hello");
    assert_eq!(summary_inputs[0].start, 0.0);
    assert_eq!(summary_inputs[0].end, 1.0);
    assert!(summary_inputs[0].is_final);
}

#[test]
fn merges_llm_segment_results_without_losing_existing_fields() {
    let mut first = sample_segment("1", "hello");
    first.speaker = Some(SpeakerTag {
        id: "speaker-a".to_string(),
        label: "Alice".to_string(),
        kind: "identified".to_string(),
        score: Some(0.91),
    });
    let mut second = sample_segment("2", "world");
    second.translation = Some("old".to_string());

    let translated = merge_translated_items_into_segments(
        vec![first.clone(), second.clone()],
        &[TranslatedSegment {
            id: "1".to_string(),
            translation: "ni hao".to_string(),
        }],
    );
    let polished = merge_polished_items_into_segments(
        vec![second],
        &[PolishedSegment {
            id: "2".to_string(),
            text: "World.".to_string(),
        }],
    );

    assert_eq!(translated[0].translation.as_deref(), Some("ni hao"));
    assert_eq!(translated[0].speaker, first.speaker);
    assert_eq!(translated[1].translation.as_deref(), Some("old"));
    assert_eq!(polished[0].text, "World.");
    assert_eq!(polished[0].translation.as_deref(), Some("old"));
}

#[test]
fn summary_fingerprint_matches_frontend_contract() {
    let mut segment = sample_segment("1", "Hello");
    segment.speaker = Some(SpeakerTag {
        id: "speaker-a".to_string(),
        label: "Alice".to_string(),
        kind: "identified".to_string(),
        score: Some(0.91),
    });

    assert_eq!(
        compute_summary_source_fingerprint(&[segment]),
        "1:Hello:0:1:true:speaker-a:Alice:identified:0.91"
    );
}
