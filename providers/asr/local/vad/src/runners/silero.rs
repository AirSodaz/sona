use super::VadModelRunner;
use ndarray::{Array3, ArrayView1, ArrayView2};
use ort::session::Session;
use ort::value::Value;
use sona_core::ports::asr::{AsrPortError, AsrPortErrorKind};
use std::path::Path;
use std::sync::Arc;
pub const DEFAULT_SILERO_WINDOW_SIZE: usize = 512; // 32ms @ 16kHz
pub const SILERO_CONTEXT_SIZE: usize = 64; // 4ms context @ 16kHz
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SileroVersion {
    V4,
    V5OrV6,
}

pub struct SileroVadRunner {
    session: Arc<Session>,
    version: SileroVersion,
    audio_input_name: String,
    sr_input_name: Option<String>,
    // V5 / V6 recurrent state [2, 1, 128]
    state: Array3<f32>,
    // V5 / V6 rolling context [64]
    context: Vec<f32>,
    // V4 recurrent states [2, 1, 64]
    h: Array3<f32>,
    c: Array3<f32>,
    window_size: usize,
}

impl SileroVadRunner {
    pub fn from_session(session: Arc<Session>) -> Result<Self, AsrPortError> {
        let mut audio_input_name = None;
        let mut sr_input_name = None;
        let mut has_h = false;
        let mut has_c = false;
        let mut has_state = false;

        for input in &session.inputs {
            match input.name.as_str() {
                "input" | "waveform" | "x" => audio_input_name = Some(input.name.clone()),
                "sr" => sr_input_name = Some(input.name.clone()),
                "state" => has_state = true,
                "h" => has_h = true,
                "c" => has_c = true,
                _ => {}
            }
        }

        let audio_input_name = audio_input_name.ok_or_else(|| {
            AsrPortError::new(
                AsrPortErrorKind::Model,
                "Unsupported Silero model: missing audio input node ('input', 'waveform', or 'x')",
            )
        })?;

        if !has_h && !has_state {
            return Err(AsrPortError::new(
                AsrPortErrorKind::Model,
                "Unsupported Silero model: missing recurrent state inputs ('state' or 'h')",
            ));
        }
        let has_output = session
            .outputs
            .iter()
            .any(|o| o.name == "output" || o.name == "prob");
        if !has_output {
            return Err(AsrPortError::new(
                AsrPortErrorKind::Model,
                "Unsupported Silero model: missing 'output' or 'prob' node in model outputs",
            ));
        }

        let version = if has_h {
            if !has_c {
                return Err(AsrPortError::new(
                    AsrPortErrorKind::Model,
                    "Unsupported Silero V4 model: found 'h' input but missing 'c' (cell state)",
                ));
            }
            let has_hn = session
                .outputs
                .iter()
                .any(|o| o.name == "hn" || o.name == "new_h");
            let has_cn = session
                .outputs
                .iter()
                .any(|o| o.name == "cn" || o.name == "new_c");
            if !has_hn || !has_cn {
                return Err(AsrPortError::new(
                    AsrPortErrorKind::Model,
                    "Unsupported Silero V4 model: missing 'hn' or 'cn' in model outputs",
                ));
            }
            SileroVersion::V4
        } else {
            let has_state_out = session
                .outputs
                .iter()
                .any(|o| o.name == "stateN" || o.name == "state");
            if !has_state_out {
                return Err(AsrPortError::new(
                    AsrPortErrorKind::Model,
                    "Unsupported Silero V5/V6 model: missing 'stateN' or 'state' in model outputs",
                ));
            }
            SileroVersion::V5OrV6
        };
        Ok(Self {
            session,
            version,
            audio_input_name,
            sr_input_name,
            state: Array3::zeros((2, 1, 128)),
            context: vec![0.0f32; SILERO_CONTEXT_SIZE],
            h: Array3::zeros((2, 1, 64)),
            c: Array3::zeros((2, 1, 64)),
            window_size: DEFAULT_SILERO_WINDOW_SIZE,
        })
    }

    pub fn from_file(path: &Path) -> Result<Self, AsrPortError> {
        let onnx_path = crate::shared::resolve_model_onnx_path(path)?;
        let session = crate::engine::get_or_create_session(&onnx_path)?;
        Self::from_session(session)
    }
}

impl VadModelRunner for SileroVadRunner {
    fn window_size(&self) -> usize {
        self.window_size
    }

    fn reset_state(&mut self) {
        self.state.fill(0.0);
        self.context.fill(0.0);
        self.h.fill(0.0);
        self.c.fill(0.0);
    }

    fn step(&mut self, chunk: &[f32]) -> Result<f32, AsrPortError> {
        if chunk.len() != self.window_size {
            return Err(AsrPortError::new(
                AsrPortErrorKind::Unsupported,
                format!(
                    "Silero step expected chunk length {}, got {}",
                    self.window_size,
                    chunk.len()
                ),
            ));
        }

        let sr_val = if self.sr_input_name.is_some() {
            let sr_arr = ArrayView1::from(&[crate::shared::SUPPORTED_SAMPLE_RATE as i64]);
            Some(Value::from_array(sr_arr).map_err(|e| {
                AsrPortError::new(
                    AsrPortErrorKind::Runtime,
                    format!("Failed to bind sample rate: {e}"),
                )
            })?)
        } else {
            None
        };

        match self.version {
            SileroVersion::V5OrV6 => {
                if self.window_size != DEFAULT_SILERO_WINDOW_SIZE {
                    return Err(AsrPortError::new(
                        AsrPortErrorKind::Unsupported,
                        format!(
                            "Silero V5/V6 requires window size {DEFAULT_SILERO_WINDOW_SIZE}, got {}",
                            self.window_size
                        ),
                    ));
                }
                let mut audio_buf = [0.0f32; SILERO_CONTEXT_SIZE + DEFAULT_SILERO_WINDOW_SIZE];
                audio_buf[..SILERO_CONTEXT_SIZE].copy_from_slice(&self.context);
                audio_buf[SILERO_CONTEXT_SIZE..SILERO_CONTEXT_SIZE + self.window_size]
                    .copy_from_slice(chunk);

                let audio_view = ArrayView2::from_shape(
                    (1, SILERO_CONTEXT_SIZE + self.window_size),
                    &audio_buf[..SILERO_CONTEXT_SIZE + self.window_size],
                )
                .map_err(|e| {
                    AsrPortError::new(
                        AsrPortErrorKind::Runtime,
                        format!("Failed to construct audio tensor view: {e}"),
                    )
                })?;
                let audio_val = Value::from_array(audio_view).map_err(|e| {
                    AsrPortError::new(
                        AsrPortErrorKind::Runtime,
                        format!("Failed to bind audio value: {e}"),
                    )
                })?;

                let state_val = Value::from_array(self.state.view()).map_err(|e| {
                    AsrPortError::new(
                        AsrPortErrorKind::Runtime,
                        format!("Failed to bind state: {e}"),
                    )
                })?;

                let mut inputs = vec![
                    (
                        self.audio_input_name.as_str(),
                        ort::session::SessionInputValue::from(audio_val),
                    ),
                    ("state", ort::session::SessionInputValue::from(state_val)),
                ];
                if let (Some(sr_name), Some(sr_v)) = (&self.sr_input_name, sr_val) {
                    inputs.push((
                        sr_name.as_str(),
                        ort::session::SessionInputValue::from(sr_v),
                    ));
                }

                let outputs = self.session.run(inputs).map_err(|e| {
                    AsrPortError::new(
                        AsrPortErrorKind::Runtime,
                        format!("Silero inference failed: {e}"),
                    )
                })?;

                let output_tensor = outputs
                    .get("output")
                    .or_else(|| outputs.get("prob"))
                    .ok_or_else(|| {
                        AsrPortError::new(
                            AsrPortErrorKind::Runtime,
                            "Missing 'output' or 'prob' in Silero outputs",
                        )
                    })?;
                let raw_prob = {
                    let (_, output_data) =
                        output_tensor.try_extract_raw_tensor::<f32>().map_err(|e| {
                            AsrPortError::new(
                                AsrPortErrorKind::Runtime,
                                format!("Failed to extract output probability: {e}"),
                            )
                        })?;
                    output_data.first().copied().ok_or_else(|| {
                        AsrPortError::new(
                            AsrPortErrorKind::Runtime,
                            "Silero 'output' tensor contains 0 elements",
                        )
                    })?
                };
                if !raw_prob.is_finite() {
                    drop(outputs);
                    self.reset_state();
                    return Ok(0.0);
                }
                let prob = raw_prob.clamp(0.0, 1.0);
                let next_state_tensor = outputs
                    .get("stateN")
                    .or_else(|| outputs.get("state"))
                    .ok_or_else(|| {
                        AsrPortError::new(
                            AsrPortErrorKind::Runtime,
                            "Missing 'stateN' or 'state' in Silero outputs",
                        )
                    })?;
                let (_, data) = next_state_tensor
                    .try_extract_raw_tensor::<f32>()
                    .map_err(|e| {
                        AsrPortError::new(
                            AsrPortErrorKind::Runtime,
                            format!("Failed to extract state tensor: {e}"),
                        )
                    })?;
                if data.len() != 2 * 128 {
                    let len = data.len();
                    drop(outputs);
                    self.reset_state();
                    return Err(AsrPortError::new(
                        AsrPortErrorKind::Runtime,
                        format!("Expected 256 state elements, got {len}"),
                    ));
                }
                if data.iter().any(|v| !v.is_finite()) {
                    drop(outputs);
                    self.reset_state();
                    return Ok(0.0);
                }
                if let Some(slice) = self.state.as_slice_mut() {
                    slice.copy_from_slice(data);
                } else {
                    self.state = Array3::from_shape_vec((2, 1, 128), data.to_vec())
                        .map_err(|e| AsrPortError::new(AsrPortErrorKind::Runtime, e.to_string()))?;
                }

                self.context
                    .copy_from_slice(&chunk[self.window_size - SILERO_CONTEXT_SIZE..]);

                Ok(prob)
            }
            SileroVersion::V4 => {
                let audio_view =
                    ArrayView2::from_shape((1, self.window_size), chunk).map_err(|e| {
                        AsrPortError::new(
                            AsrPortErrorKind::Runtime,
                            format!("Failed to construct audio tensor view: {e}"),
                        )
                    })?;
                let audio_val = Value::from_array(audio_view).map_err(|e| {
                    AsrPortError::new(
                        AsrPortErrorKind::Runtime,
                        format!("Failed to bind audio value: {e}"),
                    )
                })?;

                let h_val = Value::from_array(self.h.view()).map_err(|e| {
                    AsrPortError::new(AsrPortErrorKind::Runtime, format!("Failed to bind h: {e}"))
                })?;
                let c_val = Value::from_array(self.c.view()).map_err(|e| {
                    AsrPortError::new(AsrPortErrorKind::Runtime, format!("Failed to bind c: {e}"))
                })?;

                let mut inputs = vec![
                    (
                        self.audio_input_name.as_str(),
                        ort::session::SessionInputValue::from(audio_val),
                    ),
                    ("h", ort::session::SessionInputValue::from(h_val)),
                    ("c", ort::session::SessionInputValue::from(c_val)),
                ];
                if let (Some(sr_name), Some(sr_v)) = (&self.sr_input_name, sr_val) {
                    inputs.push((
                        sr_name.as_str(),
                        ort::session::SessionInputValue::from(sr_v),
                    ));
                }

                let outputs = self.session.run(inputs).map_err(|e| {
                    AsrPortError::new(
                        AsrPortErrorKind::Runtime,
                        format!("Silero V4 inference failed: {e}"),
                    )
                })?;

                let output_tensor = outputs
                    .get("output")
                    .or_else(|| outputs.get("prob"))
                    .ok_or_else(|| {
                        AsrPortError::new(
                            AsrPortErrorKind::Runtime,
                            "Missing 'output' or 'prob' in Silero outputs",
                        )
                    })?;
                let raw_prob = {
                    let (_, output_data) =
                        output_tensor.try_extract_raw_tensor::<f32>().map_err(|e| {
                            AsrPortError::new(
                                AsrPortErrorKind::Runtime,
                                format!("Failed to extract output probability: {e}"),
                            )
                        })?;
                    output_data.first().copied().ok_or_else(|| {
                        AsrPortError::new(
                            AsrPortErrorKind::Runtime,
                            "Silero 'output' tensor contains 0 elements",
                        )
                    })?
                };
                if !raw_prob.is_finite() {
                    drop(outputs);
                    self.reset_state();
                    return Ok(0.0);
                }
                let prob = raw_prob.clamp(0.0, 1.0);
                let hn_tensor = outputs
                    .get("hn")
                    .or_else(|| outputs.get("new_h"))
                    .ok_or_else(|| {
                        AsrPortError::new(
                            AsrPortErrorKind::Runtime,
                            "Missing 'hn' or 'new_h' in Silero V4 outputs",
                        )
                    })?;
                let (_, h_data) = hn_tensor.try_extract_raw_tensor::<f32>().map_err(|e| {
                    AsrPortError::new(
                        AsrPortErrorKind::Runtime,
                        format!("Failed to extract hn: {e}"),
                    )
                })?;
                if h_data.len() != 2 * 64 {
                    let len = h_data.len();
                    drop(outputs);
                    self.reset_state();
                    return Err(AsrPortError::new(
                        AsrPortErrorKind::Runtime,
                        format!("Expected 128 hn elements, got {len}"),
                    ));
                }
                if h_data.iter().any(|v| !v.is_finite()) {
                    drop(outputs);
                    self.reset_state();
                    return Ok(0.0);
                }

                let cn_tensor = outputs
                    .get("cn")
                    .or_else(|| outputs.get("new_c"))
                    .ok_or_else(|| {
                        AsrPortError::new(
                            AsrPortErrorKind::Runtime,
                            "Missing 'cn' or 'new_c' in Silero V4 outputs",
                        )
                    })?;
                let (_, c_data) = cn_tensor.try_extract_raw_tensor::<f32>().map_err(|e| {
                    AsrPortError::new(
                        AsrPortErrorKind::Runtime,
                        format!("Failed to extract cn: {e}"),
                    )
                })?;
                if c_data.len() != 2 * 64 {
                    let len = c_data.len();
                    drop(outputs);
                    self.reset_state();
                    return Err(AsrPortError::new(
                        AsrPortErrorKind::Runtime,
                        format!("Expected 128 cn elements, got {len}"),
                    ));
                }
                if c_data.iter().any(|v| !v.is_finite()) {
                    drop(outputs);
                    self.reset_state();
                    return Ok(0.0);
                }
                if let Some(slice) = self.h.as_slice_mut() {
                    slice.copy_from_slice(h_data);
                } else {
                    self.h = Array3::from_shape_vec((2, 1, 64), h_data.to_vec())
                        .map_err(|e| AsrPortError::new(AsrPortErrorKind::Runtime, e.to_string()))?;
                }
                if let Some(slice) = self.c.as_slice_mut() {
                    slice.copy_from_slice(c_data);
                } else {
                    self.c = Array3::from_shape_vec((2, 1, 64), c_data.to_vec())
                        .map_err(|e| AsrPortError::new(AsrPortErrorKind::Runtime, e.to_string()))?;
                }
                Ok(prob)
            }
        }
    }

    fn predict_probs_batch(&mut self, samples: &[f32]) -> Result<Vec<f32>, AsrPortError> {
        self.reset_state();

        let remainder = samples.len() % self.window_size;
        let num_chunks = samples.len() / self.window_size;
        let total_chunks = if remainder > 0 {
            num_chunks + 1
        } else {
            num_chunks
        };
        let mut probs = Vec::with_capacity(total_chunks);

        for i in 0..num_chunks {
            let start = i * self.window_size;
            let chunk = &samples[start..start + self.window_size];
            let prob = self.step(chunk).inspect_err(|_| self.reset_state())?;
            probs.push(prob);
        }

        // Process residual samples if non-empty by zero-padding to window_size
        if remainder > 0 {
            let mut padded = vec![0.0f32; self.window_size];
            let start = num_chunks * self.window_size;
            padded[..remainder].copy_from_slice(&samples[start..]);
            let prob = self.step(&padded).inspect_err(|_| self.reset_state())?;
            probs.push(prob);
        }
        self.reset_state();
        Ok(probs)
    }
}
