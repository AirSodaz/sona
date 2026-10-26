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
#[derive(Debug, thiserror::Error)]
pub enum TokenizerError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Tokenizer format error: {0}")]
    Format(String),
}

impl From<&str> for TokenizerError {
    fn from(s: &str) -> Self {
        Self::Format(s.to_string())
    }
}

impl From<String> for TokenizerError {
    fn from(s: String) -> Self {
        Self::Format(s)
    }
}
#[derive(Clone, Debug, Default)]
pub struct SentencePieceTokenizer {
    pub pieces: Vec<PieceEntry>,
    pub piece_to_id: HashMap<String, u32>,
    pub pad_id: Option<u32>,
    pub eos_id: u32,
    pub unk_id: u32,
    pub vocab_to_id: Option<HashMap<String, u32>>,
    pub id_to_vocab: Option<Vec<String>>,
    max_piece_len: usize,
}
impl SentencePieceTokenizer {
    pub fn load(path: &Path) -> Result<Self, TokenizerError> {
        let bytes = fs::read(path)?;
        Self::from_bytes(&bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, TokenizerError> {
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
                let sub_end = idx
                    .checked_add(len as usize)
                    .filter(|&end| end <= bytes.len())
                    .ok_or_else(|| {
                        "Unexpected EOF or overflow reading SentencePiece entry".to_string()
                    })?;
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
        let pad_id = piece_to_id.get("<pad>").copied().or(Some(65000));

        Ok(Self {
            pieces,
            piece_to_id,
            pad_id,
            eos_id,
            unk_id,
            vocab_to_id: None,
            id_to_vocab: None,
            max_piece_len: max_piece_len.max(1),
        })
    }

    pub fn load_vocab(&mut self, path: &Path) -> Result<(), TokenizerError> {
        let content = fs::read_to_string(path)?;
        let vocab: HashMap<String, u32> = serde_json::from_str(&content)?;
        self.set_vocab(vocab);
        if self.vocab_to_id.is_none() {
            return Err(TokenizerError::Format(format!(
                "Vocabulary in {} exceeds MAX_SUPPORTED_VOCAB_ID",
                path.display()
            )));
        }
        Ok(())
    }

    pub fn set_vocab(&mut self, vocab: HashMap<String, u32>) {
        const MAX_SUPPORTED_VOCAB_ID: u32 = 500_000;
        let max_id = vocab.values().copied().max().unwrap_or(0);
        if max_id > MAX_SUPPORTED_VOCAB_ID {
            log::error!(
                "vocab max_id {} exceeds MAX_SUPPORTED_VOCAB_ID {}",
                max_id,
                MAX_SUPPORTED_VOCAB_ID
            );
            return;
        }
        let cap = (max_id as usize)
            .checked_add(1)
            .unwrap_or(MAX_SUPPORTED_VOCAB_ID as usize);
        let mut id_to_vocab = vec![String::new(); cap];
        for (token, &id) in &vocab {
            if let Some(slot) = id_to_vocab.get_mut(id as usize) {
                *slot = token.clone();
            }
        }

        if let Some(&eos) = vocab.get("</s>") {
            self.eos_id = eos;
        }
        if let Some(&unk) = vocab.get("<unk>") {
            self.unk_id = unk;
        }
        if let Some(&pad) = vocab.get("<pad>") {
            self.pad_id = Some(pad);
        } else if self.pad_id.is_none() {
            self.pad_id = Some(65000);
        }

        self.vocab_to_id = Some(vocab);
        self.id_to_vocab = Some(id_to_vocab);
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

        let spm_unk_id = self.piece_to_id.get("<unk>").copied().unwrap_or(1);
        let mut best_score = vec![f32::NEG_INFINITY; n + 1];
        let mut best_prev = vec![(0usize, spm_unk_id); n + 1];
        best_score[0] = 0.0;
        let effective_max_l = if self.max_piece_len > 0 {
            self.max_piece_len
        } else {
            self.piece_to_id
                .keys()
                .map(|k| k.chars().count())
                .max()
                .unwrap_or(1)
        };

        for i in 0..n {
            if best_score[i] == f32::NEG_INFINITY {
                continue;
            }

            let max_l = effective_max_l.min(n - i);
            for len in 1..=max_l {
                let sub: String = chars[i..i + len].iter().collect();
                if let Some(&token_id) = self.piece_to_id.get(&sub) {
                    let score = self
                        .pieces
                        .get(token_id as usize)
                        .map_or(-100.0, |p| p.score);
                    let candidate = best_score[i] + score;
                    if candidate > best_score[i + len] {
                        best_score[i + len] = candidate;
                        best_prev[i + len] = (i, token_id);
                    }
                }
            }

            let unk_score = self
                .pieces
                .get(spm_unk_id as usize)
                .map_or(-100.0, |p| if p.score < 0.0 { p.score } else { -100.0 });
            let unk_candidate = best_score[i] + unk_score;
            if unk_candidate > best_score[i + 1] {
                best_score[i + 1] = unk_candidate;
                best_prev[i + 1] = (i, spm_unk_id);
            }
        }

        let mut piece_ids = Vec::new();
        let mut curr = n;
        while curr > 0 {
            let (prev, token_id) = best_prev[curr];
            piece_ids.push(token_id);
            curr = prev;
        }
        piece_ids.reverse();

        let mut tokens = Vec::with_capacity(piece_ids.len() + 1);
        if let Some(vocab) = &self.vocab_to_id {
            for spm_id in piece_ids {
                let piece_str = self
                    .pieces
                    .get(spm_id as usize)
                    .map(|p| p.piece.as_str())
                    .unwrap_or("<unk>");
                let id = vocab.get(piece_str).copied().unwrap_or(self.unk_id);
                tokens.push(id as i64);
            }
        } else {
            for spm_id in piece_ids {
                tokens.push(spm_id as i64);
            }
        }

        tokens.push(self.eos_id as i64);
        tokens
    }

    pub fn decode(&self, token_ids: &[i64]) -> String {
        let mut result = String::new();
        let mut byte_buf: Vec<u8> = Vec::new();

        if let Some(id_to_vocab) = &self.id_to_vocab {
            for &id in token_ids {
                if id == self.eos_id as i64 || self.pad_id.is_some_and(|pad| id == pad as i64) {
                    break;
                }
                let idx = id as usize;
                if idx >= id_to_vocab.len() {
                    continue;
                }
                let piece = &id_to_vocab[idx];
                if piece.is_empty()
                    || piece == "<unk>"
                    || piece == "<s>"
                    || piece == "</s>"
                    || piece == "<pad>"
                {
                    if !byte_buf.is_empty() {
                        result.push_str(&String::from_utf8_lossy(&byte_buf));
                        byte_buf.clear();
                    }
                    continue;
                }

                if piece.starts_with(">>") && piece.ends_with("<<") {
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
        } else {
            for &id in token_ids {
                if id == self.eos_id as i64 || self.pad_id.is_some_and(|pad| id == pad as i64) {
                    break;
                }
                let idx = id as usize;
                if idx >= self.pieces.len() {
                    continue;
                }
                let piece = &self.pieces[idx].piece;
                if piece.is_empty()
                    || piece == "<unk>"
                    || piece == "<s>"
                    || piece == "</s>"
                    || piece == "<pad>"
                {
                    if !byte_buf.is_empty() {
                        result.push_str(&String::from_utf8_lossy(&byte_buf));
                        byte_buf.clear();
                    }
                    continue;
                }

                if piece.starts_with(">>") && piece.ends_with("<<") {
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
                let end = idx
                    .checked_add(len as usize)
                    .filter(|&end| end <= bytes.len())
                    .ok_or_else(|| "Invalid piece length or overflow".to_string())?;
                piece = String::from_utf8_lossy(&bytes[idx..end]).into_owned();
                idx = end;
            }
            (2, 5) => {
                let end = idx
                    .checked_add(4)
                    .filter(|&end| end <= bytes.len())
                    .ok_or_else(|| "Invalid score bytes".to_string())?;
                let score_bytes = [bytes[idx], bytes[idx + 1], bytes[idx + 2], bytes[idx + 3]];
                score = f32::from_le_bytes(score_bytes);
                idx = end;
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
        1 => idx
            .checked_add(8)
            .filter(|&end| end <= bytes.len())
            .ok_or_else(|| "Unexpected EOF in 64-bit field".to_string()),
        2 => {
            let (len, new_idx) = read_varint(bytes, idx)?;
            new_idx
                .checked_add(len as usize)
                .filter(|&end| end <= bytes.len())
                .ok_or_else(|| "Unexpected EOF or overflow in length-delimited field".to_string())
        }
        5 => idx
            .checked_add(4)
            .filter(|&end| end <= bytes.len())
            .ok_or_else(|| "Unexpected EOF in 32-bit field".to_string()),
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

    #[test]
    fn test_sentencepiece_truncated_bytes_returns_error() {
        // Tag indicates length of 100 bytes, but only 2 bytes follow
        let truncated = vec![0x0A, 100, 0x01, 0x02];
        assert!(SentencePieceTokenizer::from_bytes(&truncated).is_err());
    }

    #[test]
    fn test_sentencepiece_default_pad_id() {
        let tok = SentencePieceTokenizer::from_bytes(&[]).expect("empty bytes succeeds");
        assert_eq!(tok.pad_id, Some(65000));
    }

    #[test]
    fn test_sentencepiece_with_vocab_mapping() {
        let mut tokenizer = SentencePieceTokenizer::default();
        let pieces = vec![
            ("</s>", 0.0),
            ("<unk>", 0.0),
            ("\u{2581}Good", -1.0),
            ("\u{2581}morning", -1.0),
            (".", -0.5),
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
        tokenizer.pad_id = Some(65000);
        tokenizer.max_piece_len = 10;

        // Setup mock vocab mapping with merged vocabulary IDs
        let mut vocab = HashMap::new();
        vocab.insert("</s>".to_string(), 0);
        vocab.insert("<unk>".to_string(), 1);
        vocab.insert("\u{2581}Good".to_string(), 1050);
        vocab.insert("\u{2581}morning".to_string(), 2340);
        vocab.insert(".".to_string(), 6);
        vocab.insert("<pad>".to_string(), 65000);
        tokenizer.set_vocab(vocab);

        let tokens = tokenizer.encode("Good morning.");
        // Output should be mapped vocab IDs rather than raw SPM piece indices (2, 3, 4, 0)
        assert_eq!(tokens, vec![1050, 2340, 6, 0]);
    }

    #[test]
    fn test_sentencepiece_decode_with_vocab_mapping_and_tags() {
        let mut tokenizer = SentencePieceTokenizer::default();
        let mut vocab = HashMap::new();
        vocab.insert("</s>".to_string(), 0);
        vocab.insert("<unk>".to_string(), 1);
        vocab.insert(">>cmn_Hans<<".to_string(), 5);
        vocab.insert("。".to_string(), 10);
        vocab.insert("\u{2581}早".to_string(), 50000);
        vocab.insert("上".to_string(), 50001);
        vocab.insert("好".to_string(), 50002);
        vocab.insert("<pad>".to_string(), 65000);
        tokenizer.set_vocab(vocab);

        // Tokens include language tag >>cmn_Hans<< (5), tokens >= 32000, and punctuation
        let token_ids = vec![5, 50000, 50001, 50002, 10, 0];
        let decoded = tokenizer.decode(&token_ids);
        assert_eq!(decoded, "早上好。");
    }

    #[test]
    fn test_set_vocab_caps_at_max_supported() {
        let mut tokenizer = SentencePieceTokenizer::default();
        let mut vocab = HashMap::new();
        vocab.insert("huge_id".to_string(), 1_000_000);
        tokenizer.set_vocab(vocab);
        assert!(tokenizer.id_to_vocab.is_none());
        assert!(tokenizer.vocab_to_id.is_none());
    }

    #[test]
    fn test_sentencepiece_unk_fallback_dp() {
        let mut tokenizer = SentencePieceTokenizer::default();
        tokenizer.pieces.push(PieceEntry {
            piece: "</s>".to_string(),
            score: 0.0,
            piece_type: 0,
        });
        tokenizer.pieces.push(PieceEntry {
            piece: "<unk>".to_string(),
            score: -20.0,
            piece_type: 0,
        });
        tokenizer.pieces.push(PieceEntry {
            piece: "\u{2581}A".to_string(),
            score: -1.0,
            piece_type: 0,
        });
        tokenizer.piece_to_id.insert("</s>".to_string(), 0);
        tokenizer.piece_to_id.insert("<unk>".to_string(), 1);
        tokenizer.piece_to_id.insert("\u{2581}A".to_string(), 2);
        tokenizer.eos_id = 0;
        tokenizer.unk_id = 1;

        // 'Z' is not in vocabulary, so it should fallback to <unk> via DP
        let tokens = tokenizer.encode("A Z");
        // Expect \u{2581}A (2), \u{2581} (unk: 1), Z (unk: 1), eos: 0
        assert!(tokens.contains(&1));
        assert_eq!(*tokens.last().unwrap(), 0);
    }
}
