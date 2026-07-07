use super::model_config::Punctuation;
use super::recognizer_output_event;
use super::sherpa_onnx::{diagnostics_instance_label, log_segment_emit_diagnostics};
use super::types::TranscriptUpdate;
#[cfg(test)]
use super::types::{
    TranscriptSegment, TranscriptTimingLevel, TranscriptTimingSource,
};
use log::info;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

pub(crate) use sona_core::transcript::{
    apply_timeline_normalization, build_transcript_update, normalize_recognizer_text,
    select_final_transcript_text, synthesize_durations,
};

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

pub(crate) fn preview_text_for_log(text: &str) -> String {
    const MAX_PREVIEW_CHARS: usize = 24;
    let flattened = text.replace(['\r', '\n'], " ");
    let mut preview = flattened
        .chars()
        .take(MAX_PREVIEW_CHARS)
        .collect::<String>();
    if flattened.chars().count() > MAX_PREVIEW_CHARS {
        preview.push('…');
    }
    preview
}

pub(crate) fn log_text_transform_diagnostics(
    instance_id: &str,
    stage: &str,
    segment_id: &str,
    is_final: bool,
    raw_text: &str,
    cleaned_text: &str,
    final_text: &str,
) {
    let Some(label) = diagnostics_instance_label(instance_id) else {
        return;
    };

    info!(
        "[Sherpa] {label} text transform. stage={} segment_id={} final={} raw_len={} cleaned_len={} final_len={} raw_preview={:?} cleaned_preview={:?} final_preview={:?}",
        stage,
        segment_id,
        is_final,
        raw_text.chars().count(),
        cleaned_text.chars().count(),
        final_text.chars().count(),
        preview_text_for_log(raw_text),
        preview_text_for_log(cleaned_text),
        preview_text_for_log(final_text)
    );
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
