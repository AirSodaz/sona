pub fn format_cjk_typography(text: &str) -> String {
    if text.is_empty() {
        return text.to_string();
    }
    let chars = text.chars().collect::<Vec<_>>();
    let mut spaced = Vec::with_capacity(chars.len() + 8);
    for (index, current) in chars.iter().copied().enumerate() {
        if index > 0 {
            let previous = chars[index - 1];
            let needs_space = (is_cjk(previous) && is_latin_or_number(current))
                || (is_latin_or_number(previous) && is_cjk(current));
            if needs_space && !previous.is_whitespace() && !current.is_whitespace() {
                spaced.push(' ');
            }
        }
        let previous = spaced.iter().rev().copied().find(|ch| !ch.is_whitespace());
        let next = chars.get(index + 1).copied();
        let converted = if previous.is_some_and(is_chinese_or_japanese) {
            match current {
                ',' => '，',
                '?' => '？',
                '!' => '！',
                ':' if !next.is_some_and(|ch| ch.is_ascii_digit() || ch == '/') => '：',
                '.' if next.is_none_or(char::is_whitespace) => '。',
                other => other,
            }
        } else {
            current
        };
        spaced.push(converted);
    }

    let mut cleaned = Vec::with_capacity(spaced.len());
    for current in spaced {
        if is_cjk_closing_punctuation(current) {
            while cleaned.last().is_some_and(|ch: &char| ch.is_whitespace()) {
                cleaned.pop();
            }
        }
        if is_cjk_opening_punctuation(cleaned.last().copied().unwrap_or('\0'))
            && current.is_whitespace()
        {
            continue;
        }
        if current.is_whitespace()
            && cleaned
                .last()
                .is_some_and(|ch| is_cjk_closing_punctuation(*ch))
        {
            continue;
        }
        if is_cjk_duplicate_punctuation(current) && cleaned.last() == Some(&current) {
            continue;
        }
        cleaned.push(current);
    }
    cleaned.into_iter().collect::<String>().trim().to_string()
}

fn is_cjk(ch: char) -> bool {
    is_chinese_or_japanese(ch)
        || matches!(ch as u32, 0xAC00..=0xD7AF | 0x1100..=0x11FF | 0x3130..=0x318F)
}

fn is_chinese_or_japanese(ch: char) -> bool {
    matches!(ch as u32, 0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FA5 | 0xF900..=0xFAFF | 0xFF66..=0xFF9F)
}

fn is_latin_or_number(ch: char) -> bool {
    ch.is_ascii_alphanumeric()
}

fn is_cjk_closing_punctuation(ch: char) -> bool {
    matches!(
        ch,
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

fn is_cjk_opening_punctuation(ch: char) -> bool {
    matches!(ch, '（' | '【' | '《' | '「' | '『' | '“' | '‘')
}

fn is_cjk_duplicate_punctuation(ch: char) -> bool {
    matches!(ch, '，' | '。' | '！' | '？' | '；' | '：' | '、')
}

#[cfg(test)]
mod tests {
    use super::format_cjk_typography;

    #[test]
    fn formats_mixed_cjk_text_and_punctuation() {
        assert_eq!(format_cjk_typography("识别到PR请求"), "识别到 PR 请求");
        assert_eq!(format_cjk_typography("第1阶段完成"), "第 1 阶段完成");
        assert_eq!(format_cjk_typography("hello世界"), "hello 世界");
        assert_eq!(format_cjk_typography("测试 ,完成."), "测试，完成。");
        assert_eq!(format_cjk_typography("时间12:30"), "时间 12:30");
    }
}
