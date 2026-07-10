use crate::punctuation::Punctuation;
use log::info;
use sona_core::ports::asr::{
    AsrRuntimeObserver, AsrTranscriptUpdateEvent, TranscriptNormalizationOptions,
};
use sona_core::transcription::transcript::{
    TranscriptSegment, TranscriptUpdate, build_transcript_update_with_id_generator,
    select_final_transcript_text,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub use sona_core::transcription::transcript::{normalize_recognizer_text, synthesize_durations};

fn new_transcript_segment_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub fn build_transcript_update(
    segment: TranscriptSegment,
    options: TranscriptNormalizationOptions,
) -> TranscriptUpdate {
    build_transcript_update_with_id_generator(segment, options, new_transcript_segment_id)
}

pub fn format_transcript(text: &str, punctuation: Option<&Punctuation>) -> String {
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

    if let Some(punctuation) = punctuation {
        result = punctuation.add_punct(&result);
    }
    result
}

pub fn finalize_transcript_text(cleaned_text: &str, punctuation: Option<&Punctuation>) -> String {
    let formatted_text = format_transcript(cleaned_text, punctuation);
    select_final_transcript_text(cleaned_text, &formatted_text)
}

pub fn observe_streaming_transcript_update(
    observer: &dyn AsrRuntimeObserver,
    instance_id: &str,
    update: &TranscriptUpdate,
    stage: &str,
    first_segment_emitted: Option<&Arc<AtomicBool>>,
) {
    for segment in &update.upsert_segments {
        log_segment_emit_diagnostics(instance_id, first_segment_emitted, segment, stage);
    }
    observer.on_transcript_update(&AsrTranscriptUpdateEvent {
        instance_id: instance_id.to_string(),
        stage: stage.to_string(),
        update: update.clone(),
    });
}

pub fn diagnostics_instance_label(instance_id: &str) -> Option<&'static str> {
    match instance_id {
        "record" => Some("record"),
        "caption" => Some("caption"),
        "voice-typing" => Some("voice-typing"),
        _ => None,
    }
}

pub fn log_segment_emit_diagnostics(
    instance_id: &str,
    first_segment_emitted: Option<&Arc<AtomicBool>>,
    segment: &TranscriptSegment,
    stage: &str,
) {
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

pub fn preview_text_for_log(text: &str) -> String {
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

pub fn log_text_transform_diagnostics(
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
    use super::*;
    use sona_core::ports::asr::{AsrRuntimeObserver, AsrTranscriptUpdateEvent};
    use sona_core::transcription::asr_metrics::{AsrInferenceMetric, AsrModelLoadMetric};
    use sona_core::transcription::transcript::TranscriptUpdate;
    use std::sync::Mutex;

    #[derive(Default)]
    struct RecordingObserver {
        events: Mutex<Vec<AsrTranscriptUpdateEvent>>,
    }

    impl AsrRuntimeObserver for RecordingObserver {
        fn on_transcript_update(&self, event: &AsrTranscriptUpdateEvent) {
            self.events.lock().unwrap().push(event.clone());
        }

        fn on_model_load(&self, _metric: &AsrModelLoadMetric) {}

        fn on_live_inference(&self, _metric: &AsrInferenceMetric) {}
    }

    #[test]
    fn all_caps_text_is_normalized_before_optional_punctuation() {
        assert_eq!(format_transcript("HELLO WORLD", None), "Hello world");
    }

    #[test]
    fn typed_update_preserves_instance_stage_and_payload() {
        let observer = RecordingObserver::default();
        let update = TranscriptUpdate {
            remove_ids: vec!["old".to_string()],
            upsert_segments: Vec::new(),
        };

        observe_streaming_transcript_update(&observer, "live-1", &update, "partial", None);

        assert_eq!(
            *observer.events.lock().unwrap(),
            vec![AsrTranscriptUpdateEvent {
                instance_id: "live-1".to_string(),
                stage: "partial".to_string(),
                update,
            }]
        );
    }
}
