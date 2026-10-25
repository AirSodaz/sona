use crate::tokenizer::SentencePieceTokenizer;
use ort::session::{Session, builder::GraphOptimizationLevel};
use ort::value::Value;
use sona_core::ports::translation::TranslationPortError;
use std::path::Path;

pub struct OnnxNmtModel {
    encoder_session: Option<Session>,
    decoder_session: Option<Session>,
    source_tokenizer: SentencePieceTokenizer,
    target_tokenizer: SentencePieceTokenizer,
    max_length: usize,
    mock_mode: bool,
}

impl OnnxNmtModel {
    pub fn load(model_dir: &Path, num_threads: i32) -> Result<Self, TranslationPortError> {
        let encoder_path = model_dir.join("encoder_model.onnx");
        let decoder_path = model_dir.join("decoder_model.onnx");
        let source_spm = model_dir.join("source.spm");
        let target_spm = model_dir.join("target.spm");

        if !encoder_path.exists()
            || !decoder_path.exists()
            || !source_spm.exists()
            || !target_spm.exists()
        {
            return Err(TranslationPortError::model_not_ready(format!(
                "Missing ONNX NMT model files in {}",
                model_dir.display()
            )));
        }

        let source_tokenizer = SentencePieceTokenizer::load(&source_spm)
            .map_err(TranslationPortError::tokenization_failed)?;
        let target_tokenizer = SentencePieceTokenizer::load(&target_spm)
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
            mock_mode: true,
        }
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

        let input_ids = self.source_tokenizer.encode(trimmed);
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

        let hidden_states_array =
            ndarray::Array3::from_shape_vec((1, seq_len, hidden_dim), last_hidden_data.to_vec())
                .map_err(TranslationPortError::inference_failed)?;

        let enc_attn_array = ndarray::Array2::from_shape_vec((1, seq_len), vec![1i64; seq_len])
            .map_err(TranslationPortError::inference_failed)?;
        let enc_attn_val =
            Value::from_array(enc_attn_array).map_err(TranslationPortError::inference_failed)?;
        let hidden_val = Value::from_array(hidden_states_array)
            .map_err(TranslationPortError::inference_failed)?;

        let mut decoder_input_ids = vec![self.target_tokenizer.pad_id.unwrap_or(65000) as i64];

        for _ in 0..self.max_length {
            let target_len = decoder_input_ids.len();
            let dec_ids_array =
                ndarray::Array2::from_shape_vec((1, target_len), decoder_input_ids.clone())
                    .map_err(TranslationPortError::inference_failed)?;

            let dec_ids_val =
                Value::from_array(dec_ids_array).map_err(TranslationPortError::inference_failed)?;
            let decoder_outputs = decoder
                .run(vec![
                    (
                        "input_ids",
                        ort::session::SessionInputValue::from(dec_ids_val),
                    ),
                    (
                        "encoder_hidden_states",
                        ort::session::SessionInputValue::from(hidden_val.view()),
                    ),
                    (
                        "encoder_attention_mask",
                        ort::session::SessionInputValue::from(enc_attn_val.view()),
                    ),
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

            let seq_len = logits_shape[1] as usize;
            let last_step = if seq_len > 0 { seq_len - 1 } else { 0 };
            let last_token_logits_start = last_step * vocab_size;
            let last_token_logits_end = last_token_logits_start + vocab_size;
            if last_token_logits_end > logits_data.len() {
                return Err(TranslationPortError::inference_failed(
                    "Logits tensor buffer length mismatch",
                ));
            }
            let last_token_logits = &logits_data[last_token_logits_start..last_token_logits_end];

            let mut best_id = 0;
            let mut best_val = f32::NEG_INFINITY;
            for (idx, &v) in last_token_logits.iter().enumerate() {
                if v > best_val {
                    best_val = v;
                    best_id = idx;
                }
            }

            let next_token = best_id as i64;
            let is_stop = next_token == self.target_tokenizer.eos_id as i64
                || self
                    .target_tokenizer
                    .pad_id
                    .is_some_and(|pad| next_token == pad as i64);
            if is_stop {
                break;
            }

            decoder_input_ids.push(next_token);
        }

        let output_tokens = if decoder_input_ids.len() > 1 {
            &decoder_input_ids[1..]
        } else {
            &[]
        };
        let decoded = self.target_tokenizer.decode(output_tokens);
        Ok(decoded)
    }
}
