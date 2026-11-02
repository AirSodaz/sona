use super::VadModelRunner;
use crate::fbank::{
    CmvnParams, FRAME_LENGTH_SAMPLES, FRAME_SHIFT_SAMPLES, FbankExtractor, apply_lfr_and_cmvn,
};
use ndarray::{Array4, ArrayD, IxDyn};
use ort::session::Session;
use ort::session::builder::GraphOptimizationLevel;
use ort::value::Value;
use sona_core::ports::asr::{AsrPortError, AsrPortErrorKind};
use std::collections::VecDeque;
use std::path::Path;

const CACHE_SHAPE: [usize; 4] = [1, 128, 19, 1];
const CACHE_LEN: usize = 128 * 19;

pub struct FsmnVadRunner {
    session: Session,
    fbank: FbankExtractor,
    cmvn: CmvnParams,
    // Caches for 4 FSMN layers
    cache0: Vec<f32>,
    cache1: Vec<f32>,
    cache2: Vec<f32>,
    cache3: Vec<f32>,
    // Streaming sample buffer
    sample_buffer: Vec<f32>,
    // Streaming recent 80-dim fbank frame history (up to 4 past frames for LFR context)
    fbank_history: VecDeque<[f32; 80]>,
    window_size: usize,
}

impl FsmnVadRunner {
    pub fn from_file(path: &Path) -> Result<Self, AsrPortError> {
        let session = Session::builder()
            .map_err(|e| AsrPortError::new(AsrPortErrorKind::Model, e.to_string()))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| AsrPortError::new(AsrPortErrorKind::Model, e.to_string()))?
            .with_intra_threads(1)
            .map_err(|e| AsrPortError::new(AsrPortErrorKind::Model, e.to_string()))?
            .commit_from_file(path)
            .map_err(|e| {
                AsrPortError::new(
                    AsrPortErrorKind::Model,
                    format!("Failed to load FSMN VAD from {}: {e}", path.display()),
                )
            })?;

        // Attempt to load external am.mvn / vad.mvn from the same directory if present
        let mut cmvn = CmvnParams::default();
        if let Some(parent) = path.parent() {
            let candidates = ["am.mvn", "vad.mvn"];
            for candidate in &candidates {
                let p = parent.join(candidate);
                if p.is_file()
                    && let Ok(loaded) = CmvnParams::load_am_mvn(&p)
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
            sample_buffer: Vec::with_capacity(1600),
            fbank_history: VecDeque::with_capacity(5),
            window_size: FRAME_SHIFT_SAMPLES, // 160 samples (10ms) hop
        })
    }

    fn extract_prob_from_slice(slice: &[f32], is_silence_audio: bool) -> f32 {
        if is_silence_audio {
            return 0.0;
        }

        if slice.len() == 2 {
            let l0 = slice[0];
            let l1 = slice[1];
            if (l0 + l1 - 1.0).abs() < 1e-3 {
                l1.clamp(0.0, 1.0)
            } else {
                let max_l = l0.max(l1);
                let e0 = (l0 - max_l).exp();
                let e1 = (l1 - max_l).exp();
                (e1 / (e0 + e1)).clamp(0.0, 1.0)
            }
        } else if !slice.is_empty() {
            let noise_score = slice[0];
            let speech_score = 1.0 - noise_score;
            let margin = speech_score - noise_score;
            if margin <= 0.0 {
                0.0
            } else {
                (margin / 1.2).clamp(0.0, 1.0)
            }
        } else {
            0.0
        }
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
        self.sample_buffer.clear();
        self.fbank_history.clear();
    }

    fn step(&mut self, chunk: &[f32]) -> Result<f32, AsrPortError> {
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

        // 2. Build 400-dim LFR stacked feature using historical frames
        let mut stacked_400 = [0.0f32; 400];
        // lfr_m = 5, left_pad = 2
        // If history is less than 2, pad with current frame
        for m in 0..5 {
            let frame_ref = match m {
                0 => self.fbank_history.front().unwrap_or(&frame_80),
                1 => self.fbank_history.get(1).unwrap_or(&frame_80),
                2 => self.fbank_history.get(2).unwrap_or(&frame_80),
                3 => self.fbank_history.get(3).unwrap_or(&frame_80),
                _ => &frame_80,
            };

            let dst = m * 80;
            for b in 0..80 {
                stacked_400[dst + b] = (frame_ref[b] + self.cmvn.shifts[b]) * self.cmvn.rescales[b];
            }
        }

        if self.fbank_history.len() >= 4 {
            self.fbank_history.pop_front();
        }
        self.fbank_history.push_back(frame_80);

        // 3. Bind ONNX inputs: speech [1, 1, 400] and in_cache0..3 [1, 128, 19, 1]
        let speech_arr = ArrayD::from_shape_vec(IxDyn(&[1, 1, 400]), stacked_400.to_vec())
            .map_err(|e| {
                AsrPortError::new(
                    AsrPortErrorKind::Runtime,
                    format!("Failed to build FSMN speech tensor: {e}"),
                )
            })?;
        let speech_val = Value::from_array(speech_arr).map_err(|e| {
            AsrPortError::new(
                AsrPortErrorKind::Runtime,
                format!("Failed to bind FSMN speech input: {e}"),
            )
        })?;

        let c0_arr = Array4::from_shape_vec(CACHE_SHAPE, self.cache0.clone())
            .map_err(|e| AsrPortError::new(AsrPortErrorKind::Runtime, e.to_string()))?;
        let c1_arr = Array4::from_shape_vec(CACHE_SHAPE, self.cache1.clone())
            .map_err(|e| AsrPortError::new(AsrPortErrorKind::Runtime, e.to_string()))?;
        let c2_arr = Array4::from_shape_vec(CACHE_SHAPE, self.cache2.clone())
            .map_err(|e| AsrPortError::new(AsrPortErrorKind::Runtime, e.to_string()))?;
        let c3_arr = Array4::from_shape_vec(CACHE_SHAPE, self.cache3.clone())
            .map_err(|e| AsrPortError::new(AsrPortErrorKind::Runtime, e.to_string()))?;
        let inputs =
            vec![
                ("speech", ort::session::SessionInputValue::from(speech_val)),
                (
                    "in_cache0",
                    ort::session::SessionInputValue::from(Value::from_array(c0_arr).map_err(
                        |e| AsrPortError::new(AsrPortErrorKind::Runtime, e.to_string()),
                    )?),
                ),
                (
                    "in_cache1",
                    ort::session::SessionInputValue::from(Value::from_array(c1_arr).map_err(
                        |e| AsrPortError::new(AsrPortErrorKind::Runtime, e.to_string()),
                    )?),
                ),
                (
                    "in_cache2",
                    ort::session::SessionInputValue::from(Value::from_array(c2_arr).map_err(
                        |e| AsrPortError::new(AsrPortErrorKind::Runtime, e.to_string()),
                    )?),
                ),
                (
                    "in_cache3",
                    ort::session::SessionInputValue::from(Value::from_array(c3_arr).map_err(
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

        let prob = Self::extract_prob_from_slice(logits_data, is_silence);

        // Update caches
        if let Some(t) = outputs.get("out_cache0")
            && let Ok((_, data)) = t.try_extract_raw_tensor::<f32>()
            && data.len() == CACHE_LEN
        {
            self.cache0.copy_from_slice(data);
        }
        if let Some(t) = outputs.get("out_cache1")
            && let Ok((_, data)) = t.try_extract_raw_tensor::<f32>()
            && data.len() == CACHE_LEN
        {
            self.cache1.copy_from_slice(data);
        }
        if let Some(t) = outputs.get("out_cache2")
            && let Ok((_, data)) = t.try_extract_raw_tensor::<f32>()
            && data.len() == CACHE_LEN
        {
            self.cache2.copy_from_slice(data);
        }
        if let Some(t) = outputs.get("out_cache3")
            && let Ok((_, data)) = t.try_extract_raw_tensor::<f32>()
            && data.len() == CACHE_LEN
        {
            self.cache3.copy_from_slice(data);
        }

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

        // Initialize zero caches
        let c0_arr = Array4::from_shape_vec(CACHE_SHAPE, vec![0.0f32; CACHE_LEN])
            .map_err(|e| AsrPortError::new(AsrPortErrorKind::Runtime, e.to_string()))?;
        let c1_arr = Array4::from_shape_vec(CACHE_SHAPE, vec![0.0f32; CACHE_LEN])
            .map_err(|e| AsrPortError::new(AsrPortErrorKind::Runtime, e.to_string()))?;
        let c2_arr = Array4::from_shape_vec(CACHE_SHAPE, vec![0.0f32; CACHE_LEN])
            .map_err(|e| AsrPortError::new(AsrPortErrorKind::Runtime, e.to_string()))?;
        let c3_arr = Array4::from_shape_vec(CACHE_SHAPE, vec![0.0f32; CACHE_LEN])
            .map_err(|e| AsrPortError::new(AsrPortErrorKind::Runtime, e.to_string()))?;

        let inputs =
            vec![
                ("speech", ort::session::SessionInputValue::from(speech_val)),
                (
                    "in_cache0",
                    ort::session::SessionInputValue::from(Value::from_array(c0_arr).map_err(
                        |e| AsrPortError::new(AsrPortErrorKind::Runtime, e.to_string()),
                    )?),
                ),
                (
                    "in_cache1",
                    ort::session::SessionInputValue::from(Value::from_array(c1_arr).map_err(
                        |e| AsrPortError::new(AsrPortErrorKind::Runtime, e.to_string()),
                    )?),
                ),
                (
                    "in_cache2",
                    ort::session::SessionInputValue::from(Value::from_array(c2_arr).map_err(
                        |e| AsrPortError::new(AsrPortErrorKind::Runtime, e.to_string()),
                    )?),
                ),
                (
                    "in_cache3",
                    ort::session::SessionInputValue::from(Value::from_array(c3_arr).map_err(
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
            logits_data.len() / num_frames
        };

        let mut probs = Vec::with_capacity(num_frames);
        for t in 0..num_frames {
            let start_s = t * FRAME_SHIFT_SAMPLES;
            let end_s = (start_s + FRAME_LENGTH_SAMPLES).min(samples.len());
            let is_silence = is_digital_silence(&samples[start_s..end_s]);

            let start = t * num_classes;
            let end = (t + 1) * num_classes;
            if end <= logits_data.len() {
                probs.push(Self::extract_prob_from_slice(
                    &logits_data[start..end],
                    is_silence,
                ));
            }
        }

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
