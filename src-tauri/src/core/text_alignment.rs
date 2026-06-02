#[derive(Clone, Debug)]
pub(crate) struct TextUnit {
    pub(crate) text: String,
    pub(crate) normalized: String,
}

#[derive(Clone, Debug)]
pub(crate) struct AlignedTextUnit {
    pub(crate) text: String,
    pub(crate) token_index: usize,
}

pub(crate) fn align_text_units_to_tokens(
    text: &str,
    tokens: &[String],
) -> Option<Vec<AlignedTextUnit>> {
    if tokens.is_empty() {
        return None;
    }

    let normalized_tokens = tokens
        .iter()
        .map(|token| normalize_search_text(token))
        .collect::<Vec<_>>();
    let units = lex_text_units(text);

    let mut joined_token_chars = Vec::new();
    let mut char_to_token_index = Vec::new();
    for (token_index, token) in normalized_tokens.iter().enumerate() {
        for ch in token.chars() {
            joined_token_chars.push(ch);
            char_to_token_index.push(token_index);
        }
    }

    if char_to_token_index.is_empty() {
        return None;
    }

    let mut char_pos = 0usize;
    let mut result = Vec::new();

    for unit in units {
        if unit.text.is_empty() {
            continue;
        }

        let token_index = if unit.normalized.is_empty() {
            fallback_token_index(char_pos, &char_to_token_index)
        } else {
            let needle = unit.normalized.chars().collect::<Vec<_>>();
            let search_limit = needle.len().saturating_mul(2).max(20);
            let window_end = (char_pos + search_limit).min(joined_token_chars.len());
            let local_index = find_subsequence(&joined_token_chars[char_pos..window_end], &needle);

            if let Some(local_index) = local_index {
                let match_pos = char_pos + local_index;
                char_pos = (match_pos + needle.len()).min(joined_token_chars.len());
                fallback_token_index(match_pos, &char_to_token_index)
            } else {
                let fallback = fallback_token_index(char_pos, &char_to_token_index);
                char_pos = (char_pos + needle.len().max(1)).min(joined_token_chars.len());
                fallback
            }
        };

        result.push(AlignedTextUnit {
            text: unit.text,
            token_index,
        });
    }

    Some(result)
}

fn fallback_token_index(char_pos: usize, char_to_token_index: &[usize]) -> usize {
    if char_to_token_index.is_empty() {
        return 0;
    }

    if char_pos >= char_to_token_index.len() {
        char_to_token_index[char_to_token_index.len() - 1]
    } else {
        char_to_token_index[char_pos]
    }
}

fn find_subsequence(haystack: &[char], needle: &[char]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }

    if needle.len() > haystack.len() {
        return None;
    }

    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

pub(crate) fn lex_text_units(text: &str) -> Vec<TextUnit> {
    let mut units = Vec::new();
    let chars = text.chars().collect::<Vec<_>>();
    let mut index = 0usize;

    while index < chars.len() {
        let ch = chars[index];

        if ch.is_whitespace() {
            let start = index;
            index += 1;
            while index < chars.len() && chars[index].is_whitespace() {
                index += 1;
            }
            units.push(TextUnit {
                text: chars[start..index].iter().collect(),
                normalized: String::new(),
            });
            continue;
        }

        if is_cjk_char(ch) {
            let text = ch.to_string();
            units.push(TextUnit {
                normalized: normalize_search_text(&text),
                text,
            });
            index += 1;
            continue;
        }

        let start = index;
        index += 1;
        while index < chars.len() && !chars[index].is_whitespace() && !is_cjk_char(chars[index]) {
            index += 1;
        }

        let text = chars[start..index].iter().collect::<String>();
        let normalized = normalize_search_text(&text);

        if normalized.is_empty() {
            if let Some(previous) = units.last_mut() {
                if !previous.normalized.is_empty() {
                    previous.text.push_str(&text);
                    continue;
                }
            }
        }

        units.push(TextUnit { text, normalized });
    }

    units
}

fn normalize_search_text(text: &str) -> String {
    text.chars()
        .flat_map(|ch| ch.to_lowercase())
        .filter(|ch| ch.is_alphanumeric())
        .collect()
}

pub(crate) fn is_cjk_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{3400}'..='\u{4DBF}'
            | '\u{4E00}'..='\u{9FFF}'
            | '\u{3040}'..='\u{309F}'
            | '\u{30A0}'..='\u{30FF}'
            | '\u{AC00}'..='\u{D7AF}'
    )
}
