use super::telemetry::{build_live_metric, record_live_metric};
use crate::punctuation::Punctuation;
use crate::recognizer::{SafeOfflineRecognizer, decode_offline_samples};
use log::{debug, info};
use sona_core::ports::asr::{
    AsrRuntimeObserver, AsrTranscriptUpdateEvent, TranscriptNormalizationOptions,
};
use sona_core::transcription::asr_metrics::duration_to_ms;
use sona_core::transcription::postprocess::TranscriptPostprocessor;
use sona_core::transcription::transcript::{
    TranscriptSegment, TranscriptUpdate, build_transcript_update_with_id_generator,
    select_final_transcript_text,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use sona_core::transcription::transcript::normalize_recognizer_text;
pub(super) use sona_core::transcription::transcript::synthesize_durations;

fn new_transcript_segment_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub(super) fn build_transcript_update(
    segment: TranscriptSegment,
    options: TranscriptNormalizationOptions,
) -> TranscriptUpdate {
    build_transcript_update_with_id_generator(segment, options, new_transcript_segment_id)
}

pub(super) fn format_transcript(text: &str, punctuation: Option<&Punctuation>) -> String {
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

fn finalize_transcript_text(cleaned_text: &str, punctuation: Option<&Punctuation>) -> String {
    let formatted_text = format_transcript(cleaned_text, punctuation);
    select_final_transcript_text(cleaned_text, &formatted_text)
}

pub(super) fn observe_streaming_transcript_update(
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

pub(super) fn diagnostics_instance_label(instance_id: &str) -> Option<&'static str> {
    match instance_id {
        "record" => Some("record"),
        "caption" => Some("caption"),
        "voice-typing" => Some("voice-typing"),
        _ => None,
    }
}

fn log_segment_emit_diagnostics(
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

fn preview_text_for_log(text: &str) -> String {
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

fn log_text_transform_diagnostics(
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

#[allow(clippy::too_many_arguments)]
pub(super) fn run_offline_inference(
    speech_buffer: &[Vec<f32>],
    observer: &dyn AsrRuntimeObserver,
    r: &SafeOfflineRecognizer,
    punctuation: Option<&Punctuation>,
    segment_id: &str,
    global_start: f64,
    is_final: bool,
    instance_id: &str,
    stage: &'static str,
    first_segment_emitted: Option<Arc<AtomicBool>>,
    normalization_options: TranscriptNormalizationOptions,
    postprocessor: TranscriptPostprocessor,
    record_metrics: bool,
    triggered_at: Instant,
) {
    if speech_buffer.is_empty() {
        if let Some(label) = diagnostics_instance_label(instance_id) {
            info!(
                "[Sherpa] {label} offline inference skipped because the speech buffer is empty. stage={stage}"
            );
        }
        return;
    }

    // Offline models decode one aggregated utterance at a time, so we flatten
    // the buffered speech chunks into one continuous waveform before calling
    // Sherpa.
    let mut full_audio = Vec::new();
    for chunk in speech_buffer {
        full_audio.extend_from_slice(chunk);
    }
    let decode_started = Instant::now();
    debug!(
        "[Offline] FFI: Calling decode_offline_samples with {} samples",
        full_audio.len()
    );
    let decode_result = decode_offline_samples(r, &full_audio);
    debug!("[Offline] FFI: Decode finished");
    let decode_ms = duration_to_ms(decode_started.elapsed());

    let record_metric = |emit_latency_ms: Option<f64>| {
        if record_metrics {
            record_live_metric(
                observer,
                build_live_metric(
                    instance_id,
                    stage,
                    is_final,
                    full_audio.len(),
                    decode_ms,
                    emit_latency_ms,
                    None,
                ),
            );
        }
    };

    if let Some(label) = diagnostics_instance_label(instance_id) {
        info!(
            "[Sherpa] {label} offline inference finished. stage={} segment_id={} final={} buffered_chunks={} buffered_samples={}",
            stage,
            segment_id,
            is_final,
            speech_buffer.len(),
            full_audio.len()
        );
    }

    if let Some(result) = decode_result {
        let raw_text = result.text.trim();
        if !raw_text.is_empty() {
            // The offline path emits only meaningful text: raw recognizer output
            // is normalized first, then final-only formatting/punctuation is
            // applied when this segment closes an utterance.
            let cleaned_text = normalize_recognizer_text(&result.text);
            if cleaned_text.is_empty() {
                if let Some(label) = diagnostics_instance_label(instance_id) {
                    info!(
                        "[Sherpa] {label} offline inference produced empty text after normalization. stage={} segment_id={} final={} raw_preview={:?}",
                        stage,
                        segment_id,
                        is_final,
                        preview_text_for_log(raw_text)
                    );
                }
                record_metric(Some(duration_to_ms(triggered_at.elapsed())));
                return;
            }

            let text = if is_final {
                finalize_transcript_text(&cleaned_text, punctuation)
            } else {
                cleaned_text.clone()
            };

            if text.is_empty() {
                if let Some(label) = diagnostics_instance_label(instance_id) {
                    info!(
                        "[Sherpa] {label} offline inference produced empty output text after normalization/formatting. stage={} segment_id={} final={} raw_preview={:?} cleaned_preview={:?}",
                        stage,
                        segment_id,
                        is_final,
                        preview_text_for_log(raw_text),
                        preview_text_for_log(&cleaned_text)
                    );
                }
                record_metric(Some(duration_to_ms(triggered_at.elapsed())));
                return;
            }

            log_text_transform_diagnostics(
                instance_id,
                stage,
                segment_id,
                is_final,
                raw_text,
                &cleaned_text,
                &text,
            );

            let global_end = global_start + (full_audio.len() as f64 / 16000.0);
            // Sherpa timestamps are relative to the decoded utterance, so we
            // shift them into the global recording timeline before emitting.
            let timestamps_abs: Option<Vec<f32>> = result
                .timestamps
                .as_ref()
                .map(|ts| ts.iter().map(|t| *t + global_start as f32).collect());
            let durations = timestamps_abs
                .as_ref()
                .and_then(|ts| synthesize_durations(ts, global_end as f32));

            let segment = TranscriptSegment {
                id: segment_id.to_string(),
                text,
                start: global_start,
                end: global_end,
                is_final,
                timing: None,
                tokens: Some(result.tokens),
                timestamps: timestamps_abs,
                durations,
                translation: None,
                speaker: None,
                speaker_attribution: None,
            };
            let update = postprocessor
                .process_update(build_transcript_update(segment, normalization_options));
            observe_streaming_transcript_update(
                observer,
                instance_id,
                &update,
                stage,
                first_segment_emitted.as_ref(),
            );
            record_metric(Some(duration_to_ms(triggered_at.elapsed())));
        } else if let Some(label) = diagnostics_instance_label(instance_id) {
            info!(
                "[Sherpa] {label} offline inference produced empty text after formatting. stage={} segment_id={} final={}",
                stage, segment_id, is_final
            );
            record_metric(Some(duration_to_ms(triggered_at.elapsed())));
        }
    } else if let Some(label) = diagnostics_instance_label(instance_id) {
        info!(
            "[Sherpa] {label} offline inference produced no recognizer result. stage={} segment_id={} final={}",
            stage, segment_id, is_final
        );
        record_metric(Some(duration_to_ms(triggered_at.elapsed())));
    }
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
