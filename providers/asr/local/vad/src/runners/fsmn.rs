use super::VadModelRunner;
use crate::fbank::{
    CmvnParams, FRAME_LENGTH_SAMPLES, FRAME_SHIFT_SAMPLES, FbankExtractor, apply_lfr_and_cmvn,
};
use ndarray::{ArrayD, ArrayView4, ArrayViewD, IxDyn};
use ort::session::Session;
use ort::value::Value;
use sona_core::ports::asr::{AsrPortError, AsrPortErrorKind};
use std::collections::VecDeque;
use std::path::Path;
use std::sync::Arc;

const CACHE_SHAPE: [usize; 4] = [1, 128, 19, 1];
const CACHE_LEN: usize = 128 * 19;

pub struct FsmnVadRunner {
    session: Arc<Session>,
    fbank: FbankExtractor,
    cmvn: CmvnParams,
    // Ping-pong caches for 4 FSMN layers
    cache0: Vec<f32>,
    cache1: Vec<f32>,
    cache2: Vec<f32>,
    cache3: Vec<f32>,
    out_cache0: Vec<f32>,
    out_cache1: Vec<f32>,
    out_cache2: Vec<f32>,
    out_cache3: Vec<f32>,
    // Streaming sample buffer
    sample_buffer: Vec<f32>,
    // Streaming recent 80-dim fbank frame history with silence flags (up to 5 frames for symmetric LFR)
    fbank_history: VecDeque<([f32; 80], bool)>,
    window_size: usize,
}

impl FsmnVadRunner {
    pub fn from_session(session: Arc<Session>, path: Option<&Path>) -> Result<Self, AsrPortError> {
        let has_speech = session.inputs.iter().any(|i| i.name == "speech");
        let has_caches = (0..4).all(|idx| {
            let in_name = format!("in_cache{idx}");
            session.inputs.iter().any(|i| i.name == in_name)
        });
        if !has_speech || !has_caches {
            return Err(AsrPortError::new(
                AsrPortErrorKind::Model,
                "Unsupported FSMN model: missing 'speech' or 'in_cache0..3' inputs",
            ));
        }

        let has_logits = session.outputs.iter().any(|o| o.name == "logits");
        let has_out_caches = (0..4).all(|idx| {
            let out_name = format!("out_cache{idx}");
            session.outputs.iter().any(|o| o.name == out_name)
        });
        if !has_logits || !has_out_caches {
            return Err(AsrPortError::new(
                AsrPortErrorKind::Model,
                "Unsupported FSMN model: missing 'logits' or 'out_cache0..3' outputs",
            ));
        }

        let mut cmvn = CmvnParams::default();
        if let Some(p) = path {
            let search_dir = if p.is_dir() {
                p.to_path_buf()
            } else {
                p.parent().unwrap_or_else(|| Path::new("")).to_path_buf()
            };
            let candidates = ["am.mvn", "vad.mvn"];
            for candidate in &candidates {
                let cand_path = search_dir.join(candidate);
                if cand_path.is_file()
                    && let Ok(loaded) = CmvnParams::load_am_mvn(&cand_path)
                {
                    cmvn = loaded;
                    break;
                }
            }
        }
        Ok(Self {
            session,
            fbank: FbankExtractor::new(),
            cmvn,
            cache0: vec![0.0f32; CACHE_LEN],
            cache1: vec![0.0f32; CACHE_LEN],
            cache2: vec![0.0f32; CACHE_LEN],
            cache3: vec![0.0f32; CACHE_LEN],
            out_cache0: vec![0.0f32; CACHE_LEN],
            out_cache1: vec![0.0f32; CACHE_LEN],
            out_cache2: vec![0.0f32; CACHE_LEN],
            out_cache3: vec![0.0f32; CACHE_LEN],
            sample_buffer: Vec::with_capacity(1600),
            fbank_history: VecDeque::with_capacity(6),
            window_size: FRAME_SHIFT_SAMPLES, // 160 samples (10ms) hop
        })
    }

    pub fn from_file(path: &Path) -> Result<Self, AsrPortError> {
        let session = crate::engine::get_or_create_session(path)?;
        Self::from_session(session, Some(path))
    }

    fn extract_prob_from_slice(slice: &[f32], is_silence_audio: bool) -> f32 {
        if is_silence_audio {
            return 0.0;
        }

        let prob = if slice.len() == 2 {
            let l0 = slice[0];
            let l1 = slice[1];
            if !l0.is_finite() || !l1.is_finite() {
                0.0
            } else if l0 >= 0.0 && l1 >= 0.0 && (l0 + l1 - 1.0).abs() < 1e-2 {
                l1.clamp(0.0, 1.0)
            } else {
                let max_l = l0.max(l1);
                let e0 = (l0 - max_l).exp();
                let e1 = (l1 - max_l).exp();
                let sum = e0 + e1;
                if sum.is_finite() && sum > 0.0 {
                    (e1 / sum).clamp(0.0, 1.0)
                } else {
                    0.0
                }
            }
        } else if slice.len() == 1 {
            let s = slice[0];
            if !s.is_finite() {
                0.0
            } else if (0.0..=1.0).contains(&s) {
                s
            } else {
                (1.0 / (1.0 + (-s).exp())).clamp(0.0, 1.0)
            }
        } else if !slice.is_empty() {
            let noise_score = slice[0];
            if noise_score.is_finite() {
                (1.0 - noise_score.clamp(0.0, 1.0)).clamp(0.0, 1.0)
            } else {
                0.0
            }
        } else {
            0.0
        };

        if prob.is_nan() { 0.0 } else { prob }
    }
}

impl VadModelRunner for FsmnVadRunner {
    fn window_size(&self) -> usize {
        self.window_size
    }

    fn reset_state(&mut self) {
        self.cache0.fill(0.0);
        self.cache1.fill(0.0);
        self.cache2.fill(0.0);
        self.cache3.fill(0.0);
        self.out_cache0.fill(0.0);
        self.out_cache1.fill(0.0);
        self.out_cache2.fill(0.0);
        self.out_cache3.fill(0.0);
        self.sample_buffer.clear();
        self.fbank_history.clear();
    }

    fn step(&mut self, chunk: &[f32]) -> Result<f32, AsrPortError> {
        if chunk.len() != self.window_size {
            return Err(AsrPortError::new(
                AsrPortErrorKind::Unsupported,
                format!(
                    "FsmnVadRunner expected chunk of {} samples (got {})",
                    self.window_size,
                    chunk.len()
                ),
            ));
        }
        self.sample_buffer.extend_from_slice(chunk);

        if self.sample_buffer.len() < FRAME_LENGTH_SAMPLES {
            return Ok(0.0);
        }

        let is_silence = is_digital_silence(&self.sample_buffer[..FRAME_LENGTH_SAMPLES]);
        // 1. Extract 80-dim log-mel fbank for current 400-sample window
        let frame_80 = self
            .fbank
            .extract_frame(&self.sample_buffer[..FRAME_LENGTH_SAMPLES]);
        // Shift buffer by 160 samples (10ms)
        self.sample_buffer.drain(..FRAME_SHIFT_SAMPLES);

        // 2. Symmetric LFR: lfr_m = 5, left_pad = 2.
        // Pad left context with the first frame on stream start
        if self.fbank_history.is_empty() {
            self.fbank_history.push_back((frame_80, is_silence));
            self.fbank_history.push_back((frame_80, is_silence));
        }
        self.fbank_history.push_back((frame_80, is_silence));

        // Require 5 frames before evaluating the center frame (slot 2)
        if self.fbank_history.len() < 5 {
            return Ok(0.0);
        }
        while self.fbank_history.len() > 5 {
            self.fbank_history.pop_front();
        }
        let center_is_silence = self.fbank_history[2].1;
        let mut stacked_400 = [0.0f32; 400];
        for m in 0..5 {
            let frame_ref = &self.fbank_history[m].0;
            let dst = m * 80;
            for b in 0..80 {
                stacked_400[dst + b] = (frame_ref[b] + self.cmvn.shifts[b]) * self.cmvn.rescales[b];
            }
        }
        // 3. Bind ONNX inputs: speech [1, 1, 400] and in_cache0..3 [1, 128, 19, 1]
        let speech_view =
            ArrayViewD::from_shape(IxDyn(&[1, 1, 400]), &stacked_400).map_err(|e| {
                AsrPortError::new(
                    AsrPortErrorKind::Runtime,
                    format!("Failed to build FSMN speech tensor view: {e}"),
                )
            })?;
        let speech_val = Value::from_array(speech_view).map_err(|e| {
            AsrPortError::new(
                AsrPortErrorKind::Runtime,
                format!("Failed to bind FSMN speech input: {e}"),
            )
        })?;

        let c0_view = ArrayView4::from_shape(CACHE_SHAPE, &self.cache0)
            .map_err(|e| AsrPortError::new(AsrPortErrorKind::Runtime, e.to_string()))?;
        let c1_view = ArrayView4::from_shape(CACHE_SHAPE, &self.cache1)
            .map_err(|e| AsrPortError::new(AsrPortErrorKind::Runtime, e.to_string()))?;
        let c2_view = ArrayView4::from_shape(CACHE_SHAPE, &self.cache2)
            .map_err(|e| AsrPortError::new(AsrPortErrorKind::Runtime, e.to_string()))?;
        let c3_view = ArrayView4::from_shape(CACHE_SHAPE, &self.cache3)
            .map_err(|e| AsrPortError::new(AsrPortErrorKind::Runtime, e.to_string()))?;
        let inputs =
            vec![
                ("speech", ort::session::SessionInputValue::from(speech_val)),
                (
                    "in_cache0",
                    ort::session::SessionInputValue::from(Value::from_array(c0_view).map_err(
                        |e| AsrPortError::new(AsrPortErrorKind::Runtime, e.to_string()),
                    )?),
                ),
                (
                    "in_cache1",
                    ort::session::SessionInputValue::from(Value::from_array(c1_view).map_err(
                        |e| AsrPortError::new(AsrPortErrorKind::Runtime, e.to_string()),
                    )?),
                ),
                (
                    "in_cache2",
                    ort::session::SessionInputValue::from(Value::from_array(c2_view).map_err(
                        |e| AsrPortError::new(AsrPortErrorKind::Runtime, e.to_string()),
                    )?),
                ),
                (
                    "in_cache3",
                    ort::session::SessionInputValue::from(Value::from_array(c3_view).map_err(
                        |e| AsrPortError::new(AsrPortErrorKind::Runtime, e.to_string()),
                    )?),
                ),
            ];

        let outputs = self.session.run(inputs).map_err(|e| {
            AsrPortError::new(
                AsrPortErrorKind::Runtime,
                format!("FSMN VAD inference failed: {e}"),
            )
        })?;

        let logits_tensor = outputs.get("logits").ok_or_else(|| {
            AsrPortError::new(AsrPortErrorKind::Runtime, "Missing logits in FSMN output")
        })?;
        let (_, logits_data) = logits_tensor.try_extract_raw_tensor::<f32>().map_err(|e| {
            AsrPortError::new(
                AsrPortErrorKind::Runtime,
                format!("Failed to extract FSMN logits: {e}"),
            )
        })?;

        let prob = Self::extract_prob_from_slice(logits_data, center_is_silence);

        // Update caches and fail if missing or malformed
        for i in 0..4 {
            let name = match i {
                0 => "out_cache0",
                1 => "out_cache1",
                2 => "out_cache2",
                _ => "out_cache3",
            };
            let cache_tensor = outputs.get(name).ok_or_else(|| {
                AsrPortError::new(
                    AsrPortErrorKind::Runtime,
                    format!("Missing {name} in FSMN output"),
                )
            })?;
            let (_, data) = cache_tensor.try_extract_raw_tensor::<f32>().map_err(|e| {
                AsrPortError::new(
                    AsrPortErrorKind::Runtime,
                    format!("Failed to extract {name}: {e}"),
                )
            })?;
            if data.len() != CACHE_LEN {
                return Err(AsrPortError::new(
                    AsrPortErrorKind::Runtime,
                    format!(
                        "Expected {CACHE_LEN} elements for {name}, got {}",
                        data.len()
                    ),
                ));
            }
            match i {
                0 => self.out_cache0.copy_from_slice(data),
                1 => self.out_cache1.copy_from_slice(data),
                2 => self.out_cache2.copy_from_slice(data),
                _ => self.out_cache3.copy_from_slice(data),
            }
        }

        std::mem::swap(&mut self.cache0, &mut self.out_cache0);
        std::mem::swap(&mut self.cache1, &mut self.out_cache1);
        std::mem::swap(&mut self.cache2, &mut self.out_cache2);
        std::mem::swap(&mut self.cache3, &mut self.out_cache3);
        self.fbank_history.pop_front();
        Ok(prob)
    }

    fn predict_probs_batch(&mut self, samples: &[f32]) -> Result<Vec<f32>, AsrPortError> {
        self.reset_state();

        let frames = self.fbank.extract_all(samples);
        if frames.is_empty() {
            return Ok(Vec::new());
        }

        let lfr_feats = apply_lfr_and_cmvn(&frames, &self.cmvn, 5, 1);
        let num_frames = lfr_feats.len();

        let mut flat_data = Vec::with_capacity(num_frames * 400);
        for frame in &lfr_feats {
            flat_data.extend_from_slice(frame);
        }

        let speech_arr =
            ArrayD::from_shape_vec(IxDyn(&[1, num_frames, 400]), flat_data).map_err(|e| {
                AsrPortError::new(
                    AsrPortErrorKind::Runtime,
                    format!("Failed to build batch FSMN tensor: {e}"),
                )
            })?;
        let speech_val = Value::from_array(speech_arr).map_err(|e| {
            AsrPortError::new(
                AsrPortErrorKind::Runtime,
                format!("Failed to bind batch FSMN tensor: {e}"),
            )
        })?;

        // Bind zero cache views (already zeroed by reset_state above)
        let c0_view = ArrayView4::from_shape(CACHE_SHAPE, &self.cache0)
            .map_err(|e| AsrPortError::new(AsrPortErrorKind::Runtime, e.to_string()))?;
        let c1_view = ArrayView4::from_shape(CACHE_SHAPE, &self.cache1)
            .map_err(|e| AsrPortError::new(AsrPortErrorKind::Runtime, e.to_string()))?;
        let c2_view = ArrayView4::from_shape(CACHE_SHAPE, &self.cache2)
            .map_err(|e| AsrPortError::new(AsrPortErrorKind::Runtime, e.to_string()))?;
        let c3_view = ArrayView4::from_shape(CACHE_SHAPE, &self.cache3)
            .map_err(|e| AsrPortError::new(AsrPortErrorKind::Runtime, e.to_string()))?;
        let inputs =
            vec![
                ("speech", ort::session::SessionInputValue::from(speech_val)),
                (
                    "in_cache0",
                    ort::session::SessionInputValue::from(Value::from_array(c0_view).map_err(
                        |e| AsrPortError::new(AsrPortErrorKind::Runtime, e.to_string()),
                    )?),
                ),
                (
                    "in_cache1",
                    ort::session::SessionInputValue::from(Value::from_array(c1_view).map_err(
                        |e| AsrPortError::new(AsrPortErrorKind::Runtime, e.to_string()),
                    )?),
                ),
                (
                    "in_cache2",
                    ort::session::SessionInputValue::from(Value::from_array(c2_view).map_err(
                        |e| AsrPortError::new(AsrPortErrorKind::Runtime, e.to_string()),
                    )?),
                ),
                (
                    "in_cache3",
                    ort::session::SessionInputValue::from(Value::from_array(c3_view).map_err(
                        |e| AsrPortError::new(AsrPortErrorKind::Runtime, e.to_string()),
                    )?),
                ),
            ];

        let outputs = self.session.run(inputs).map_err(|e| {
            AsrPortError::new(
                AsrPortErrorKind::Runtime,
                format!("Batch FSMN VAD inference failed: {e}"),
            )
        })?;

        let logits_tensor = outputs.get("logits").ok_or_else(|| {
            AsrPortError::new(AsrPortErrorKind::Runtime, "Missing logits in FSMN output")
        })?;
        let (shape, logits_data) = logits_tensor.try_extract_raw_tensor::<f32>().map_err(|e| {
            AsrPortError::new(
                AsrPortErrorKind::Runtime,
                format!("Failed to extract FSMN logits: {e}"),
            )
        })?;

        let num_classes = if shape.len() >= 3 {
            shape[2] as usize
        } else {
            logits_data.len().checked_div(num_frames).unwrap_or(0)
        };

        if num_classes == 0 {
            return Err(AsrPortError::new(
                AsrPortErrorKind::Model,
                "FSMN output logits tensor has 0 classes",
            ));
        }
        if shape.len() >= 3 && shape[1] as usize != num_frames {
            return Err(AsrPortError::new(
                AsrPortErrorKind::Model,
                format!("Expected {num_frames} frames from FSMN, got {}", shape[1]),
            ));
        }

        if logits_data.len() < num_frames * num_classes {
            return Err(AsrPortError::new(
                AsrPortErrorKind::Runtime,
                format!(
                    "FSMN logits tensor truncated: expected {} elements, got {}",
                    num_frames * num_classes,
                    logits_data.len()
                ),
            ));
        }

        let mut probs = Vec::with_capacity(num_frames);
        for t in 0..num_frames {
            let start_s = t * FRAME_SHIFT_SAMPLES;
            let end_s = (start_s + FRAME_LENGTH_SAMPLES).min(samples.len());
            let is_silence = is_digital_silence(&samples[start_s..end_s]);

            let start = t * num_classes;
            let end = (t + 1) * num_classes;
            probs.push(Self::extract_prob_from_slice(
                &logits_data[start..end],
                is_silence,
            ));
        }
        drop(outputs);
        self.reset_state();
        Ok(probs)
    }
}

#[inline]
fn is_digital_silence(samples: &[f32]) -> bool {
    if samples.is_empty() {
        return true;
    }
    let sum_sq: f32 = samples.iter().map(|&s| s * s).sum();
    let rms = (sum_sq / samples.len() as f32).sqrt();
    rms < 1e-4
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_prob_multi_class_and_binary() {
        // Binary softmax
        assert!((FsmnVadRunner::extract_prob_from_slice(&[0.1, 0.9], false) - 0.9).abs() < 1e-4);
        assert!((FsmnVadRunner::extract_prob_from_slice(&[0.9, 0.1], false) - 0.1).abs() < 1e-4);

        // Digital silence override
        assert_eq!(
            FsmnVadRunner::extract_prob_from_slice(&[0.1, 0.9], true),
            0.0
        );

        // Multi-class (noise score at index 0)
        let mut multi = vec![0.0f32; 248];
        multi[0] = 0.05; // noise score 0.05 -> speech prob 0.95
        assert!((FsmnVadRunner::extract_prob_from_slice(&multi, false) - 0.95).abs() < 1e-4);

        multi[0] = 0.95; // noise score 0.95 -> speech prob 0.05
        assert!((FsmnVadRunner::extract_prob_from_slice(&multi, false) - 0.05).abs() < 1e-4);
    }
}
