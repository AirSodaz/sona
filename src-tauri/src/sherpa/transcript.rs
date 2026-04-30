use super::model_config::Punctuation;
use super::recognizer_output_event;
use super::state::{diagnostics_instance_label, log_segment_emit_diagnostics};
use super::types::{
    TranscriptNormalizationOptions, TranscriptSegment, TranscriptTiming, TranscriptTimingLevel,
    TranscriptTimingSource, TranscriptTimingUnit, TranscriptUpdate,
};
use crate::text_alignment::{align_text_units_to_tokens, is_cjk_char};
use log::info;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

const MAX_SEGMENT_LENGTH_CJK: usize = 36;
const MAX_SEGMENT_LENGTH_WESTERN: usize = 84;
const ABBREVIATIONS: &[&str] = &[
    "mr", "mrs", "ms", "dr", "prof", "sr", "jr", "st", "vs", "etc", "no", "op", "vol", "fig",
    "inc", "ltd", "co", "dept",
];

#[derive(Clone, Debug)]
struct TokenMap {
    start_indices: Vec<usize>,
    end_indices: Vec<usize>,
    timestamps: Vec<f32>,
}

#[derive(Clone, Debug)]
struct SplitterState {
    current_text: String,
    current_start: f64,
    current_segment_start: f64,
    char_index: usize,
    effective_char_index: usize,
    last_token_index: usize,
    next_token_slice_start: usize,
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
    let aligned_units = align_text_units_to_tokens(text, tokens)?;

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

pub(crate) fn ensure_transcript_segment_timing(segment: &mut TranscriptSegment) {
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

fn normalize_transcript_segments(mut segments: Vec<TranscriptSegment>) -> Vec<TranscriptSegment> {
    for segment in &mut segments {
        ensure_transcript_segment_timing(segment);
    }
    segments
}

fn is_meaningful_segment_char(ch: char) -> bool {
    ch.is_alphanumeric()
}

fn effective_length(text: &str) -> usize {
    text.chars()
        .filter(|ch| is_meaningful_segment_char(*ch))
        .count()
}

fn ends_with_abbreviation(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }

    let last_word = trimmed
        .split_whitespace()
        .last()
        .unwrap_or_default()
        .trim_matches(|ch: char| !ch.is_alphanumeric())
        .to_ascii_lowercase();
    ABBREVIATIONS.iter().any(|value| *value == last_word)
}

fn contains_cjk(text: &str) -> bool {
    text.chars().any(is_cjk_char)
}

fn is_strong_split_char(ch: char) -> bool {
    matches!(ch, '.' | '?' | '!' | '。' | '？' | '！')
}

fn is_weak_split_char(ch: char) -> bool {
    matches!(ch, ',' | '，' | ';' | '；' | ':' | '：')
}

fn split_text_parts<F>(text: &str, is_delimiter: F) -> Vec<String>
where
    F: Fn(char) -> bool,
{
    let chars = text.chars().collect::<Vec<_>>();
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut index = 0usize;

    while index < chars.len() {
        if is_delimiter(chars[index]) {
            if !current.is_empty() {
                parts.push(current.clone());
                current.clear();
            }

            let start = index;
            index += 1;
            while index < chars.len() && is_delimiter(chars[index]) {
                index += 1;
            }
            parts.push(chars[start..index].iter().collect());
            continue;
        }

        current.push(chars[index]);
        index += 1;
    }

    if !current.is_empty() {
        parts.push(current);
    }

    parts
}

fn segment_has_token_timestamps(segment: &TranscriptSegment) -> bool {
    matches!(
        (segment.tokens.as_ref(), segment.timestamps.as_ref()),
        (Some(tokens), Some(timestamps)) if !tokens.is_empty() && tokens.len() == timestamps.len()
    )
}

fn build_token_map(segment: &TranscriptSegment) -> Option<TokenMap> {
    let tokens = segment.tokens.as_ref()?;
    let timestamps = segment.timestamps.as_ref()?;
    if tokens.is_empty() || tokens.len() != timestamps.len() {
        return None;
    }

    let mut start_indices = Vec::with_capacity(tokens.len());
    let mut end_indices = Vec::with_capacity(tokens.len());
    let mut current_len = 0usize;

    for token in tokens {
        let token_len = effective_length(token);
        start_indices.push(current_len);
        current_len += token_len;
        end_indices.push(current_len);
    }

    Some(TokenMap {
        start_indices,
        end_indices,
        timestamps: timestamps.clone(),
    })
}

fn find_timestamp_from_map(
    map: &TokenMap,
    effective_index: usize,
    hint_index: usize,
) -> Option<(f32, usize)> {
    if hint_index < map.start_indices.len()
        && map.start_indices[hint_index] <= effective_index
        && effective_index < map.end_indices[hint_index]
    {
        return Some((map.timestamps[hint_index], hint_index));
    }

    let next = hint_index + 1;
    if next < map.start_indices.len()
        && map.start_indices[next] <= effective_index
        && effective_index < map.end_indices[next]
    {
        return Some((map.timestamps[next], next));
    }

    let mut left = if hint_index < map.start_indices.len()
        && map.start_indices[hint_index] <= effective_index
    {
        hint_index
    } else {
        0
    };
    let mut right = map.start_indices.len().saturating_sub(1);
    let mut index = None;

    while left <= right {
        let mid = (left + right) / 2;
        if map.start_indices[mid] <= effective_index {
            index = Some(mid);
            left = mid + 1;
        } else if mid == 0 {
            break;
        } else {
            right = mid - 1;
        }
    }

    let found_index = index?;
    if effective_index >= map.end_indices[found_index] {
        return None;
    }

    Some((map.timestamps[found_index], found_index))
}

fn finalize_split_segment(
    mut segment: TranscriptSegment,
    is_first: bool,
    original_id: &str,
) -> TranscriptSegment {
    if !is_first {
        segment.id = uuid::Uuid::new_v4().to_string();
    } else {
        segment.id = original_id.to_string();
    }
    segment
}

fn split_segment_by_parts<F>(
    segment: &TranscriptSegment,
    is_delimiter: F,
    check_abbreviations: bool,
) -> Vec<TranscriptSegment>
where
    F: Fn(char) -> bool + Copy,
{
    let parts = split_text_parts(&segment.text, is_delimiter);
    if parts.len() <= 1 {
        return vec![segment.clone()];
    }

    let has_timestamps = segment_has_token_timestamps(segment);
    let token_map = if has_timestamps {
        build_token_map(segment)
    } else {
        None
    };
    let total_duration = segment.end - segment.start;
    let total_char_len = segment.text.chars().count().max(1);

    let mut state = SplitterState {
        current_text: String::new(),
        current_start: segment.start,
        current_segment_start: token_map
            .as_ref()
            .and_then(|map| map.timestamps.first().copied())
            .map(|value| value as f64)
            .unwrap_or(segment.start),
        char_index: 0,
        effective_char_index: 0,
        last_token_index: 0,
        next_token_slice_start: 0,
    };

    let mut results = Vec::new();
    let original_id = segment.id.clone();

    for part in parts {
        let part_effective_len = effective_length(&part);
        let is_delimiter_part = part.chars().next().map(is_delimiter).unwrap_or(false);

        if is_delimiter_part {
            let should_merge = check_abbreviations
                && part.contains('.')
                && ends_with_abbreviation(&state.current_text);
            state.current_text.push_str(&part);
            state.char_index += part.chars().count();
            state.effective_char_index += part_effective_len;

            if should_merge {
                continue;
            }

            let fallback_ratio = state.current_text.chars().count() as f64 / total_char_len as f64;
            let fallback_segment_end = state.current_start + fallback_ratio * total_duration;

            let mut segment_end = fallback_segment_end;
            let mut current_tokens = None;
            let mut current_timestamps = None;
            let mut current_durations = None;

            if let Some(map) = token_map.as_ref() {
                let mut slice_end = map.timestamps.len();
                if let Some((timestamp, found_index)) =
                    find_timestamp_from_map(map, state.effective_char_index, state.last_token_index)
                {
                    slice_end = found_index;
                    segment_end = timestamp as f64;
                    state.last_token_index = found_index;
                }

                if slice_end > state.next_token_slice_start {
                    if let Some(tokens) = segment.tokens.as_ref() {
                        current_tokens =
                            Some(tokens[state.next_token_slice_start..slice_end].to_vec());
                    }
                    if let Some(timestamps) = segment.timestamps.as_ref() {
                        current_timestamps =
                            Some(timestamps[state.next_token_slice_start..slice_end].to_vec());
                    }
                    if let Some(durations) = segment
                        .durations
                        .as_ref()
                        .filter(|values| values.len() == map.timestamps.len())
                    {
                        current_durations =
                            Some(durations[state.next_token_slice_start..slice_end].to_vec());
                    }
                    state.next_token_slice_start = slice_end;
                }

                if let Some(timestamps) = current_timestamps
                    .as_ref()
                    .filter(|values| !values.is_empty())
                {
                    state.current_segment_start = timestamps[0] as f64;
                }
            }

            let child = TranscriptSegment {
                id: original_id.clone(),
                text: state.current_text.trim().to_string(),
                start: state.current_segment_start,
                end: segment_end.max(state.current_segment_start),
                is_final: true,
                timing: None,
                tokens: current_tokens,
                timestamps: current_timestamps,
                durations: current_durations,
                translation: segment.translation.clone(),
                speaker: segment.speaker.clone(),
            };

            if !child.text.is_empty() {
                results.push(finalize_split_segment(
                    child,
                    results.is_empty(),
                    &original_id,
                ));
            }

            state.current_start = segment_end;
            state.current_segment_start = segment_end;
            state.current_text.clear();

            if let Some(map) = token_map.as_ref() {
                if state.last_token_index == state.next_token_slice_start
                    && state.next_token_slice_start < map.timestamps.len()
                {
                    state.current_segment_start =
                        map.timestamps[state.next_token_slice_start] as f64;
                    state.current_start = state.current_segment_start;
                }
            }

            continue;
        }

        state.current_text.push_str(&part);
        state.char_index += part.chars().count();
        state.effective_char_index += part_effective_len;
    }

    if !state.current_text.trim().is_empty() {
        let mut current_tokens = None;
        let mut current_timestamps = None;
        let mut current_durations = None;

        if let Some(map) = token_map.as_ref() {
            if let Some(tokens) = segment.tokens.as_ref() {
                current_tokens = Some(tokens[state.next_token_slice_start..].to_vec());
            }
            if let Some(timestamps) = segment.timestamps.as_ref() {
                current_timestamps = Some(timestamps[state.next_token_slice_start..].to_vec());
            }
            if let Some(durations) = segment
                .durations
                .as_ref()
                .filter(|values| values.len() == map.timestamps.len())
            {
                current_durations = Some(durations[state.next_token_slice_start..].to_vec());
            }
            if let Some(timestamps) = current_timestamps
                .as_ref()
                .filter(|values| !values.is_empty())
            {
                state.current_segment_start = timestamps[0] as f64;
            }
        }

        let child = TranscriptSegment {
            id: original_id.clone(),
            text: state.current_text.trim().to_string(),
            start: state.current_segment_start,
            end: segment.end,
            is_final: true,
            timing: None,
            tokens: current_tokens,
            timestamps: current_timestamps,
            durations: current_durations,
            translation: segment.translation.clone(),
            speaker: segment.speaker.clone(),
        };

        if !child.text.is_empty() {
            results.push(finalize_split_segment(
                child,
                results.is_empty(),
                &original_id,
            ));
        }
    }

    if results.is_empty() {
        vec![segment.clone()]
    } else {
        results
    }
}

fn split_segment_by_punctuation_rules(segment: &TranscriptSegment) -> Vec<TranscriptSegment> {
    let first_pass = split_segment_by_parts(segment, is_strong_split_char, true);
    let mut final_segments = Vec::new();

    for segment in first_pass {
        let limit = if contains_cjk(&segment.text) {
            MAX_SEGMENT_LENGTH_CJK
        } else {
            MAX_SEGMENT_LENGTH_WESTERN
        };

        if segment.text.chars().count() > limit {
            final_segments.extend(split_segment_by_parts(&segment, is_weak_split_char, false));
        } else {
            final_segments.push(segment);
        }
    }

    final_segments
}

pub(crate) fn apply_timeline_normalization(
    segments: Vec<TranscriptSegment>,
    options: TranscriptNormalizationOptions,
) -> Vec<TranscriptSegment> {
    if !options.enable_timeline {
        return normalize_transcript_segments(segments);
    }

    let mut results = Vec::new();
    for segment in segments {
        if segment.is_final {
            results.extend(split_segment_by_punctuation_rules(&segment));
        } else {
            results.push(segment);
        }
    }

    results.sort_by(|left, right| {
        left.start
            .partial_cmp(&right.start)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                left.end
                    .partial_cmp(&right.end)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    normalize_transcript_segments(results)
}

pub(crate) fn build_transcript_update(
    segment: TranscriptSegment,
    options: TranscriptNormalizationOptions,
) -> TranscriptUpdate {
    let remove_ids = if options.enable_timeline && segment.is_final {
        vec![segment.id.clone()]
    } else {
        Vec::new()
    };

    TranscriptUpdate {
        remove_ids,
        upsert_segments: apply_timeline_normalization(vec![segment], options),
    }
}

pub(crate) fn emit_transcript_update<R: tauri::Runtime>(
    app: &AppHandle<R>,
    instance_id: &str,
    update: &TranscriptUpdate,
    stage: &str,
    first_segment_emitted: Option<&Arc<AtomicBool>>,
) {
    let event_name = recognizer_output_event(instance_id);
    for segment in &update.upsert_segments {
        log_segment_emit_diagnostics(instance_id, first_segment_emitted, segment, stage);
    }
    let _ = app.emit(&event_name, update);
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

pub(crate) fn normalize_recognizer_text(text: &str) -> String {
    let mut result = text.trim();

    while result.starts_with("<|") && result.contains("|>") {
        let Some(tag_end) = result.find("|>") else {
            break;
        };
        result = result[tag_end + 2..].trim();
    }

    result.trim().to_string()
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
    let flattened = text.replace('\r', " ").replace('\n', " ");
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

pub(crate) fn synthesize_durations(timestamps: &[f32], end_time: f32) -> Option<Vec<f32>> {
    if timestamps.is_empty() {
        return None;
    }
    let mut durations = Vec::with_capacity(timestamps.len());
    for i in 0..timestamps.len() {
        let next_time = if i + 1 < timestamps.len() {
            timestamps[i + 1]
        } else {
            end_time
        };
        durations.push(next_time - timestamps[i]);
    }
    Some(durations)
}

#[allow(clippy::too_many_arguments)]
#[cfg(test)]
mod tests {
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
