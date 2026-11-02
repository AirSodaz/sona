use super::VadModelRunner;
use ndarray::{Array1, Array2, Array3};
use ort::session::Session;
use ort::session::builder::GraphOptimizationLevel;
use ort::value::Value;
use sona_core::ports::asr::{AsrPortError, AsrPortErrorKind};
use std::path::Path;

pub const DEFAULT_SILERO_WINDOW_SIZE: usize = 512; // 32ms @ 16kHz

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SileroVersion {
    V4,
    V5OrV6,
}

pub struct SileroVadRunner {
    session: Session,
    version: SileroVersion,
    audio_input_name: String,
    sr_input_name: Option<String>,
    // V5 / V6 recurrent state [2, 1, 128]
    state: Array3<f32>,
    // V4 recurrent states [2, 1, 64]
    h: Array3<f32>,
    c: Array3<f32>,
    window_size: usize,
}

impl SileroVadRunner {
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
                    format!("Failed to load Silero VAD from {}: {e}", path.display()),
                )
            })?;

        let mut audio_input_name = "input".to_string();
        let mut sr_input_name = None;
        let mut has_h = false;
        let mut _has_state = false;

        for input in &session.inputs {
            match input.name.as_str() {
                "input" | "waveform" => audio_input_name = input.name.clone(),
                "sr" => sr_input_name = Some(input.name.clone()),
                "state" => _has_state = true,
                "h" => has_h = true,
                _ => {}
            }
        }

        let version = if has_h {
            SileroVersion::V4
        } else {
            SileroVersion::V5OrV6
        };

        Ok(Self {
            session,
            version,
            audio_input_name,
            sr_input_name,
            state: Array3::zeros((2, 1, 128)),
            h: Array3::zeros((2, 1, 64)),
            c: Array3::zeros((2, 1, 64)),
            window_size: DEFAULT_SILERO_WINDOW_SIZE,
        })
    }
}

impl VadModelRunner for SileroVadRunner {
    fn window_size(&self) -> usize {
        self.window_size
    }

    fn reset_state(&mut self) {
        self.state.fill(0.0);
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

        let audio_arr =
            Array2::from_shape_vec((1, self.window_size), chunk.to_vec()).map_err(|e| {
                AsrPortError::new(
                    AsrPortErrorKind::Runtime,
                    format!("Failed to construct audio tensor: {e}"),
                )
            })?;
        let audio_val = Value::from_array(audio_arr).map_err(|e| {
            AsrPortError::new(
                AsrPortErrorKind::Runtime,
                format!("Failed to bind audio value: {e}"),
            )
        })?;

        let sr_val = if self.sr_input_name.is_some() {
            let sr_arr = Array1::from_vec(vec![16_000i64]);
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
                let state_val = Value::from_array(self.state.clone()).map_err(|e| {
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

                let output_tensor = outputs.get("output").ok_or_else(|| {
                    AsrPortError::new(
                        AsrPortErrorKind::Runtime,
                        "Missing 'output' in Silero outputs",
                    )
                })?;
                let (_, output_data) =
                    output_tensor.try_extract_raw_tensor::<f32>().map_err(|e| {
                        AsrPortError::new(
                            AsrPortErrorKind::Runtime,
                            format!("Failed to extract output probability: {e}"),
                        )
                    })?;
                let prob = output_data.first().copied().unwrap_or(0.0);

                if let Some(next_state_tensor) =
                    outputs.get("stateN").or_else(|| outputs.get("state"))
                {
                    let (_shape, data) = next_state_tensor
                        .try_extract_raw_tensor::<f32>()
                        .map_err(|e| {
                            AsrPortError::new(
                                AsrPortErrorKind::Runtime,
                                format!("Failed to extract stateN: {e}"),
                            )
                        })?;
                    if data.len() == 2 * 128
                        && let Ok(new_state) = Array3::from_shape_vec((2, 1, 128), data.to_vec())
                    {
                        self.state = new_state;
                    }
                }

                Ok(prob)
            }
            SileroVersion::V4 => {
                let h_val = Value::from_array(self.h.clone()).map_err(|e| {
                    AsrPortError::new(AsrPortErrorKind::Runtime, format!("Failed to bind h: {e}"))
                })?;
                let c_val = Value::from_array(self.c.clone()).map_err(|e| {
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

                let output_tensor = outputs.get("output").ok_or_else(|| {
                    AsrPortError::new(
                        AsrPortErrorKind::Runtime,
                        "Missing 'output' in Silero outputs",
                    )
                })?;
                let (_, output_data) =
                    output_tensor.try_extract_raw_tensor::<f32>().map_err(|e| {
                        AsrPortError::new(
                            AsrPortErrorKind::Runtime,
                            format!("Failed to extract output probability: {e}"),
                        )
                    })?;
                let prob = output_data.first().copied().unwrap_or(0.0);

                if let Some(hn_tensor) = outputs.get("hn")
                    && let Ok((_, data)) = hn_tensor.try_extract_raw_tensor::<f32>()
                    && let Ok(new_h) = Array3::from_shape_vec((2, 1, 64), data.to_vec())
                {
                    self.h = new_h;
                }
                if let Some(cn_tensor) = outputs.get("cn")
                    && let Ok((_, data)) = cn_tensor.try_extract_raw_tensor::<f32>()
                    && let Ok(new_c) = Array3::from_shape_vec((2, 1, 64), data.to_vec())
                {
                    self.c = new_c;
                }

                Ok(prob)
            }
        }
    }

    fn predict_probs_batch(&mut self, samples: &[f32]) -> Result<Vec<f32>, AsrPortError> {
        self.reset_state();

        let num_chunks = samples.len() / self.window_size;
        let mut probs = Vec::with_capacity(num_chunks);

        for i in 0..num_chunks {
            let start = i * self.window_size;
            let chunk = &samples[start..start + self.window_size];
            let prob = self.step(chunk)?;
            probs.push(prob);
        }

        // Process residual samples if non-empty by zero-padding to window_size
        let remainder = samples.len() % self.window_size;
        if remainder > 0 {
            let mut padded = vec![0.0f32; self.window_size];
            let start = num_chunks * self.window_size;
            padded[..remainder].copy_from_slice(&samples[start..]);
            let prob = self.step(&padded)?;
            probs.push(prob);
        }

        Ok(probs)
    }
}
