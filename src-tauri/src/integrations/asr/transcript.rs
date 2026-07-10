use super::recognizer_output_event;
use super::types::{TranscriptNormalizationOptions, TranscriptSegment, TranscriptUpdate};
#[cfg(test)]
use super::types::{TranscriptTimingLevel, TranscriptTimingSource};
use log::info;
use sona_local_asr::punctuation::Punctuation;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) use sona_core::transcription::transcript::{
    normalize_recognizer_text, select_final_transcript_text, synthesize_durations,
};

fn new_transcript_segment_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub(crate) fn diagnostics_instance_label(instance_id: &str) -> Option<&'static str> {
    match instance_id {
        "record" => Some("record"),
        "caption" => Some("caption"),
        "voice-typing" => Some("voice-typing"),
        _ => None,
    }
}

pub(crate) fn log_segment_emit_diagnostics(
    instance_id: &str,
    first_segment_emitted: Option<&Arc<AtomicBool>>,
    segment: &TranscriptSegment,
    stage: &str,
) {
    // These logs are intentionally scoped to the long-lived live instances we
    // debug most often (`record`, `caption`, `voice-typing`), not to every
    // possible recognizer consumer.
    let Some(label) = diagnostics_instance_label(instance_id) else {
        return;
    };

    let text_len = segment.text.chars().count();
    if let Some(first_segment_emitted) = first_segment_emitted
        && first_segment_emitted
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    {
        info!(
            "[Sherpa] {label} first segment emitted. stage={} segment_id={} final={} text_len={}",
            stage, segment.id, segment.is_final, text_len
        );
    }

    info!(
        "[Sherpa] {label} emit. stage={} segment_id={} final={} text_len={}",
        stage, segment.id, segment.is_final, text_len
    );
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

pub(crate) fn build_transcript_update(
    segment: TranscriptSegment,
    options: TranscriptNormalizationOptions,
) -> TranscriptUpdate {
    sona_core::transcription::transcript::build_transcript_update_with_id_generator(
        segment,
        options,
        new_transcript_segment_id,
    )
}

pub(crate) fn emit_transcript_update(
    emitter: &dyn crate::platform::event::EventEmitter,
    instance_id: &str,
    update: &TranscriptUpdate,
    stage: &str,
    first_segment_emitted: Option<&Arc<AtomicBool>>,
) {
    let event_name = recognizer_output_event(instance_id);
    for segment in &update.upsert_segments {
        log_segment_emit_diagnostics(instance_id, first_segment_emitted, segment, stage);
    }
    let _ = emitter.emit(&event_name, serde_json::to_value(update).unwrap());
}

pub(crate) fn format_transcript(text: &str, punctuation: Option<&Punctuation>) -> String {
    let mut result = text.trim().to_string();
    if result.is_empty() {
        return result;
    }

    let has_ascii_letters = result.chars().any(|c| c.is_ascii_alphabetic());
    let is_all_caps = has_ascii_letters && result == result.to_uppercase();

    if is_all_caps {
        let mut chars = result.chars();
        if let Some(first) = chars.next() {
            let lower = chars.as_str().to_lowercase();
            result = first.to_uppercase().collect::<String>() + &lower;
        }
    }

    if let Some(p) = punctuation {
        result = p.add_punct(&result);
    }
    result
}

pub(crate) fn finalize_transcript_text(
    cleaned_text: &str,
    punctuation: Option<&Punctuation>,
) -> String {
    let formatted_text = format_transcript(cleaned_text, punctuation);
    select_final_transcript_text(cleaned_text, &formatted_text)
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
