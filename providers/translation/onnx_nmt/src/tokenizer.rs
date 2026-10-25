//! Pure Rust SentencePiece protobuf reader and Unigram tokenizer for MarianMT / Opus-MT models.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug, PartialEq)]
pub struct PieceEntry {
    pub piece: String,
    pub score: f32,
    pub piece_type: u32,
}

#[derive(Clone, Debug, Default)]
pub struct SentencePieceTokenizer {
    pub pieces: Vec<PieceEntry>,
    pub piece_to_id: HashMap<String, u32>,
    pub pad_id: Option<u32>,
    pub eos_id: u32,
    pub unk_id: u32,
    max_piece_len: usize,
}

impl SentencePieceTokenizer {
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes =
            fs::read(path).map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
        Self::from_bytes(&bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let mut idx = 0;
        let mut pieces = Vec::new();
        let mut piece_to_id = HashMap::new();
        let mut max_piece_len = 0;

        while idx < bytes.len() {
            let (tag, new_idx) = read_varint(bytes, idx)?;
            idx = new_idx;
            let wire_type = tag & 0x7;
            let field_num = tag >> 3;

            if field_num == 1 && wire_type == 2 {
                let (len, new_idx) = read_varint(bytes, idx)?;
                idx = new_idx;
                let sub_end = idx + len as usize;
                if sub_end > bytes.len() {
                    return Err("Unexpected EOF reading SentencePiece entry".into());
                }
                let piece_bytes = &bytes[idx..sub_end];
                idx = sub_end;

                let entry = parse_sentencepiece_entry(piece_bytes)?;
                let token_id = pieces.len() as u32;
                max_piece_len = max_piece_len.max(entry.piece.chars().count());
                piece_to_id.insert(entry.piece.clone(), token_id);
                pieces.push(entry);
            } else {
                idx = skip_field(bytes, idx, wire_type)?;
            }
        }

        let eos_id = piece_to_id.get("</s>").copied().unwrap_or(0);
        let unk_id = piece_to_id.get("<unk>").copied().unwrap_or(1);
        let pad_id = piece_to_id.get("<pad>").copied();

        Ok(Self {
            pieces,
            piece_to_id,
            pad_id,
            eos_id,
            unk_id,
            max_piece_len: max_piece_len.max(1),
        })
    }

    pub fn encode(&self, text: &str) -> Vec<i64> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return vec![self.eos_id as i64];
        }

        let mut normalized = String::new();
        for ch in trimmed.chars() {
            if ch.is_whitespace() {
                if !normalized.ends_with('\u{2581}') {
                    normalized.push('\u{2581}');
                }
            } else {
                if normalized.is_empty() {
                    normalized.push('\u{2581}');
                }
                normalized.push(ch);
            }
        }

        let chars: Vec<char> = normalized.chars().collect();
        let n = chars.len();
        if n == 0 {
            return vec![self.eos_id as i64];
        }

        let mut best_score = vec![f32::NEG_INFINITY; n + 1];
        let mut best_prev = vec![(0usize, self.unk_id); n + 1];
        best_score[0] = 0.0;

        for i in 0..n {
            if best_score[i] == f32::NEG_INFINITY {
                continue;
            }

            let max_l = self.max_piece_len.min(n - i);
            let mut matched = false;

            for len in 1..=max_l {
                let sub: String = chars[i..i + len].iter().collect();
                if let Some(&token_id) = self.piece_to_id.get(&sub) {
                    matched = true;
                    let score = self.pieces[token_id as usize].score;
                    let candidate = best_score[i] + score;
                    if candidate > best_score[i + len] {
                        best_score[i + len] = candidate;
                        best_prev[i + len] = (i, token_id);
                    }
                }
            }

            if !matched && best_score[i + 1] == f32::NEG_INFINITY {
                best_score[i + 1] = best_score[i] - 100.0;
                best_prev[i + 1] = (i, self.unk_id);
            }
        }

        let mut tokens = Vec::new();
        let mut curr = n;
        while curr > 0 {
            let (prev, token_id) = best_prev[curr];
            tokens.push(token_id as i64);
            curr = prev;
        }
        tokens.reverse();
        tokens.push(self.eos_id as i64);
        tokens
    }

    pub fn decode(&self, token_ids: &[i64]) -> String {
        let mut result = String::new();
        let mut byte_buf: Vec<u8> = Vec::new();

        for &id in token_ids {
            if id == self.eos_id as i64 || self.pad_id.is_some_and(|pad| id == pad as i64) {
                break;
            }
            let idx = id as usize;
            if idx >= self.pieces.len() {
                continue;
            }
            let piece = &self.pieces[idx].piece;
            if piece == "<unk>" || piece == "<s>" || piece == "</s>" || piece == "<pad>" {
                if !byte_buf.is_empty() {
                    result.push_str(&String::from_utf8_lossy(&byte_buf));
                    byte_buf.clear();
                }
                continue;
            }

            if let Some(b) = parse_hex_byte(piece) {
                byte_buf.push(b);
            } else {
                if !byte_buf.is_empty() {
                    result.push_str(&String::from_utf8_lossy(&byte_buf));
                    byte_buf.clear();
                }
                result.push_str(piece);
            }
        }

        if !byte_buf.is_empty() {
            result.push_str(&String::from_utf8_lossy(&byte_buf));
        }

        result.replace('\u{2581}', " ").trim().to_string()
    }
}

fn parse_sentencepiece_entry(bytes: &[u8]) -> Result<PieceEntry, String> {
    let mut idx = 0;
    let mut piece = String::new();
    let mut score = 0.0f32;
    let mut piece_type = 0u32;

    while idx < bytes.len() {
        let (tag, new_idx) = read_varint(bytes, idx)?;
        idx = new_idx;
        let wire_type = tag & 0x7;
        let field_num = tag >> 3;

        match (field_num, wire_type) {
            (1, 2) => {
                let (len, new_idx) = read_varint(bytes, idx)?;
                idx = new_idx;
                let end = idx + len as usize;
                if end > bytes.len() {
                    return Err("Invalid piece length".into());
                }
                piece = String::from_utf8_lossy(&bytes[idx..end]).into_owned();
                idx = end;
            }
            (2, 5) => {
                if idx + 4 > bytes.len() {
                    return Err("Invalid score bytes".into());
                }
                let score_bytes = [bytes[idx], bytes[idx + 1], bytes[idx + 2], bytes[idx + 3]];
                score = f32::from_le_bytes(score_bytes);
                idx += 4;
            }
            (3, 0) => {
                let (t, new_idx) = read_varint(bytes, idx)?;
                idx = new_idx;
                piece_type = t as u32;
            }
            _ => {
                idx = skip_field(bytes, idx, wire_type)?;
            }
        }
    }

    Ok(PieceEntry {
        piece,
        score,
        piece_type,
    })
}

fn read_varint(bytes: &[u8], mut idx: usize) -> Result<(u64, usize), String> {
    let mut result = 0u64;
    let mut shift = 0;
    while idx < bytes.len() {
        let byte = bytes[idx];
        idx += 1;
        result |= ((byte & 0x7F) as u64) << shift;
        if (byte & 0x80) == 0 {
            return Ok((result, idx));
        }
        shift += 7;
        if shift >= 64 {
            return Err("Varint overflow".into());
        }
    }
    Err("Unexpected EOF reading varint".into())
}

fn skip_field(bytes: &[u8], idx: usize, wire_type: u64) -> Result<usize, String> {
    match wire_type {
        0 => {
            let (_, new_idx) = read_varint(bytes, idx)?;
            Ok(new_idx)
        }
        1 => {
            if idx + 8 > bytes.len() {
                return Err("Unexpected EOF in 64-bit field".into());
            }
            Ok(idx + 8)
        }
        2 => {
            let (len, new_idx) = read_varint(bytes, idx)?;
            let end = new_idx + len as usize;
            if end > bytes.len() {
                return Err("Unexpected EOF in length-delimited field".into());
            }
            Ok(end)
        }
        5 => {
            if idx + 4 > bytes.len() {
                return Err("Unexpected EOF in 32-bit field".into());
            }
            Ok(idx + 4)
        }
        _ => Err(format!("Unsupported wire type: {wire_type}")),
    }
}

fn parse_hex_byte(piece: &str) -> Option<u8> {
    if piece.starts_with("<0x") && piece.ends_with('>') && piece.len() == 6 {
        u8::from_str_radix(&piece[3..5], 16).ok()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sentencepiece_synthetic_roundtrip() {
        let mut tokenizer = SentencePieceTokenizer::default();
        let pieces = vec![
            ("</s>", 0.0),
            ("<unk>", 0.0),
            ("\u{2581}Hello", -1.0),
            ("\u{2581}world", -1.0),
            ("!", -0.5),
        ];
        for (i, (p, s)) in pieces.into_iter().enumerate() {
            tokenizer.pieces.push(PieceEntry {
                piece: p.to_string(),
                score: s,
                piece_type: 0,
            });
            tokenizer.piece_to_id.insert(p.to_string(), i as u32);
        }
        tokenizer.eos_id = 0;
        tokenizer.unk_id = 1;
        tokenizer.pad_id = Some(0);
        tokenizer.max_piece_len = 10;

        let tokens = tokenizer.encode("Hello world!");
        assert_eq!(tokens, vec![2, 3, 4, 0]);

        let decoded = tokenizer.decode(&tokens);
        assert_eq!(decoded, "Hello world!");
    }

    #[test]
    fn test_sentencepiece_byte_fallback() {
        let mut tokenizer = SentencePieceTokenizer::default();
        let pieces = vec![
            ("</s>", 0.0),
            ("<unk>", 0.0),
            ("<0xE4>", -5.0),
            ("<0xBD>", -5.0),
            ("<0xA0>", -5.0),
            ("好", -1.0),
        ];
        for (i, (p, s)) in pieces.into_iter().enumerate() {
            tokenizer.pieces.push(PieceEntry {
                piece: p.to_string(),
                score: s,
                piece_type: 0,
            });
            tokenizer.piece_to_id.insert(p.to_string(), i as u32);
        }
        tokenizer.eos_id = 0;
        tokenizer.unk_id = 1;
        tokenizer.pad_id = Some(0);
        tokenizer.max_piece_len = 10;

        // <0xE4><0xBD><0xA0> is UTF-8 for "你"
        let decoded = tokenizer.decode(&[2, 3, 4, 5]);
        assert_eq!(decoded, "你好");
    }

    #[test]
    fn test_sentencepiece_protobuf_field_1_parsing() {
        // Construct a binary protobuf message matching sentencepiece_model.proto:
        // ModelProto { repeated SentencePiece pieces = 1; }
        // SentencePiece { string piece = 1; float score = 2; }
        let mut raw = Vec::new();

        // Piece 1: "</s>"
        let mut piece1 = Vec::new();
        piece1.push(0x0A); // field 1, wire 2
        piece1.push(4); // length 4
        piece1.extend_from_slice(b"</s>");

        // Outer field 1, wire 2
        raw.push(0x0A);
        raw.push(piece1.len() as u8);
        raw.extend_from_slice(&piece1);

        // Piece 2: "hello"
        let mut piece2 = Vec::new();
        piece2.push(0x0A);
        piece2.push(5);
        piece2.extend_from_slice(b"hello");

        raw.push(0x0A);
        raw.push(piece2.len() as u8);
        raw.extend_from_slice(&piece2);

        let tok = SentencePieceTokenizer::from_bytes(&raw).expect("from_bytes should succeed");
        assert_eq!(tok.pieces.len(), 2);
        assert_eq!(tok.pieces[0].piece, "</s>");
        assert_eq!(tok.pieces[1].piece, "hello");
    }
}
