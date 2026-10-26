//! Canonical CJK character, punctuation, and boundary utilities.

/// Returns true if character is in one of the CJK Unicode blocks:
/// - CJK Unified Ideographs (`\u{4E00}..=\u{9FFF}`)
/// - CJK Extension A (`\u{3400}..=\u{4DBF}`)
/// - CJK Compatibility Ideographs (`\u{F900}..=\u{FAFF}`)
/// - CJK Extensions B-I (`\u{20000}..=\u{2FA1F}`, `\u{30000}..=\u{323AF}`)
/// - Hiragana (`\u{3040}..=\u{309F}`)
/// - Katakana (`\u{30A0}..=\u{30FF}`)
/// - Bopomofo (`\u{3100}..=\u{312F}`, `\u{31A0}..=\u{31BF}`)
/// - Hangul Syllables (`\u{AC00}..=\u{D7AF}`)
/// - Hangul Jamo (`\u{1100}..=\u{11FF}`, `\u{3130}..=\u{318F}`)
pub fn is_cjk_char(c: char) -> bool {
    matches!(
        c,
        '\u{4E00}'..='\u{9FFF}'
            | '\u{3400}'..='\u{4DBF}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{20000}'..='\u{2FA1F}'
            | '\u{30000}'..='\u{323AF}'
            | '\u{3040}'..='\u{309F}'
            | '\u{30A0}'..='\u{30FF}'
            | '\u{3100}'..='\u{312F}'
            | '\u{31A0}'..='\u{31BF}'
            | '\u{AC00}'..='\u{D7AF}'
            | '\u{1100}'..='\u{11FF}'
            | '\u{3130}'..='\u{318F}'
    )
}

/// Returns true if character is a CJK punctuation mark or fullwidth ASCII variant:
/// - CJK Symbols and Punctuation (`\u{3000}..=\u{303F}`)
/// - Fullwidth ASCII variants (`\u{FF01}..=\u{FF0F}`, `\u{FF1A}..=\u{FF20}`, `\u{FF3B}..=\u{FF40}`, `\u{FF5B}..=\u{FF65}`)
pub fn is_cjk_punctuation(c: char) -> bool {
    matches!(
        c,
        '\u{3000}'..='\u{303F}'
            | '\u{FF01}'..='\u{FF0F}'
            | '\u{FF1A}'..='\u{FF20}'
            | '\u{FF3B}'..='\u{FF40}'
            | '\u{FF5B}'..='\u{FF65}'
    )
}

/// Returns true if character is a Chinese, Japanese (Kanji/Kana), or Bopomofo character
/// where word boundaries occur per-character without whitespace delimiter requirements.
pub fn is_cjk_ideograph_or_kana(c: char) -> bool {
    matches!(
        c,
        '\u{4E00}'..='\u{9FFF}'
            | '\u{3400}'..='\u{4DBF}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{20000}'..='\u{2FA1F}'
            | '\u{30000}'..='\u{323AF}'
            | '\u{3040}'..='\u{309F}'
            | '\u{30A0}'..='\u{30FF}'
            | '\u{3100}'..='\u{312F}'
            | '\u{31A0}'..='\u{31BF}'
    )
}

/// Returns true if character is a word or segment boundary character:
/// whitespace, ASCII punctuation, typographic quotes/symbols, CJK punctuation, or
/// CJK ideographs/kana (excluding Hangul, which uses whitespace word boundaries).
pub fn is_boundary_char(c: char) -> bool {
    c.is_whitespace()
        || c.is_ascii_punctuation()
        || matches!(
            c,
            '\u{2000}'
                ..='\u{206F}' | '\u{00AB}' | '\u{00BB}' | '\u{00BF}' | '\u{00A1}' | '\u{00B7}'
        )
        || is_cjk_ideograph_or_kana(c)
        || is_cjk_punctuation(c)
}

/// Returns false if either adjacent boundary character is a CJK character,
/// CJK punctuation, or whitespace. Returns true only between Latin/numeric word
/// boundaries lacking whitespace.
pub fn needs_word_boundary_space(left: &str, right: &str) -> bool {
    let Some(last) = left.chars().last() else {
        return false;
    };
    let Some(first) = right.chars().next() else {
        return false;
    };
    if last.is_whitespace() || first.is_whitespace() {
        return false;
    }
    if is_cjk_char(last)
        || is_cjk_punctuation(last)
        || is_cjk_char(first)
        || is_cjk_punctuation(first)
    {
        return false;
    }
    true
}

/// Combines two strings inserting a space only when `needs_word_boundary_space` is true.
pub fn join_text_with_boundary_space(left: &str, right: &str) -> String {
    let left = left.trim();
    let right = right.trim();
    if left.is_empty() {
        return right.to_string();
    }
    if right.is_empty() {
        return left.to_string();
    }
    if needs_word_boundary_space(left, right) {
        format!("{left} {right}")
    } else {
        format!("{left}{right}")
    }
}

/// Joins an array/slice of segment strings using CJK-aware boundary spacing
/// instead of naive `.join(" ")`.
pub fn join_segments_with_boundary_space<S: AsRef<str>>(segments: &[S]) -> String {
    let mut iter = segments
        .iter()
        .map(|s| s.as_ref().trim())
        .filter(|s| !s.is_empty());
    let Some(first) = iter.next() else {
        return String::new();
    };
    let mut acc = first.to_string();
    for seg in iter {
        acc = join_text_with_boundary_space(&acc, seg);
    }
    acc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cjk_character_ranges() {
        // CJK Unified Ideographs
        assert!(is_cjk_char('中'));
        assert!(is_cjk_char('\u{4E00}'));
        assert!(is_cjk_char('\u{9FFF}'));

        // CJK Extension A
        assert!(is_cjk_char('\u{3400}'));
        assert!(is_cjk_char('\u{4DBF}'));

        // CJK Compatibility Ideographs
        assert!(is_cjk_char('\u{F900}'));
        assert!(is_cjk_char('\u{FAFF}'));

        // CJK Extension B-I
        assert!(is_cjk_char('\u{20000}'));
        assert!(is_cjk_char('\u{2FA1F}'));
        assert!(is_cjk_char('\u{30000}'));
        assert!(is_cjk_char('\u{323AF}'));

        // Hiragana & Katakana
        assert!(is_cjk_char('あ'));
        assert!(is_cjk_char('ア'));

        // Bopomofo
        assert!(is_cjk_char('ㄅ'));
        assert!(is_cjk_char('\u{31A0}'));

        // Hangul Syllables and Jamo
        assert!(is_cjk_char('한'));
        assert!(is_cjk_char('글'));
        assert!(is_cjk_char('\u{AC00}'));
        assert!(is_cjk_char('\u{D7AF}'));
        assert!(is_cjk_char('\u{1100}'));
        assert!(is_cjk_char('\u{3130}'));

        // Non-CJK
        assert!(!is_cjk_char('a'));
        assert!(!is_cjk_char('Z'));
        assert!(!is_cjk_char('9'));
        assert!(!is_cjk_char(' '));
        assert!(!is_cjk_char(','));
    }

    #[test]
    fn test_cjk_punctuation() {
        assert!(is_cjk_punctuation('。'));
        assert!(is_cjk_punctuation('，'));
        assert!(is_cjk_punctuation('、'));
        assert!(is_cjk_punctuation('！'));
        assert!(is_cjk_punctuation('？'));
        assert!(is_cjk_punctuation('：'));

        assert!(!is_cjk_punctuation('.'));
        assert!(!is_cjk_punctuation(','));
        assert!(!is_cjk_punctuation('!'));
        assert!(!is_cjk_punctuation('?'));
    }

    #[test]
    fn test_needs_word_boundary_space() {
        assert!(needs_word_boundary_space("Hello", "world"));
        assert!(needs_word_boundary_space("Step", "1"));

        // CJK to CJK
        assert!(!needs_word_boundary_space("你好", "世界"));

        // CJK to Latin & Latin to CJK
        assert!(!needs_word_boundary_space("你好", "Sona"));
        assert!(!needs_word_boundary_space("Sona", "你好"));

        // CJK punctuation
        assert!(!needs_word_boundary_space("你好", "，"));
        assert!(!needs_word_boundary_space("！", "你好"));

        // Whitespace
        assert!(!needs_word_boundary_space("Hello ", "world"));
        assert!(!needs_word_boundary_space("Hello", " world"));
    }

    #[test]
    fn test_join_text_with_boundary_space() {
        assert_eq!(
            join_text_with_boundary_space("Hello", "world"),
            "Hello world"
        );
        assert_eq!(join_text_with_boundary_space("你好", "世界"), "你好世界");
        assert_eq!(join_text_with_boundary_space("你好", "Sona"), "你好Sona");
        assert_eq!(
            join_text_with_boundary_space("  Hello  ", "  world  "),
            "Hello world"
        );
        assert_eq!(join_text_with_boundary_space("你好", ""), "你好");
        assert_eq!(join_text_with_boundary_space("", "世界"), "世界");
    }

    #[test]
    fn test_join_segments_with_boundary_space() {
        let latin_segments = vec!["Hello", "world", "from", "Rust"];
        assert_eq!(
            join_segments_with_boundary_space(&latin_segments),
            "Hello world from Rust"
        );

        let cjk_segments = vec!["你好", "世界", "，", "欢迎"];
        assert_eq!(
            join_segments_with_boundary_space(&cjk_segments),
            "你好世界，欢迎"
        );

        let mixed_segments = vec!["Hello", "world", "你好", "世界"];
        assert_eq!(
            join_segments_with_boundary_space(&mixed_segments),
            "Hello world你好世界"
        );

        let empty: Vec<&str> = vec![];
        assert_eq!(join_segments_with_boundary_space(&empty), "");

        let single = vec!["Single"];
        assert_eq!(join_segments_with_boundary_space(&single), "Single");
    }
}
