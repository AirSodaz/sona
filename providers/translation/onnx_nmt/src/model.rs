use crate::tokenizer::SentencePieceTokenizer;
use ort::session::{Session, builder::GraphOptimizationLevel};
use ort::value::Value;
use sona_core::ports::translation::TranslationPortError;
use std::path::Path;

#[derive(Clone, Debug)]
struct BeamHypothesis {
    tokens: Vec<i64>,
    score: f32,
}

fn log_softmax(logits: &[f32]) -> Vec<f32> {
    let mut max_val = f32::NEG_INFINITY;
    for &v in logits {
        if v > max_val {
            max_val = v;
        }
    }
    if max_val.is_infinite() {
        return vec![0.0; logits.len()];
    }
    let mut sum_exp = 0.0f32;
    for &v in logits {
        sum_exp += (v - max_val).exp();
    }
    let log_sum_exp = max_val + sum_exp.ln();
    logits.iter().map(|&v| v - log_sum_exp).collect()
}

pub struct OnnxNmtModel {
    encoder_session: Option<Session>,
    decoder_session: Option<Session>,
    source_tokenizer: SentencePieceTokenizer,
    target_tokenizer: SentencePieceTokenizer,
    max_length: usize,
    beam_size: usize,
    mock_mode: bool,
}

impl OnnxNmtModel {
    pub fn load(model_dir: &Path, num_threads: i32) -> Result<Self, TranslationPortError> {
        let encoder_path = model_dir.join("encoder_model.onnx");
        let decoder_path = model_dir.join("decoder_model.onnx");
        let source_spm = model_dir.join("source.spm");
        let target_spm = model_dir.join("target.spm");
        let vocab_path = model_dir.join("vocab.json");

        if !encoder_path.exists()
            || !decoder_path.exists()
            || !source_spm.exists()
            || !target_spm.exists()
            || !vocab_path.is_file()
        {
            return Err(TranslationPortError::model_not_ready(format!(
                "Missing ONNX NMT model files or vocab.json in {}",
                model_dir.display()
            )));
        }

        let mut source_tokenizer = SentencePieceTokenizer::load(&source_spm)
            .map_err(TranslationPortError::tokenization_failed)?;
        let mut target_tokenizer = SentencePieceTokenizer::load(&target_spm)
            .map_err(TranslationPortError::tokenization_failed)?;

        source_tokenizer
            .load_vocab(&vocab_path)
            .map_err(TranslationPortError::tokenization_failed)?;
        target_tokenizer
            .load_vocab(&vocab_path)
            .map_err(TranslationPortError::tokenization_failed)?;
        let encoder_session = Session::builder()
            .map_err(TranslationPortError::engine_failed)?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(TranslationPortError::engine_failed)?
            .with_intra_threads(num_threads.max(1) as usize)
            .map_err(TranslationPortError::engine_failed)?
            .commit_from_file(&encoder_path)
            .map_err(TranslationPortError::engine_failed)?;

        let decoder_session = Session::builder()
            .map_err(TranslationPortError::engine_failed)?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(TranslationPortError::engine_failed)?
            .with_intra_threads(num_threads.max(1) as usize)
            .map_err(TranslationPortError::engine_failed)?
            .commit_from_file(&decoder_path)
            .map_err(TranslationPortError::engine_failed)?;
        Ok(Self {
            encoder_session: Some(encoder_session),
            decoder_session: Some(decoder_session),
            source_tokenizer,
            target_tokenizer,
            max_length: 256,
            beam_size: 4,
            mock_mode: false,
        })
    }

    pub fn new_mock(
        source_tokenizer: SentencePieceTokenizer,
        target_tokenizer: SentencePieceTokenizer,
    ) -> Self {
        Self {
            encoder_session: None,
            decoder_session: None,
            source_tokenizer,
            target_tokenizer,
            max_length: 256,
            beam_size: 4,
            mock_mode: true,
        }
    }

    pub fn with_beam_size(mut self, beam_size: usize) -> Self {
        self.beam_size = beam_size.max(1);
        self
    }

    pub fn translate(&self, text: &str) -> Result<String, TranslationPortError> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(String::new());
        }

        if self.mock_mode {
            let tokens = self.source_tokenizer.encode(trimmed);
            return Ok(self.target_tokenizer.decode(&tokens));
        }

        let (Some(encoder), Some(decoder)) = (&self.encoder_session, &self.decoder_session) else {
            return Err(TranslationPortError::model_not_ready(
                "Model sessions not loaded",
            ));
        };

        let mut input_ids = self.source_tokenizer.encode(trimmed);
        const MAX_POS_EMBED: usize = 512;
        if input_ids.len() > MAX_POS_EMBED {
            let eos = self.source_tokenizer.eos_id as i64;
            input_ids.truncate(MAX_POS_EMBED - 1);
            input_ids.push(eos);
        }
        let seq_len = input_ids.len();

        let input_ids_array = ndarray::Array2::from_shape_vec((1, seq_len), input_ids.clone())
            .map_err(TranslationPortError::inference_failed)?;

        let attention_mask_array =
            ndarray::Array2::from_shape_vec((1, seq_len), vec![1i64; seq_len])
                .map_err(TranslationPortError::inference_failed)?;

        let input_ids_val =
            Value::from_array(input_ids_array).map_err(TranslationPortError::inference_failed)?;
        let attention_mask_val = Value::from_array(attention_mask_array)
            .map_err(TranslationPortError::inference_failed)?;

        let encoder_outputs = encoder
            .run(
                ort::inputs![
                    "input_ids" => input_ids_val,
                    "attention_mask" => attention_mask_val,
                ]
                .map_err(TranslationPortError::inference_failed)?,
            )
            .map_err(TranslationPortError::inference_failed)?;

        let last_hidden_state = encoder_outputs
            .get("last_hidden_state")
            .or_else(|| encoder_outputs.get("encoder_outputs"))
            .ok_or_else(|| {
                TranslationPortError::inference_failed("Encoder missing last_hidden_state output")
            })?;

        let (last_hidden_shape, last_hidden_data) = last_hidden_state
            .try_extract_raw_tensor::<f32>()
            .map_err(TranslationPortError::inference_failed)?;

        let hidden_dim = if last_hidden_shape.len() >= 3 {
            last_hidden_shape[2] as usize
        } else {
            512
        };

        let start_pad = self.target_tokenizer.pad_id.unwrap_or(65000) as i64;
        let eos_id = self.target_tokenizer.eos_id as i64;
        let beam_size = self.beam_size.max(1);

        let mut active_beams = vec![BeamHypothesis {
            tokens: vec![start_pad],
            score: 0.0,
        }];
        let mut completed_hypotheses: Vec<(Vec<i64>, f32)> = Vec::new();

        let greedy_encoder_tensors = if beam_size == 1 {
            let hidden_states_array = ndarray::Array3::from_shape_vec(
                (1, seq_len, hidden_dim),
                last_hidden_data.to_vec(),
            )
            .map_err(TranslationPortError::inference_failed)?;
            let hidden_val = Value::from_array(hidden_states_array)
                .map_err(TranslationPortError::inference_failed)?;
            let enc_attn_array = ndarray::Array2::from_shape_vec((1, seq_len), vec![1i64; seq_len])
                .map_err(TranslationPortError::inference_failed)?;
            let enc_attn_val = Value::from_array(enc_attn_array)
                .map_err(TranslationPortError::inference_failed)?;
            Some((hidden_val, enc_attn_val))
        } else {
            None
        };

        for _ in 0..self.max_length {
            let b = active_beams.len();
            let target_len = active_beams[0].tokens.len();

            let mut flattened_ids = Vec::with_capacity(b * target_len);
            for beam in &active_beams {
                flattened_ids.extend_from_slice(&beam.tokens);
            }
            let dec_ids_array = ndarray::Array2::from_shape_vec((b, target_len), flattened_ids)
                .map_err(TranslationPortError::inference_failed)?;
            let dec_ids_val =
                Value::from_array(dec_ids_array).map_err(TranslationPortError::inference_failed)?;

            let (hidden_session_val, enc_attn_session_val) = match &greedy_encoder_tensors {
                Some((hidden_val, enc_attn_val)) => (
                    ort::session::SessionInputValue::from(hidden_val.view()),
                    ort::session::SessionInputValue::from(enc_attn_val.view()),
                ),
                None => {
                    let mut b_hidden_data = Vec::with_capacity(b * seq_len * hidden_dim);
                    for _ in 0..b {
                        b_hidden_data.extend_from_slice(last_hidden_data);
                    }
                    let hidden_states_array =
                        ndarray::Array3::from_shape_vec((b, seq_len, hidden_dim), b_hidden_data)
                            .map_err(TranslationPortError::inference_failed)?;
                    let hidden_val = Value::from_array(hidden_states_array)
                        .map_err(TranslationPortError::inference_failed)?;

                    let enc_attn_array =
                        ndarray::Array2::from_shape_vec((b, seq_len), vec![1i64; b * seq_len])
                            .map_err(TranslationPortError::inference_failed)?;
                    let enc_attn_val = Value::from_array(enc_attn_array)
                        .map_err(TranslationPortError::inference_failed)?;

                    (
                        ort::session::SessionInputValue::from(hidden_val),
                        ort::session::SessionInputValue::from(enc_attn_val),
                    )
                }
            };

            let decoder_outputs = decoder
                .run(vec![
                    (
                        "input_ids",
                        ort::session::SessionInputValue::from(dec_ids_val),
                    ),
                    ("encoder_hidden_states", hidden_session_val),
                    ("encoder_attention_mask", enc_attn_session_val),
                ])
                .map_err(TranslationPortError::inference_failed)?;

            let logits_val = decoder_outputs.get("logits").ok_or_else(|| {
                TranslationPortError::inference_failed("Decoder missing logits output")
            })?;

            let (logits_shape, logits_data) = logits_val
                .try_extract_raw_tensor::<f32>()
                .map_err(TranslationPortError::inference_failed)?;

            let vocab_size = if logits_shape.len() >= 3 {
                logits_shape[2] as usize
            } else {
                return Err(TranslationPortError::inference_failed(
                    "Invalid logits shape",
                ));
            };

            let current_seq_len = logits_shape[1] as usize;
            let last_step = if current_seq_len > 0 {
                current_seq_len - 1
            } else {
                0
            };

            let mut all_candidates: Vec<BeamHypothesis> = Vec::new();

            for (beam_idx, beam) in active_beams.iter().enumerate() {
                let beam_offset = beam_idx * current_seq_len * vocab_size + last_step * vocab_size;
                let beam_end = beam_offset + vocab_size;
                if beam_end > logits_data.len() {
                    return Err(TranslationPortError::inference_failed(
                        "Logits tensor buffer length mismatch",
                    ));
                }
                let beam_logits = &logits_data[beam_offset..beam_end];
                let log_probs = log_softmax(beam_logits);

                let mut token_scores: Vec<(usize, f32)> = Vec::with_capacity(vocab_size);
                for (token_id, &lp) in log_probs.iter().enumerate() {
                    if token_id as i64 == start_pad {
                        continue;
                    }
                    token_scores.push((token_id, lp));
                }

                let k = (2 * beam_size).min(token_scores.len());
                if k > 0 {
                    token_scores.select_nth_unstable_by(k - 1, |a, b| {
                        b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
                    });
                    token_scores.truncate(k);
                }

                for (token_id, lp) in token_scores {
                    let total_score = beam.score + lp;
                    let token_i64 = token_id as i64;
                    if token_i64 == eos_id {
                        let gen_len = beam.tokens.len().saturating_sub(1).max(1);
                        let length_penalty = ((5.0 + gen_len as f32) / 6.0).powf(1.0);
                        let normalized_score = total_score / length_penalty;
                        completed_hypotheses.push((beam.tokens.clone(), normalized_score));
                    } else {
                        let mut next_tokens = beam.tokens.clone();
                        next_tokens.push(token_i64);
                        all_candidates.push(BeamHypothesis {
                            tokens: next_tokens,
                            score: total_score,
                        });
                    }
                }
            }

            if all_candidates.is_empty() {
                break;
            }

            all_candidates.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            all_candidates.truncate(beam_size);
            active_beams = all_candidates;

            if !completed_hypotheses.is_empty() {
                let best_completed_score = completed_hypotheses
                    .iter()
                    .map(|c| c.1)
                    .fold(f32::NEG_INFINITY, f32::max);
                let active_len = active_beams[0].tokens.len().saturating_sub(1).max(1);
                let active_penalty = ((5.0 + active_len as f32) / 6.0).powf(1.0);
                let best_active_potential = active_beams[0].score / active_penalty;
                if completed_hypotheses.len() >= beam_size
                    || best_completed_score >= best_active_potential
                {
                    break;
                }
            }
        }

        let best_tokens = if !completed_hypotheses.is_empty() {
            completed_hypotheses
                .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            completed_hypotheses[0].0.clone()
        } else {
            active_beams[0].tokens.clone()
        };

        let output_tokens = if best_tokens.len() > 1 {
            &best_tokens[1..]
        } else {
            &[]
        };
        let decoded = self.target_tokenizer.decode(output_tokens);
        Ok(decoded)
    }
}
