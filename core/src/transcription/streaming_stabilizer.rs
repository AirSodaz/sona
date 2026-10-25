//! Streaming translation stabilizer implementing Local Agreement prefix locking and draft tail management.

pub fn is_cjk_char(c: char) -> bool {
    matches!(c,
        '\u{4E00}'..='\u{9FFF}' | // CJK Unified Ideographs
        '\u{3400}'..='\u{4DBF}' | // CJK Extension A
        '\u{F900}'..='\u{FAFF}' | // CJK Compatibility Ideographs
        '\u{3040}'..='\u{309F}' | // Hiragana
        '\u{30A0}'..='\u{30FF}' | // Katakana
        '\u{AC00}'..='\u{D7AF}'   // Hangul Syllables
    )
}

pub fn is_cjk_punctuation(c: char) -> bool {
    matches!(c,
        '\u{3000}'..='\u{303F}' | // CJK Symbols and Punctuation (，、。 etc)
        '\u{FF01}'..='\u{FF0F}' | // Fullwidth ! " # $ % & ' ( ) * + , - . /
        '\u{FF1A}'..='\u{FF20}' | // Fullwidth : ; < = > ? @
        '\u{FF3B}'..='\u{FF40}' | // Fullwidth [ \ ] ^ _ `
        '\u{FF5B}'..='\u{FF65}'   // Fullwidth { | } ~
    )
}

pub fn is_boundary_char(c: char) -> bool {
    c.is_whitespace() || c.is_ascii_punctuation() || is_cjk_char(c) || is_cjk_punctuation(c)
}

/// Computes the longest common prefix of `prev` and `curr` truncated at word or punctuation boundaries.
pub fn calculate_longest_common_prefix(prev: &str, curr: &str) -> String {
    if prev.is_empty() || curr.is_empty() {
        return String::new();
    }

    let mut common_chars = Vec::new();
    for (cp, cc) in prev.chars().zip(curr.chars()) {
        if cp == cc {
            common_chars.push(cp);
        } else {
            break;
        }
    }

    if common_chars.is_empty() {
        return String::new();
    }

    let raw_lcp: String = common_chars.into_iter().collect();

    // If completely identical to both, return raw_lcp
    if raw_lcp == prev && raw_lcp == curr {
        return raw_lcp;
    }

    let lcp_len = raw_lcp.len();
    let next_curr_char = curr[lcp_len..].chars().next();
    let next_prev_char = prev[lcp_len..].chars().next();

    // If prev ended at a word boundary and curr continues with whitespace,
    // include the whitespace in the boundary prefix.
    if raw_lcp == prev && next_curr_char.is_some_and(|c| c.is_whitespace()) {
        let ws = next_curr_char.unwrap();
        let mut s = raw_lcp;
        s.push(ws);
        return s;
    }

    // If curr ended at a word boundary and prev continues with whitespace
    if raw_lcp == curr && next_prev_char.is_some_and(|c| c.is_whitespace()) {
        let ws = next_prev_char.unwrap();
        let mut s = raw_lcp;
        s.push(ws);
        return s;
    }

    // Check if raw_lcp already ends at a boundary character
    let last_char = raw_lcp.chars().last().unwrap();
    if is_boundary_char(last_char) {
        return raw_lcp;
    }

    // If the next character in curr is a boundary char
    if let Some(nc) = next_curr_char.filter(|&nc| is_boundary_char(nc)) {
        if nc.is_whitespace() {
            let mut s = raw_lcp;
            s.push(nc);
            return s;
        }
        return raw_lcp;
    }

    // Otherwise, raw_lcp cut in the middle of a token.
    // Scan backwards in raw_lcp for the latest boundary char.
    let mut boundary_byte_end = None;
    for (byte_idx, ch) in raw_lcp.char_indices() {
        if is_boundary_char(ch) {
            boundary_byte_end = Some(byte_idx + ch.len_utf8());
        }
    }

    if let Some(end) = boundary_byte_end {
        raw_lcp[..end].to_string()
    } else {
        String::new()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct StabilizerOutput {
    pub frozen_prefix: Option<String>,
    pub draft_tail: Option<String>,
}

impl StabilizerOutput {
    pub fn translation(&self) -> Option<&str> {
        self.frozen_prefix.as_deref()
    }

    pub fn draft_translation(&self) -> Option<&str> {
        self.draft_tail.as_deref()
    }
}

#[derive(Clone, Debug, Default)]
pub struct StreamingStabilizer {
    frozen_prefix: String,
    last_hypothesis: String,
}

impl StreamingStabilizer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.frozen_prefix.clear();
        self.last_hypothesis.clear();
    }

    pub fn frozen_prefix(&self) -> &str {
        &self.frozen_prefix
    }

    pub fn update(&mut self, hypothesis: &str, is_final: bool) -> StabilizerOutput {
        if is_final {
            let final_text = hypothesis.trim().to_string();
            self.frozen_prefix = final_text.clone();
            self.last_hypothesis.clear();
            return StabilizerOutput {
                frozen_prefix: if final_text.is_empty() {
                    None
                } else {
                    Some(final_text)
                },
                draft_tail: None,
            };
        }

        let trimmed_hypothesis = hypothesis.trim();
        if trimmed_hypothesis.is_empty() {
            return StabilizerOutput {
                frozen_prefix: if self.frozen_prefix.is_empty() {
                    None
                } else {
                    Some(self.frozen_prefix.clone())
                },
                draft_tail: None,
            };
        }

        if !self.last_hypothesis.is_empty() {
            let candidate_lcp =
                calculate_longest_common_prefix(&self.last_hypothesis, trimmed_hypothesis);
            if candidate_lcp.len() > self.frozen_prefix.len()
                && candidate_lcp.starts_with(&self.frozen_prefix)
            {
                self.frozen_prefix = candidate_lcp;
            }
        }

        self.last_hypothesis = trimmed_hypothesis.to_string();

        let draft_tail = if trimmed_hypothesis.starts_with(&self.frozen_prefix) {
            let tail = &trimmed_hypothesis[self.frozen_prefix.len()..];
            let trimmed_tail = tail.trim_start();
            if trimmed_tail.is_empty() {
                None
            } else {
                Some(trimmed_tail.to_string())
            }
        } else {
            Some(trimmed_hypothesis.to_string())
        };

        StabilizerOutput {
            frozen_prefix: if self.frozen_prefix.is_empty() {
                None
            } else {
                Some(self.frozen_prefix.clone())
            },
            draft_tail,
        }
    }
}
