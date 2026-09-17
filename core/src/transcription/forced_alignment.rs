use crate::ports::aligner::AlignerPortError;
use crate::transcription::transcript::{
    TranscriptSegment, TranscriptTiming, TranscriptTimingLevel, TranscriptTimingSource,
    TranscriptTimingUnit, ensure_transcript_segment_timing,
};
use std::collections::HashMap;

/// Configuration for CTC Trellis dynamic programming forced alignment.
#[derive(Clone, Debug, PartialEq)]
pub struct TrellisConfig {
    /// Frame duration in seconds (e.g., 0.020 for MMS/Wav2Vec2 20ms frames,
    /// 0.040 for NeMo Conformer 40ms frames).
    pub time_per_frame_sec: f64,
    /// CTC blank token index (0 for MMS / Wav2Vec2, often V-1 for NeMo).
    pub blank_id: usize,
}

impl Default for TrellisConfig {
    fn default() -> Self {
        Self {
            // MMS-300M-FA defaults: 16kHz audio / 320 stride = 50Hz = 20ms per frame.
            time_per_frame_sec: 0.020,
            // MMS blank token is 0 (<pad>).
            blank_id: 0,
        }
    }
}

impl TrellisConfig {
    /// Configuration preset for Meta MMS-300M-FA (Wav2Vec2 CTC).
    pub fn mms_300m_fa() -> Self {
        Self::default()
    }

    /// Configuration preset for NVIDIA NeMo Conformer-CTC (4x subsampling, 40ms frames).
    pub fn nemo_conformer_ctc(vocab_size: usize) -> Self {
        Self {
            time_per_frame_sec: 0.040,
            blank_id: vocab_size.saturating_sub(1),
        }
    }
}

/// A target token to be aligned to acoustic frames.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetToken {
    /// Vocabulary ID matching the emission matrix class dimension.
    pub id: usize,
    /// Human-readable token text (e.g., character or subword string).
    pub text: String,
    /// Index of the word to which this token belongs.
    pub word_index: usize,
}

/// The extracted frame span for an aligned token.
#[derive(Clone, Debug, PartialEq)]
pub struct TokenSpan {
    pub token_id: usize,
    pub text: String,
    pub word_index: usize,
    pub start_frame: usize,
    /// Exclusive end frame index.
    pub end_frame: usize,
    /// Average log-probability score across the assigned frames.
    pub score: f32,
}

impl TokenSpan {
    pub fn duration_frames(&self) -> usize {
        self.end_frame.saturating_sub(self.start_frame)
    }

    pub fn start_time(&self, time_per_frame_sec: f64, offset_sec: f64) -> f64 {
        self.start_frame as f64 * time_per_frame_sec + offset_sec
    }

    pub fn end_time(&self, time_per_frame_sec: f64, offset_sec: f64) -> f64 {
        self.end_frame as f64 * time_per_frame_sec + offset_sec
    }
}

/// An aggregated aligned word containing its timestamp boundaries and confidence score.
#[derive(Clone, Debug, PartialEq)]
pub struct AlignedWord {
    pub text: String,
    pub start: f64,
    pub end: f64,
    pub score: f32,
}

impl AlignedWord {
    pub fn to_timing_unit(&self) -> TranscriptTimingUnit {
        TranscriptTimingUnit {
            text: self.text.clone(),
            start: self.start,
            end: self.end.max(self.start),
        }
    }
}

/// Computes CTC forced alignment on a 2D emission matrix using the Viterbi Trellis algorithm.
///
/// `emissions` is a flat slice of length `num_frames * num_classes` containing log probabilities.
/// `tokens` is the sequence of target tokens.
pub fn ctc_trellis_align(
    emissions: &[f32],
    num_frames: usize,
    num_classes: usize,
    tokens: &[TargetToken],
    config: &TrellisConfig,
) -> Result<Vec<TokenSpan>, AlignerPortError> {
    if num_classes == 0 {
        return Err(AlignerPortError::invalid_request(
            "Emission classes dimension cannot be 0",
        ));
    }

    if emissions.len() != num_frames * num_classes {
        return Err(AlignerPortError::invalid_request(format!(
            "Emission buffer size mismatch: expected {} ({} frames * {} classes), got {}",
            num_frames * num_classes,
            num_frames,
            num_classes,
            emissions.len()
        )));
    }

    if tokens.is_empty() {
        return Ok(Vec::new());
    }

    if num_frames == 0 {
        return Err(AlignerPortError::audio_too_short(
            "Cannot align tokens against 0 audio frames",
        ));
    }

    if emissions.iter().any(|v| v.is_nan()) {
        return Err(AlignerPortError::invalid_request(
            "Emission buffer contains NaN values",
        ));
    }

    let u = tokens.len();
    // Extended CTC states: S = [blank, y_0, blank, y_1, ..., blank, y_{U-1}, blank]
    // Total states L = 2 * U + 1
    let num_states = 2 * u + 1;

    let mut state_labels = Vec::with_capacity(num_states);
    for token in tokens {
        state_labels.push(config.blank_id);
        state_labels.push(token.id);
    }
    state_labels.push(config.blank_id);

    // Minimum frames required: each token needs at least 1 frame, plus mandatory blanks
    // between identical adjacent tokens.
    let mut min_required_frames = u;
    for i in 0..u.saturating_sub(1) {
        if tokens[i].id == tokens[i + 1].id {
            min_required_frames += 1;
        }
    }

    if num_frames < min_required_frames {
        return Err(AlignerPortError::audio_too_short(format!(
            "Audio has {num_frames} frames, but target text requires at least {min_required_frames} frames"
        )));
    }

    // Verify all token IDs fit in the emission classes and do not collide with blank
    for (idx, token) in tokens.iter().enumerate() {
        if token.id >= num_classes {
            return Err(AlignerPortError::invalid_request(format!(
                "Token index {} (id {}) exceeds emission classes ({})",
                idx, token.id, num_classes
            )));
        }
        if token.id == config.blank_id {
            return Err(AlignerPortError::invalid_request(format!(
                "Token index {} (id {}) matches blank id {}",
                idx, token.id, config.blank_id
            )));
        }
    }
    if config.blank_id >= num_classes {
        return Err(AlignerPortError::invalid_request(format!(
            "Blank id {} exceeds emission classes ({})",
            config.blank_id, num_classes
        )));
    }

    // Memory-optimized Trellis DP: keep only two rows of float scores (prev and curr)
    // to fit entirely inside CPU L1 cache, while storing transitions in a flat u8 backtrack table.
    let mut prev_trellis = vec![f32::NEG_INFINITY; num_states];
    let mut curr_trellis = vec![f32::NEG_INFINITY; num_states];
    // Backtrack transitions: 0 = stay (from s), 1 = from s-1, 2 = from s-2
    let mut backtrack = vec![0u8; num_frames * num_states];

    // t = 0 initialization: path can begin at state 0 (blank) or state 1 (first token)
    let emission_val_0 = emissions[state_labels[0]];
    if emission_val_0.is_finite() {
        prev_trellis[0] = emission_val_0;
    }

    let emission_val_1 = emissions[state_labels[1]];
    if emission_val_1.is_finite() {
        prev_trellis[1] = emission_val_1;
    }

    // Forward pass
    for t in 1..num_frames {
        let frame_offset = t * num_classes;
        let backtrack_row = t * num_states;
        curr_trellis.fill(f32::NEG_INFINITY);

        for s in 0..num_states {
            let label = state_labels[s];
            let emission = emissions[frame_offset + label];

            // Option 0: stay in state s
            let mut best_score = prev_trellis[s];
            let mut best_choice = 0u8;

            // Option 1: transition from state s-1
            if s > 0 {
                let score_from_prev = prev_trellis[s - 1];
                if score_from_prev > best_score {
                    best_score = score_from_prev;
                    best_choice = 1u8;
                }
            }

            // Option 2: skip blank between distinct tokens (transition from s-2)
            // Allowed if s >= 2, state s is a token state (s is odd), and label != state_labels[s - 2]
            if s >= 2 && s % 2 == 1 && label != state_labels[s - 2] {
                let score_skip_blank = prev_trellis[s - 2];
                if score_skip_blank > best_score {
                    best_score = score_skip_blank;
                    best_choice = 2u8;
                }
            }

            if best_score.is_finite() && emission.is_finite() {
                let total = best_score + emission;
                if total.is_finite() {
                    curr_trellis[s] = total;
                    backtrack[backtrack_row + s] = best_choice;
                }
            }
        }

        std::mem::swap(&mut prev_trellis, &mut curr_trellis);
    }

    // Backtracking from t = num_frames - 1:
    // prev_trellis now holds the scores of the last frame (after the final swap).
    let final_blank_state = num_states - 1; // 2U
    let final_token_state = num_states - 2; // 2U - 1

    let score_blank = prev_trellis[final_blank_state];
    let score_token = prev_trellis[final_token_state];

    let current_state = match (score_blank.is_finite(), score_token.is_finite()) {
        (true, true) => {
            if score_blank > score_token {
                final_blank_state
            } else {
                final_token_state
            }
        }
        (true, false) => final_blank_state,
        (false, true) => final_token_state,
        (false, false) => {
            return Err(AlignerPortError::alignment_failed(
                "No valid alignment path found connecting target tokens to acoustic frames",
            ));
        }
    };

    let mut path = vec![0usize; num_frames];
    let mut state = current_state;
    path[num_frames - 1] = state;

    for t in (1..num_frames).rev() {
        let choice = backtrack[t * num_states + state];
        state = state.saturating_sub(choice as usize);
        path[t - 1] = state;
    }

    // A valid CTC path must reach the start of the sequence (state 0 or 1)
    if path[0] > 1 {
        return Err(AlignerPortError::alignment_failed(
            "Backtracked alignment path did not reach the initial token",
        ));
    }

    // Fast O(T + U) extraction of token boundaries
    #[derive(Clone, Copy)]
    struct SpanStats {
        first_frame: usize,
        last_frame: usize,
        score_sum: f32,
        frame_count: usize,
    }

    let mut stats = vec![
        SpanStats {
            first_frame: usize::MAX,
            last_frame: 0,
            score_sum: 0.0,
            frame_count: 0,
        };
        u
    ];

    for (t, &st) in path.iter().enumerate() {
        if st % 2 == 1 {
            let k = (st - 1) / 2;
            if k < u {
                let s = &mut stats[k];
                if s.first_frame == usize::MAX {
                    s.first_frame = t;
                }
                s.last_frame = t;
                s.score_sum += emissions[t * num_classes + tokens[k].id];
                s.frame_count += 1;
            }
        }
    }

    let mut token_spans = Vec::with_capacity(u);
    for (k, token) in tokens.iter().enumerate() {
        let s = stats[k];
        let (start_frame, end_frame, avg_score) = if s.frame_count > 0 {
            (
                s.first_frame,
                s.last_frame + 1,
                s.score_sum / s.frame_count as f32,
            )
        } else {
            let fallback = token_spans
                .last()
                .map(|prev: &TokenSpan| prev.end_frame)
                .unwrap_or(0);
            (fallback, fallback.max(1), 0.0)
        };

        token_spans.push(TokenSpan {
            token_id: token.id,
            text: token.text.clone(),
            word_index: token.word_index,
            start_frame,
            end_frame,
            score: avg_score,
        });
    }

    Ok(token_spans)
}

/// Convenience wrapper for 2D matrix emissions `[num_frames][num_classes]`.
pub fn ctc_trellis_align_matrix(
    emissions: &[Vec<f32>],
    tokens: &[TargetToken],
    config: &TrellisConfig,
) -> Result<Vec<TokenSpan>, AlignerPortError> {
    if emissions.is_empty() {
        return ctc_trellis_align(&[], 0, 0, tokens, config);
    }
    let num_frames = emissions.len();
    let num_classes = emissions[0].len();
    let mut flat = Vec::with_capacity(num_frames * num_classes);
    for frame in emissions {
        if frame.len() != num_classes {
            return Err(AlignerPortError::invalid_request(
                "Inconsistent class dimension across emission frames",
            ));
        }
        flat.extend_from_slice(frame);
    }
    ctc_trellis_align(&flat, num_frames, num_classes, tokens, config)
}

/// Aggregates token spans into word-level timing units.
///
/// Converts frame numbers to seconds using `config.time_per_frame_sec` and adds `time_offset_sec`.
pub fn aggregate_words_timing(
    token_spans: &[TokenSpan],
    time_per_frame_sec: f64,
    time_offset_sec: f64,
) -> Vec<TranscriptTimingUnit> {
    if token_spans.is_empty() {
        return Vec::new();
    }

    let mut words: Vec<TranscriptTimingUnit> = Vec::new();
    let mut current_word_idx: Option<usize> = None;
    let mut current_text = String::new();
    let mut current_start_frame = 0usize;
    let mut current_end_frame = 0usize;

    for span in token_spans {
        match current_word_idx {
            Some(idx) if idx == span.word_index => {
                current_text.push_str(&span.text);
                current_start_frame = current_start_frame.min(span.start_frame);
                current_end_frame = current_end_frame.max(span.end_frame);
            }
            Some(_) => {
                let start = current_start_frame as f64 * time_per_frame_sec + time_offset_sec;
                let end = current_end_frame as f64 * time_per_frame_sec + time_offset_sec;
                words.push(TranscriptTimingUnit {
                    text: current_text,
                    start,
                    end: end.max(start),
                });

                current_word_idx = Some(span.word_index);
                current_text = span.text.clone();
                current_start_frame = span.start_frame;
                current_end_frame = span.end_frame;
            }
            None => {
                current_word_idx = Some(span.word_index);
                current_text = span.text.clone();
                current_start_frame = span.start_frame;
                current_end_frame = span.end_frame;
            }
        }
    }

    if current_word_idx.is_some() {
        let start = current_start_frame as f64 * time_per_frame_sec + time_offset_sec;
        let end = current_end_frame as f64 * time_per_frame_sec + time_offset_sec;
        words.push(TranscriptTimingUnit {
            text: current_text,
            start,
            end: end.max(start),
        });
    }

    words
}

/// Applies aligned timing units to a `TranscriptSegment` and normalizes timing invariants.
pub fn apply_alignment_to_transcript_segment(
    segment: &mut TranscriptSegment,
    timing_units: Vec<TranscriptTimingUnit>,
) {
    if !timing_units.is_empty() {
        if let (Some(first), Some(last)) = (timing_units.first(), timing_units.last()) {
            segment.start = first.start;
            segment.end = last.end.max(first.start);
        }
        segment.timing = Some(TranscriptTiming {
            level: TranscriptTimingLevel::Token,
            source: TranscriptTimingSource::Model,
            units: timing_units,
        });
    }
    ensure_transcript_segment_timing(segment);
}

/// Extracts a subslice of audio samples around `[start_sec, end_sec]` with padding,
/// clamped to the bounds of `audio`.
///
/// Returns the sliced samples and `actual_slice_start_sec` (timestamp of frame 0 in the slice).
pub fn slice_audio_with_padding(
    audio: &[f32],
    sample_rate: u32,
    start_sec: f64,
    end_sec: f64,
    padding_sec: f64,
) -> (Vec<f32>, f64) {
    if audio.is_empty() || sample_rate == 0 {
        return (Vec::new(), start_sec.max(0.0));
    }

    let clamped_start = (start_sec - padding_sec).max(0.0);
    let clamped_end = (end_sec + padding_sec).max(clamped_start);

    let start_idx = ((clamped_start * sample_rate as f64).floor() as usize).min(audio.len());
    let end_idx = ((clamped_end * sample_rate as f64).ceil() as usize).min(audio.len());

    let actual_start_sec = start_idx as f64 / sample_rate as f64;
    let slice = if start_idx < end_idx {
        audio[start_idx..end_idx].to_vec()
    } else {
        Vec::new()
    };

    (slice, actual_start_sec)
}

/// Synthesizes fallback `TranscriptTimingUnit`s for text across `[start_sec, end_sec]`,
/// dividing time proportionally across words/CJK characters.
pub fn synthesize_fallback_timing_units(
    text: &str,
    start_sec: f64,
    end_sec: f64,
) -> Vec<TranscriptTimingUnit> {
    let effective_start = start_sec.max(0.0);
    let effective_end = end_sec.max(effective_start);
    let total_duration = (effective_end - effective_start).max(0.001);

    let mut units = crate::transcription::text_alignment::lex_text_units(text);
    units.retain(|unit| !unit.text.trim().is_empty());
    if units.is_empty() {
        return Vec::new();
    }

    let count = units.len();
    let step = total_duration / count as f64;

    units
        .into_iter()
        .enumerate()
        .map(|(i, unit)| {
            let start = effective_start + i as f64 * step;
            let end = if i + 1 == count {
                effective_end
            } else {
                start + step
            };
            TranscriptTimingUnit {
                text: unit.text,
                start,
                end: end.max(start),
            }
        })
        .collect()
}

/// Applies fallback timing units to a segment if its timing level is not already `Token`.
pub fn apply_fallback_timing_to_transcript_segment(segment: &mut TranscriptSegment) {
    if let Some(timing) = segment.timing.as_ref()
        && timing.level == TranscriptTimingLevel::Token
        && !timing.units.is_empty()
    {
        ensure_transcript_segment_timing(segment);
        return;
    }

    let units = synthesize_fallback_timing_units(&segment.text, segment.start, segment.end);
    if !units.is_empty() {
        segment.timing = Some(TranscriptTiming {
            level: TranscriptTimingLevel::Token,
            source: TranscriptTimingSource::Derived,
            units,
        });
    }
    ensure_transcript_segment_timing(segment);
}

/// MMS / Fairseq style character vocabulary dictionary.
#[derive(Clone, Debug)]
pub struct MmsDictionary {
    token_to_id: HashMap<String, usize>,
    id_to_token: HashMap<usize, String>,
    blank_id: usize,
    star_id: Option<usize>,
}

impl MmsDictionary {
    /// Constructs a dictionary from lines of text.
    /// Supports both simple one-token-per-line and fairseq `token count` formats.
    pub fn from_lines(lines: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        let mut token_to_id = HashMap::new();
        let mut id_to_token = HashMap::new();
        let mut blank_id = 0;
        let mut star_id = None;
        let mut next_id = 0usize;

        for line in lines {
            let line_ref = line.as_ref().trim_end_matches(['\r', '\n']);
            let line_trimmed = line_ref.trim();
            if line_trimmed.is_empty() {
                continue;
            }

            // Support fairseq format "token count" where count is parsed from the end,
            // as well as simple single-token-per-line format.
            let token = if let Some((left, right)) = line_trimmed.rsplit_once(' ') {
                if right.trim().parse::<u64>().is_ok() {
                    left.trim()
                } else {
                    line_trimmed
                }
            } else {
                line_trimmed
            };

            if token.is_empty() {
                continue;
            }

            let token_str = token.to_string();
            if token == "<pad>" || token == "<blank>" || token == "-" {
                blank_id = next_id;
            } else if token == "<star>" || token == "*" {
                star_id = Some(next_id);
            }

            token_to_id.insert(token_str.clone(), next_id);
            id_to_token.insert(next_id, token_str);
            next_id += 1;
        }

        Self {
            token_to_id,
            id_to_token,
            blank_id,
            star_id,
        }
    }

    pub fn len(&self) -> usize {
        self.token_to_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.token_to_id.is_empty()
    }

    pub fn blank_id(&self) -> usize {
        self.blank_id
    }

    pub fn star_id(&self) -> Option<usize> {
        self.star_id
    }

    pub fn get_token_id(&self, token: &str) -> Option<usize> {
        self.token_to_id.get(token).copied()
    }

    pub fn get_token_str(&self, id: usize) -> Option<&str> {
        self.id_to_token.get(&id).map(|s| s.as_str())
    }

    /// Tokenizes input text into a sequence of `TargetToken`s.
    ///
    /// Preserves word grouping:
    /// - Words separated by whitespace.
    /// - For CJK ideographic characters, each character is treated as an individual word unit.
    pub fn tokenize_text(&self, text: &str) -> Vec<TargetToken> {
        let mut tokens = Vec::new();
        let mut word_idx = 0usize;

        // Split text by whitespace first
        for raw_word in text.split_whitespace() {
            let normalized_word = raw_word.to_lowercase();
            let mut word_has_tokens = false;

            for ch in normalized_word.chars() {
                // Normalize smart/curly single quotes to ASCII quote
                let ch = match ch {
                    '’' | '‘' | '\u{FF07}' => '\'',
                    other => other,
                };

                let ch_str = ch.to_string();

                // If character is a clause/word delimiter punctuation and not part of the vocabulary,
                // advance word_idx if current word already has tokens so that unspaced phrases
                // (e.g. "hello,world") split cleanly without word index collisions or gaps.
                if is_word_delimiter_punctuation(ch) && !self.token_to_id.contains_key(&ch_str) {
                    if word_has_tokens {
                        word_idx += 1;
                        word_has_tokens = false;
                    }
                    continue;
                }

                // Ignore other non-delimiter punctuation if not in vocabulary
                if ch.is_ascii_punctuation()
                    && ch != '\''
                    && !self.token_to_id.contains_key(&ch_str)
                {
                    continue;
                }

                if let Some(id) = self.token_to_id.get(&ch_str).copied() {
                    let is_cjk = crate::transcription::text_alignment::is_cjk_char(ch);
                    if is_cjk && word_has_tokens {
                        word_idx += 1;
                    }

                    tokens.push(TargetToken {
                        id,
                        text: ch_str,
                        word_index: word_idx,
                    });

                    if is_cjk {
                        word_idx += 1;
                        word_has_tokens = false;
                    } else {
                        word_has_tokens = true;
                    }
                }
            }

            if word_has_tokens {
                word_idx += 1;
            }
        }

        tokens
    }
}

fn is_word_delimiter_punctuation(ch: char) -> bool {
    matches!(
        ch,
        ',' | ';' | ':' | '!' | '?' | '/' | '\\' | '|'
            | '\u{FF0C}' // Fullwidth comma
            | '\u{FF1B}' // Fullwidth semicolon
            | '\u{FF1A}' // Fullwidth colon
            | '\u{FF01}' // Fullwidth exclamation
            | '\u{FF1F}' // Fullwidth question
            | '\u{3001}' // Ideographic comma
            | '\u{3002}' // Ideographic full stop
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::aligner::AlignerPortErrorKind;

    fn make_synthetic_emissions(
        num_frames: usize,
        num_classes: usize,
        peaks: &[(usize, usize, usize)], // (token_id, start_frame, end_frame)
    ) -> Vec<f32> {
        // Initialize with high probability for blank (id 0)
        let mut emissions = vec![-10.0f32; num_frames * num_classes];
        for t in 0..num_frames {
            emissions[t * num_classes] = 0.0; // blank has highest log-prob by default
        }

        // Apply peaks
        for &(token_id, start, end) in peaks {
            for t in start..end.min(num_frames) {
                emissions[t * num_classes] = -10.0; // blank suppressed
                emissions[t * num_classes + token_id] = 0.0; // target token dominant
            }
        }
        emissions
    }

    #[test]
    fn test_trellis_align_synthetic_peaks() {
        // Classes: 0: blank, 1: 'c', 2: 'a', 3: 't'
        let num_classes = 4;
        let num_frames = 30;
        let config = TrellisConfig {
            time_per_frame_sec: 0.020,
            blank_id: 0,
        };

        // Peak layout:
        // 0..5: blank
        // 5..10: 'c' (id 1)
        // 10..15: blank
        // 15..20: 'a' (id 2)
        // 20..22: blank
        // 22..28: 't' (id 3)
        // 28..30: blank
        let emissions = make_synthetic_emissions(
            num_frames,
            num_classes,
            &[(1, 5, 10), (2, 15, 20), (3, 22, 28)],
        );

        let tokens = vec![
            TargetToken {
                id: 1,
                text: "c".to_string(),
                word_index: 0,
            },
            TargetToken {
                id: 2,
                text: "a".to_string(),
                word_index: 0,
            },
            TargetToken {
                id: 3,
                text: "t".to_string(),
                word_index: 0,
            },
        ];

        let spans = ctc_trellis_align(&emissions, num_frames, num_classes, &tokens, &config)
            .expect("alignment should succeed");

        assert_eq!(spans.len(), 3);
        // Verify frame boundaries
        assert_eq!(spans[0].text, "c");
        assert_eq!(spans[0].start_frame, 5);
        assert_eq!(spans[0].end_frame, 10);

        assert_eq!(spans[1].text, "a");
        assert_eq!(spans[1].start_frame, 15);
        assert_eq!(spans[1].end_frame, 20);

        assert_eq!(spans[2].text, "t");
        assert_eq!(spans[2].start_frame, 22);
        assert_eq!(spans[2].end_frame, 28);
    }

    #[test]
    fn test_trellis_handles_repeated_tokens_with_mandatory_blank() {
        // "look": l, o, o, k -> identical tokens 'o' and 'o' require blank between them
        let num_classes = 5;
        let num_frames = 25;
        let config = TrellisConfig {
            time_per_frame_sec: 0.020,
            blank_id: 0,
        };

        // 1: 'l', 2: 'o', 3: 'k'
        let emissions = make_synthetic_emissions(
            num_frames,
            num_classes,
            &[
                (1, 2, 5),   // 'l'
                (2, 6, 9),   // first 'o'
                (2, 12, 16), // second 'o' with blank at 9..12
                (3, 18, 22), // 'k'
            ],
        );

        let tokens = vec![
            TargetToken {
                id: 1,
                text: "l".to_string(),
                word_index: 0,
            },
            TargetToken {
                id: 2,
                text: "o".to_string(),
                word_index: 0,
            },
            TargetToken {
                id: 2,
                text: "o".to_string(),
                word_index: 0,
            },
            TargetToken {
                id: 3,
                text: "k".to_string(),
                word_index: 0,
            },
        ];

        let spans = ctc_trellis_align(&emissions, num_frames, num_classes, &tokens, &config)
            .expect("should align repeated tokens");

        assert_eq!(spans.len(), 4);
        assert_eq!(spans[1].text, "o");
        assert_eq!(spans[1].start_frame, 6);
        assert_eq!(spans[1].end_frame, 9);

        assert_eq!(spans[2].text, "o");
        assert_eq!(spans[2].start_frame, 12);
        assert_eq!(spans[2].end_frame, 16);
    }

    #[test]
    fn test_trellis_audio_too_short_error() {
        let config = TrellisConfig::default();
        let tokens = vec![
            TargetToken {
                id: 1,
                text: "a".to_string(),
                word_index: 0,
            },
            TargetToken {
                id: 2,
                text: "b".to_string(),
                word_index: 0,
            },
            TargetToken {
                id: 3,
                text: "c".to_string(),
                word_index: 0,
            },
        ];
        // 3 tokens require at least 3 frames, provide only 2
        let emissions = vec![0.0f32; 2 * 4];
        let err = ctc_trellis_align(&emissions, 2, 4, &tokens, &config).unwrap_err();
        assert_eq!(err.kind, AlignerPortErrorKind::AudioTooShort);
    }

    #[test]
    fn test_aggregate_words_timing() {
        let spans = vec![
            TokenSpan {
                token_id: 1,
                text: "h".to_string(),
                word_index: 0,
                start_frame: 10,
                end_frame: 15,
                score: -0.1,
            },
            TokenSpan {
                token_id: 2,
                text: "i".to_string(),
                word_index: 0,
                start_frame: 15,
                end_frame: 20,
                score: -0.2,
            },
            TokenSpan {
                token_id: 3,
                text: "w".to_string(),
                word_index: 1,
                start_frame: 30,
                end_frame: 35,
                score: -0.1,
            },
        ];

        // 20ms per frame, offset 1.0s
        let words = aggregate_words_timing(&spans, 0.020, 1.0);
        assert_eq!(words.len(), 2);

        // "hi": start = 10 * 0.020 + 1.0 = 1.20s, end = 20 * 0.020 + 1.0 = 1.40s
        assert_eq!(words[0].text, "hi");
        assert!((words[0].start - 1.20).abs() < 1e-4);
        assert!((words[0].end - 1.40).abs() < 1e-4);

        // "w": start = 30 * 0.020 + 1.0 = 1.60s, end = 35 * 0.020 + 1.0 = 1.70s
        assert_eq!(words[1].text, "w");
        assert!((words[1].start - 1.60).abs() < 1e-4);
        assert!((words[1].end - 1.70).abs() < 1e-4);
    }

    #[test]
    fn test_mms_dictionary_tokenization() {
        let lines = vec![
            "<pad> 1000",
            "h 500",
            "e 400",
            "l 300",
            "o 200",
            "w 100",
            "r 50",
            "d 40",
            "你 10",
            "好 10",
        ];
        let dict = MmsDictionary::from_lines(lines);
        assert_eq!(dict.blank_id(), 0);

        let tokens = dict.tokenize_text("Hello World! 你好");
        // "hello" -> 5 tokens with word_index 0
        // "world" -> 5 tokens with word_index 1
        // "你" -> 1 token with word_index 2
        // "好" -> 1 token with word_index 3
        assert_eq!(tokens[0].text, "h");
        assert_eq!(tokens[0].word_index, 0);
        assert_eq!(tokens[4].text, "o");
        assert_eq!(tokens[4].word_index, 0);

        assert_eq!(tokens[5].text, "w");
        assert_eq!(tokens[5].word_index, 1);

        assert_eq!(tokens[10].text, "你");
        assert_eq!(tokens[10].word_index, 2);

        assert_eq!(tokens[11].text, "好");
        assert_eq!(tokens[11].word_index, 3);
    }

    #[test]
    fn test_apply_alignment_to_transcript_segment() {
        let mut segment = TranscriptSegment {
            id: "seg-1".to_string(),
            text: "Hello world".to_string(),
            start: 1.0,
            end: 3.0,
            is_final: true,
            timing: None,
            tokens: None,
            timestamps: None,
            durations: None,
            translation: None,
            speaker: None,
            speaker_attribution: None,
        };

        let timing_units = vec![
            TranscriptTimingUnit {
                text: "Hello".to_string(),
                start: 1.1,
                end: 1.8,
            },
            TranscriptTimingUnit {
                text: "world".to_string(),
                start: 2.0,
                end: 2.8,
            },
        ];

        apply_alignment_to_transcript_segment(&mut segment, timing_units);

        let timing = segment.timing.expect("timing should be populated");
        assert_eq!(timing.level, TranscriptTimingLevel::Token);
        assert_eq!(timing.source, TranscriptTimingSource::Model);
        assert_eq!(timing.units.len(), 2);
        assert_eq!(timing.units[0].text, "Hello");
        assert_eq!(timing.units[1].text, "world");
    }

    #[test]
    fn test_trellis_empty_tokens() {
        let config = TrellisConfig::default();
        let emissions = vec![0.0f32; 10 * 5];
        let spans = ctc_trellis_align(&emissions, 10, 5, &[], &config)
            .expect("empty tokens should return empty vec");
        assert!(spans.is_empty());
    }

    #[test]
    fn test_trellis_zero_frames_or_classes_error() {
        let config = TrellisConfig::default();
        let tokens = vec![TargetToken {
            id: 1,
            text: "a".to_string(),
            word_index: 0,
        }];

        let err = ctc_trellis_align(&[], 0, 5, &tokens, &config).unwrap_err();
        assert_eq!(err.kind, AlignerPortErrorKind::AudioTooShort);

        let err_zero_classes = ctc_trellis_align(&[], 5, 0, &tokens, &config).unwrap_err();
        assert_eq!(err_zero_classes.kind, AlignerPortErrorKind::InvalidRequest);
    }

    #[test]
    fn test_trellis_nan_emissions_error() {
        let config = TrellisConfig::default();
        let tokens = vec![TargetToken {
            id: 1,
            text: "a".to_string(),
            word_index: 0,
        }];
        let mut emissions = vec![0.0f32; 5 * 2];
        emissions[3] = f32::NAN;
        let err = ctc_trellis_align(&emissions, 5, 2, &tokens, &config).unwrap_err();
        assert_eq!(err.kind, AlignerPortErrorKind::InvalidRequest);
    }

    #[test]
    fn test_trellis_token_id_matches_blank_id_error() {
        let config = TrellisConfig {
            blank_id: 0,
            ..Default::default()
        };
        let tokens = vec![TargetToken {
            id: 0, // matches blank_id
            text: "<blank>".to_string(),
            word_index: 0,
        }];
        let emissions = vec![0.0f32; 5 * 2];
        let err = ctc_trellis_align(&emissions, 5, 2, &tokens, &config).unwrap_err();
        assert_eq!(err.kind, AlignerPortErrorKind::InvalidRequest);
    }

    #[test]
    fn test_trellis_unreachable_alignment_error() {
        let config = TrellisConfig::default();
        let tokens = vec![TargetToken {
            id: 1,
            text: "a".to_string(),
            word_index: 0,
        }];
        // All emissions are -infinity
        let emissions = vec![f32::NEG_INFINITY; 5 * 2];
        let err = ctc_trellis_align(&emissions, 5, 2, &tokens, &config).unwrap_err();
        assert_eq!(err.kind, AlignerPortErrorKind::AlignmentFailed);
    }

    #[test]
    fn test_mms_dictionary_standard_line_format() {
        let lines = vec!["<pad>", "a", "b", "c", "*"];
        let dict = MmsDictionary::from_lines(lines);
        assert_eq!(dict.len(), 5);
        assert_eq!(dict.blank_id(), 0);
        assert_eq!(dict.star_id(), Some(4));
        assert_eq!(dict.get_token_id("a"), Some(1));
        assert_eq!(dict.get_token_id("b"), Some(2));
        assert_eq!(dict.get_token_id("c"), Some(3));
    }

    #[test]
    fn test_tokenize_text_word_index_continuity_mixed_cjk_punctuation() {
        let lines = vec![
            "<pad> 100",
            "' 50",
            "h 50",
            "e 50",
            "l 50",
            "o 50",
            "w 50",
            "r 50",
            "d 50",
            "i 50",
            "t 50",
            "s 50",
            "你 10",
            "好 10",
            "世 10",
            "界 10",
        ];
        let dict = MmsDictionary::from_lines(lines);

        // Test 1: smart curly quotes "it’s" should normalize to "it's"
        let tokens_smart_quote = dict.tokenize_text("it’s");
        assert_eq!(tokens_smart_quote.len(), 4);
        assert_eq!(tokens_smart_quote[2].text, "'");
        for t in &tokens_smart_quote {
            assert_eq!(t.word_index, 0);
        }

        // Test 2: Unspaced comma "hello,world" should split into two words (word_index 0 and 1)
        let tokens_unspaced = dict.tokenize_text("hello,world");
        assert_eq!(tokens_unspaced[0].text, "h");
        assert_eq!(tokens_unspaced[0].word_index, 0);
        assert_eq!(tokens_unspaced[4].text, "o");
        assert_eq!(tokens_unspaced[4].word_index, 0);
        assert_eq!(tokens_unspaced[5].text, "w");
        assert_eq!(tokens_unspaced[5].word_index, 1);
        assert_eq!(tokens_unspaced[9].text, "d");
        assert_eq!(tokens_unspaced[9].word_index, 1);

        // Test 3: Mixed CJK and punctuation "你好，世界！"
        let tokens_cjk = dict.tokenize_text("你好，世界！");
        assert_eq!(tokens_cjk.len(), 4);
        assert_eq!(tokens_cjk[0].text, "你");
        assert_eq!(tokens_cjk[0].word_index, 0);
        assert_eq!(tokens_cjk[1].text, "好");
        assert_eq!(tokens_cjk[1].word_index, 1);
        assert_eq!(tokens_cjk[2].text, "世");
        assert_eq!(tokens_cjk[2].word_index, 2);
        assert_eq!(tokens_cjk[3].text, "界");
        assert_eq!(tokens_cjk[3].word_index, 3);
    }

    #[test]
    fn test_slice_audio_with_padding() {
        let sample_rate = 1000; // 1 sample = 1ms for simplicity
        let audio: Vec<f32> = (0..10_000).map(|v| v as f32).collect(); // 10s audio

        // Slice from 2.0s to 3.0s with 0.2s padding -> 1.8s to 3.2s
        let (slice, start_sec) = slice_audio_with_padding(&audio, sample_rate, 2.0, 3.0, 0.2);
        assert_eq!(slice.len(), 1400); // 1.4s * 1000 = 1400 samples
        assert!((start_sec - 1.8).abs() < 1e-4);
        assert_eq!(slice[0], 1800.0);
        assert_eq!(slice[slice.len() - 1], 3199.0);

        // Clamping to start: from 0.1s with 0.5s padding -> clamped to 0.0s
        let (slice_clamp, start_sec_clamp) =
            slice_audio_with_padding(&audio, sample_rate, 0.1, 1.0, 0.5);
        assert!((start_sec_clamp - 0.0).abs() < 1e-4);
        assert_eq!(slice_clamp.len(), 1500); // 0.0 to 1.5s
        assert_eq!(slice_clamp[0], 0.0);
    }

    #[test]
    fn test_fallback_timing_synthesis() {
        let text = "Hello world 你好";
        let units = synthesize_fallback_timing_units(text, 1.0, 3.0);
        assert_eq!(units.len(), 4); // "Hello", "world", "你", "好"
        assert_eq!(units[0].text, "Hello");
        assert!((units[0].start - 1.0).abs() < 1e-4);
        assert_eq!(units[3].text, "好");
        assert!((units[3].end - 3.0).abs() < 1e-4);

        let mut segment = TranscriptSegment {
            id: "seg-fb".to_string(),
            text: "Hello world".to_string(),
            start: 1.0,
            end: 2.0,
            is_final: true,
            timing: None,
            tokens: None,
            timestamps: None,
            durations: None,
            translation: None,
            speaker: None,
            speaker_attribution: None,
        };
        apply_fallback_timing_to_transcript_segment(&mut segment);
        let timing = segment.timing.expect("timing should be populated");
        assert_eq!(timing.level, TranscriptTimingLevel::Token);
        assert_eq!(timing.source, TranscriptTimingSource::Derived);
        assert_eq!(timing.units.len(), 2);
        assert_eq!(timing.units[0].text, "Hello");
        assert_eq!(timing.units[1].text, "world");
    }
}
