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

fn is_cj_typography_char(c: char) -> bool {
    is_cjk_ideograph_or_kana(c) || matches!(c, '\u{FF66}'..='\u{FF9F}')
}

fn is_cjk_typography_pangu(c: char) -> bool {
    is_cjk_char(c) || matches!(c, '\u{FF66}'..='\u{FF9F}')
}

fn is_closing_or_middle_cjk_punct(c: char) -> bool {
    matches!(
        c,
        '，' | '。'
            | '！'
            | '？'
            | '；'
            | '：'
            | '、'
            | '）'
            | '】'
            | '》'
            | '」'
            | '』'
            | '”'
            | '’'
    )
}

fn is_opening_cjk_punct(c: char) -> bool {
    matches!(c, '（' | '【' | '《' | '「' | '『' | '“' | '‘')
}

fn is_dedup_cjk_punct(c: char) -> bool {
    matches!(c, '，' | '。' | '！' | '？' | '；' | '：' | '、')
}

fn insert_pangu_spaces(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut prev: Option<char> = None;
    for ch in text.chars() {
        if let Some(p) = prev {
            let p_cjk = is_cjk_typography_pangu(p);
            let p_latin = p.is_ascii_alphanumeric();
            let c_cjk = is_cjk_typography_pangu(ch);
            let c_latin = ch.is_ascii_alphanumeric();
            if (p_cjk && c_latin) || (p_latin && c_cjk) {
                out.push(' ');
            }
        }
        out.push(ch);
        prev = Some(ch);
    }
    out
}

fn normalize_cj_ascii_punctuation(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (i, &ch) in chars.iter().enumerate() {
        let prev_cj = i > 0 && is_cj_typography_char(chars[i - 1]);
        if prev_cj {
            let next = chars.get(i + 1).copied();
            match ch {
                ',' => out.push('，'),
                '?' => out.push('？'),
                '!' => out.push('！'),
                ':' if !matches!(next, Some('0'..='9' | '/')) => out.push('：'),
                '.' if next.is_none() || next.is_some_and(char::is_whitespace) => out.push('。'),
                _ => out.push(ch),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn clean_cjk_punctuation_spacing_and_duplicates(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    // Pass 5a: remove whitespace before closing / middle CJK punctuation
    let mut after_before = Vec::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_whitespace() {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if j < chars.len() && is_closing_or_middle_cjk_punct(chars[j]) {
                i = j;
                continue;
            }
        }
        after_before.push(chars[i]);
        i += 1;
    }

    // Pass 5b: remove whitespace after opening CJK punctuation
    let mut after_open = Vec::with_capacity(after_before.len());
    let mut k = 0;
    while k < after_before.len() {
        let ch = after_before[k];
        after_open.push(ch);
        if is_opening_cjk_punct(ch) {
            while k + 1 < after_before.len() && after_before[k + 1].is_whitespace() {
                k += 1;
            }
        }
        k += 1;
    }

    // Pass 5c: remove whitespace between closing/middle CJK punctuation and CJK char
    let mut after_close = Vec::with_capacity(after_open.len());
    let mut m = 0;
    while m < after_open.len() {
        let ch = after_open[m];
        after_close.push(ch);
        if is_closing_or_middle_cjk_punct(ch) {
            let mut n = m + 1;
            while n < after_open.len() && after_open[n].is_whitespace() {
                n += 1;
            }
            if n > m + 1 && n < after_open.len() && is_cjk_typography_pangu(after_open[n]) {
                m = n;
                continue;
            }
        }
        m += 1;
    }

    // Pass 6: deduplicate consecutive Chinese punctuations
    let mut out = String::with_capacity(text.len());
    let mut prev: Option<char> = None;
    for ch in after_close {
        if is_dedup_cjk_punct(ch) && prev == Some(ch) {
            continue;
        }
        out.push(ch);
        prev = Some(ch);
    }
    out
}

/// Formats mixed CJK and Latin text with proper Pangu spacing, punctuation harmony,
/// and duplicate Chinese punctuation cleanup (matching frontend `formatCjkTypography`).
pub fn format_cjk_typography(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let with_pangu = insert_pangu_spaces(text);
    let with_punct = normalize_cj_ascii_punctuation(&with_pangu);
    clean_cjk_punctuation_spacing_and_duplicates(&with_punct)
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

    #[test]
    fn test_format_cjk_typography_matches_frontend_semantics() {
        assert_eq!(format_cjk_typography(""), "");
        assert_eq!(format_cjk_typography("   "), "   ");

        // Pangu spacing: CJK <-> Latin
        assert_eq!(format_cjk_typography("识别到PR请求"), "识别到 PR 请求");
        assert_eq!(format_cjk_typography("完成TODO事项"), "完成 TODO 事项");
        assert_eq!(format_cjk_typography("hello世界"), "hello 世界");
        assert_eq!(format_cjk_typography("世界hello"), "世界 hello");

        // Pangu spacing: CJK <-> Numbers
        assert_eq!(format_cjk_typography("第1阶段完成"), "第 1 阶段完成");
        assert_eq!(format_cjk_typography("有99个人"), "有 99 个人");
        assert_eq!(
            format_cjk_typography("今天是2026年9月19日"),
            "今天是 2026 年 9 月 19 日"
        );

        // Full-width punctuation adjacent to CJ
        assert_eq!(format_cjk_typography("你好,我是Sona"), "你好，我是 Sona");
        assert_eq!(format_cjk_typography("你好,我是世界."), "你好，我是世界。");
        assert_eq!(format_cjk_typography("真的吗?当然!"), "真的吗？当然！");
        assert_eq!(format_cjk_typography("请注意:重要通知"), "请注意：重要通知");

        // Punctuation space cleanup
        assert_eq!(format_cjk_typography("你好 ， 很好 。"), "你好，很好。");
        assert_eq!(format_cjk_typography("【 重点 】 内容"), "【重点】内容");

        // Deduplicate Chinese punctuation
        assert_eq!(
            format_cjk_typography("好的。。没问题！！"),
            "好的。没问题！"
        );
        assert_eq!(format_cjk_typography("你好，，世界"), "你好，世界");

        // Preserve decimals, times, URLs
        assert_eq!(
            format_cjk_typography("圆周率是3.14左右"),
            "圆周率是 3.14 左右"
        );
        assert_eq!(format_cjk_typography("时间是12:30分"), "时间是 12:30 分");
        assert_eq!(
            format_cjk_typography("访问https://sona.ai即可"),
            "访问 https://sona.ai 即可"
        );

        // Preserve Hangul punctuation while applying Pangu spacing
        assert_eq!(format_cjk_typography("테스트1완료"), "테스트 1 완료");
        assert_eq!(format_cjk_typography("1단계"), "1 단계");
        assert_eq!(
            format_cjk_typography("한국어English혼합"),
            "한국어 English 혼합"
        );
        assert_eq!(
            format_cjk_typography("안녕하세요. 오늘 1단계입니다."),
            "안녕하세요. 오늘 1 단계입니다."
        );
    }
}
