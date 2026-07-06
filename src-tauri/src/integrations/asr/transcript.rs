use super::model_config::Punctuation;
use super::recognizer_output_event;
use super::sherpa_onnx::{diagnostics_instance_label, log_segment_emit_diagnostics};
use super::types::TranscriptUpdate;
#[cfg(test)]
use super::types::{TranscriptNormalizationOptions, TranscriptSegment};
use log::info;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

pub(crate) use sona_core::transcript::{
    apply_timeline_normalization, build_transcript_update, ensure_transcript_segment_timing,
    normalize_recognizer_text, synthesize_durations,
};

pub(crate) fn emit_transcript_update(
    emitter: &dyn crate::core::event::EventEmitter,
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

fn is_meaningful_text_char(ch: char) -> bool {
    ch.is_alphanumeric()
}

fn extract_meaningful_text(text: &str) -> String {
    text.chars()
        .filter(|ch| is_meaningful_text_char(*ch))
        .collect()
}

fn extract_ascii_digits(text: &str) -> String {
    text.chars().filter(|ch| ch.is_ascii_digit()).collect()
}

fn is_preservable_trailing_punctuation(ch: char) -> bool {
    matches!(
        ch,
        '。' | '，'
            | '！'
            | '？'
            | '：'
            | '；'
            | '、'
            | '.'
            | ','
            | '!'
            | '?'
            | ':'
            | ';'
            | ')'
            | '）'
            | ']'
            | '】'
            | '}'
            | '」'
            | '』'
            | '》'
            | '〉'
            | '"'
            | '\''
            | '”'
            | '’'
    )
}

fn extract_trailing_punctuation(text: &str) -> String {
    let trimmed = text.trim_end();
    if trimmed.is_empty() {
        return String::new();
    }

    let mut start = trimmed.len();
    for (idx, ch) in trimmed.char_indices().rev() {
        if is_preservable_trailing_punctuation(ch) {
            start = idx;
        } else {
            break;
        }
    }

    if start < trimmed.len() {
        trimmed[start..].to_string()
    } else {
        String::new()
    }
}

fn merge_cleaned_text_with_trailing_punctuation(
    cleaned_text: &str,
    formatted_text: &str,
) -> String {
    let mut result = cleaned_text.trim().to_string();
    let trailing_punctuation = extract_trailing_punctuation(formatted_text);

    if !trailing_punctuation.is_empty() && !result.ends_with(&trailing_punctuation) {
        result.push_str(&trailing_punctuation);
    }

    result
}

fn should_fallback_to_cleaned_text(cleaned_text: &str, formatted_text: &str) -> bool {
    let cleaned_meaningful = extract_meaningful_text(cleaned_text);
    if cleaned_meaningful.is_empty() {
        return false;
    }

    let formatted_meaningful = extract_meaningful_text(formatted_text);
    if formatted_meaningful.is_empty() {
        return true;
    }

    let cleaned_digits = extract_ascii_digits(cleaned_text);
    if !cleaned_digits.is_empty() && extract_ascii_digits(formatted_text) != cleaned_digits {
        return true;
    }

    false
}

fn select_final_transcript_text(cleaned_text: &str, formatted_text: &str) -> String {
    let normalized_formatted = normalize_recognizer_text(formatted_text);
    if should_fallback_to_cleaned_text(cleaned_text, &normalized_formatted) {
        return merge_cleaned_text_with_trailing_punctuation(cleaned_text, &normalized_formatted);
    }

    normalized_formatted
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

#[allow(clippy::too_many_arguments)]
#[cfg(test)]
mod tests {
    use super::super::types::{
        TranscriptTimingLevel, TranscriptTimingSource, TranscriptTimingUnit,
    };
    use super::*;

    #[test]
    fn normalize_recognizer_text_strips_multiple_leading_tags() {
        assert_eq!(
            normalize_recognizer_text("  <|zh|><|withitn|><|noise|> 123。 "),
            "123。"
        );
    }

    #[test]
    fn select_final_transcript_text_falls_back_to_cleaned_digits_when_formatting_drops_them() {
        assert_eq!(select_final_transcript_text("123", "。"), "123。");
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

    #[test]
    fn apply_timeline_normalization_splits_token_level_segments_with_model_timing() {
        let segment = TranscriptSegment {
            text: "你好。世界。".to_string(),
            tokens: Some(vec![
                "你".to_string(),
                "好".to_string(),
                "。".to_string(),
                "世".to_string(),
                "界".to_string(),
                "。".to_string(),
            ]),
            timestamps: Some(vec![0.0, 0.2, 0.4, 0.6, 0.8, 1.0]),
            durations: Some(vec![0.2; 6]),
            ..sample_segment("你好。世界。", 0.0, 1.2)
        };

        let results = apply_timeline_normalization(
            vec![segment],
            TranscriptNormalizationOptions {
                enable_timeline: true,
            },
        );

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].text, "你好。");
        assert_eq!(results[1].text, "世界。");
        assert_eq!(
            results[0].timing.as_ref().map(|timing| timing.level),
            Some(TranscriptTimingLevel::Token)
        );
        assert_eq!(
            results[0].timing.as_ref().map(|timing| timing.source),
            Some(TranscriptTimingSource::Model)
        );
        assert!(results[1].start >= results[0].end);
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

    #[test]
    fn build_transcript_update_replaces_final_segments_atomically_when_timeline_enabled() {
        let update = build_transcript_update(
            sample_segment("Hello. World.", 0.0, 2.0),
            TranscriptNormalizationOptions {
                enable_timeline: true,
            },
        );

        assert_eq!(update.remove_ids, vec!["segment-1".to_string()]);
        assert_eq!(update.upsert_segments.len(), 2);
        assert_eq!(update.upsert_segments[0].id, "segment-1");
    }
}
