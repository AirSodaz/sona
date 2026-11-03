use sona_core::ports::asr::{AsrPortError, AsrPortErrorKind};
use std::f32::consts::PI;
use std::path::Path;

pub const SAMPLE_RATE: u32 = 16_000;
pub const FRAME_LENGTH_SAMPLES: usize = 400; // 25ms @ 16kHz
pub const FRAME_SHIFT_SAMPLES: usize = 160; // 10ms @ 16kHz
pub const PREEMPHASIS: f32 = 0.97;
pub const FFT_SIZE: usize = 512;
pub const NUM_MEL_BINS: usize = 80;
pub const LOW_FREQ: f32 = 20.0;
pub const HIGH_FREQ: f32 = 8000.0;

/// Embedded CMVN shifts for speech_fsmn_vad_zh-cn-16k-common.
#[allow(clippy::excessive_precision)]
pub const FSMN_CMVN_SHIFTS: [f32; 80] = [
    -8.311879_f32,
    -8.600912_f32,
    -9.615928_f32,
    -10.435950_f32,
    -11.212920_f32,
    -11.883330_f32,
    -12.362430_f32,
    -12.637060_f32,
    -12.881800_f32,
    -12.830660_f32,
    -12.891030_f32,
    -12.956660_f32,
    -13.197630_f32,
    -13.405980_f32,
    -13.491130_f32,
    -13.554600_f32,
    -13.556390_f32,
    -13.519150_f32,
    -13.682840_f32,
    -13.532890_f32,
    -13.421070_f32,
    -13.655190_f32,
    -13.507130_f32,
    -13.752510_f32,
    -13.767150_f32,
    -13.874080_f32,
    -13.731090_f32,
    -13.704120_f32,
    -13.560730_f32,
    -13.534880_f32,
    -13.548950_f32,
    -13.562280_f32,
    -13.594080_f32,
    -13.620470_f32,
    -13.641980_f32,
    -13.661090_f32,
    -13.626690_f32,
    -13.582970_f32,
    -13.573870_f32,
    -13.473900_f32,
    -13.530630_f32,
    -13.483480_f32,
    -13.610470_f32,
    -13.647160_f32,
    -13.715460_f32,
    -13.791840_f32,
    -13.906140_f32,
    -14.030980_f32,
    -14.182050_f32,
    -14.358810_f32,
    -14.484190_f32,
    -14.601720_f32,
    -14.705910_f32,
    -14.833620_f32,
    -14.921220_f32,
    -15.006220_f32,
    -15.051220_f32,
    -15.031190_f32,
    -14.990280_f32,
    -14.923020_f32,
    -14.869270_f32,
    -14.826910_f32,
    -14.797200_f32,
    -14.769090_f32,
    -14.713560_f32,
    -14.612770_f32,
    -14.516960_f32,
    -14.422520_f32,
    -14.364050_f32,
    -14.304510_f32,
    -14.231610_f32,
    -14.198510_f32,
    -14.166330_f32,
    -14.156490_f32,
    -14.105040_f32,
    -13.995180_f32,
    -13.795620_f32,
    -13.399600_f32,
    -12.776700_f32,
    -11.712080_f32,
];

/// Embedded CMVN rescales for speech_fsmn_vad_zh-cn-16k-common.
#[allow(clippy::excessive_precision)]
pub const FSMN_CMVN_RESCALES: [f32; 80] = [
    0.155775_f32,
    0.154484_f32,
    0.152738_f32,
    0.151872_f32,
    0.150603_f32,
    0.148926_f32,
    0.147067_f32,
    0.144706_f32,
    0.143631_f32,
    0.144357_f32,
    0.145185_f32,
    0.145516_f32,
    0.145282_f32,
    0.144572_f32,
    0.143920_f32,
    0.143587_f32,
    0.143602_f32,
    0.143878_f32,
    0.144209_f32,
    0.144884_f32,
    0.145476_f32,
    0.145663_f32,
    0.146268_f32,
    0.146739_f32,
    0.147272_f32,
    0.147664_f32,
    0.148091_f32,
    0.148374_f32,
    0.148884_f32,
    0.149364_f32,
    0.149709_f32,
    0.150038_f32,
    0.150292_f32,
    0.150539_f32,
    0.150679_f32,
    0.150710_f32,
    0.150599_f32,
    0.150544_f32,
    0.150594_f32,
    0.150813_f32,
    0.150957_f32,
    0.151240_f32,
    0.151462_f32,
    0.151619_f32,
    0.151616_f32,
    0.151556_f32,
    0.151497_f32,
    0.151398_f32,
    0.151261_f32,
    0.151076_f32,
    0.151060_f32,
    0.151043_f32,
    0.151077_f32,
    0.151117_f32,
    0.151192_f32,
    0.151023_f32,
    0.150805_f32,
    0.150588_f32,
    0.150349_f32,
    0.150237_f32,
    0.150173_f32,
    0.150076_f32,
    0.150006_f32,
    0.149978_f32,
    0.150057_f32,
    0.150266_f32,
    0.150469_f32,
    0.150533_f32,
    0.150551_f32,
    0.150533_f32,
    0.150427_f32,
    0.150244_f32,
    0.149967_f32,
    0.149712_f32,
    0.149466_f32,
    0.149310_f32,
    0.149368_f32,
    0.149550_f32,
    0.149974_f32,
    0.150965_f32,
];

/// Cepstral Mean and Variance Normalization parameters.
#[derive(Clone, Debug)]
pub struct CmvnParams {
    pub shifts: [f32; 80],
    pub rescales: [f32; 80],
}

impl Default for CmvnParams {
    fn default() -> Self {
        Self {
            shifts: FSMN_CMVN_SHIFTS,
            rescales: FSMN_CMVN_RESCALES,
        }
    }
}

impl CmvnParams {
    pub fn apply(&self, feats: &mut [f32; 80]) {
        for (i, feat) in feats.iter_mut().enumerate() {
            *feat = (*feat + self.shifts[i]) * self.rescales[i];
        }
    }

    /// Loads CMVN parameters from an `am.mvn` or `vad.mvn` Kaldi-style text file.
    pub fn load_am_mvn(path: &Path) -> Result<Self, AsrPortError> {
        let content = std::fs::read_to_string(path).map_err(|err| {
            AsrPortError::new(
                AsrPortErrorKind::Model,
                format!("Failed to read CMVN file at {}: {err}", path.display()),
            )
        })?;

        let mut shifts = [0.0f32; 80];
        let mut rescales = [1.0f32; 80];
        let mut found_shift = false;
        let mut found_rescale = false;

        let lines: Vec<&str> = content.lines().collect();
        let mut i = 0;
        while i < lines.len() {
            let line = lines[i].trim();
            if line.starts_with("<AddShift>") {
                let (vals, next_i) = parse_kaldi_vector_multiline(&lines, i)?;
                if vals.len() >= 80 {
                    shifts.copy_from_slice(&vals[..80]);
                    found_shift = true;
                }
                i = next_i;
                continue;
            } else if line.starts_with("<Rescale>") {
                let (vals, next_i) = parse_kaldi_vector_multiline(&lines, i)?;
                if vals.len() >= 80 {
                    rescales.copy_from_slice(&vals[..80]);
                    found_rescale = true;
                }
                i = next_i;
                continue;
            }
            i += 1;
        }
        if !found_shift || !found_rescale {
            return Err(AsrPortError::new(
                AsrPortErrorKind::Model,
                format!("Invalid CMVN file structure at {}", path.display()),
            ));
        }

        Ok(Self { shifts, rescales })
    }
}

fn parse_kaldi_vector_multiline(
    lines: &[&str],
    start_idx: usize,
) -> Result<(Vec<f32>, usize), AsrPortError> {
    let mut out = Vec::with_capacity(80);
    let mut collecting = false;
    let mut current_idx = start_idx;

    while current_idx < lines.len() {
        let line = lines[current_idx].trim();
        let start = if !collecting {
            if let Some(pos) = line.find('[') {
                collecting = true;
                pos + 1
            } else {
                current_idx += 1;
                continue;
            }
        } else {
            0
        };

        if collecting {
            let end = line[start..].find(']').map(|p| start + p);
            let slice = match end {
                Some(end_pos) => &line[start..end_pos],
                None => &line[start..],
            };

            for token in slice.split_whitespace() {
                let v = token.parse::<f32>().map_err(|err| {
                    AsrPortError::new(
                        AsrPortErrorKind::Model,
                        format!("Failed to parse CMVN float token '{token}': {err}"),
                    )
                })?;
                out.push(v);
            }

            if end.is_some() {
                return Ok((out, current_idx + 1));
            }
        }
        current_idx += 1;
    }

    if collecting {
        Err(AsrPortError::new(
            AsrPortErrorKind::Model,
            "Unclosed '[' in Kaldi vector",
        ))
    } else {
        Err(AsrPortError::new(
            AsrPortErrorKind::Model,
            "Missing '[' in Kaldi vector",
        ))
    }
}

/// Sparse triangular mel filter definition.
#[derive(Clone, Debug)]
struct MelFilter {
    offset: usize,
    weights: Vec<f32>,
}

/// 80-dim Log-Mel Filterbank Extractor matching Kaldi and FunASR standard.
#[derive(Clone, Debug)]
pub struct FbankExtractor {
    window: [f32; FRAME_LENGTH_SAMPLES],
    filters: Vec<MelFilter>,
    bit_rev: [u16; FFT_SIZE],
}

impl Default for FbankExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl FbankExtractor {
    pub fn new() -> Self {
        // 1. Precompute Hamming window
        let mut window = [0.0f32; FRAME_LENGTH_SAMPLES];
        for (n, w) in window.iter_mut().enumerate() {
            *w = 0.54 - 0.46 * (2.0 * PI * n as f32 / (FRAME_LENGTH_SAMPLES as f32 - 1.0)).cos();
        }

        // 2. Precompute triangular Mel filterbanks matching Kaldi mel-computations
        let low_mel = mel_scale(LOW_FREQ);
        let high_mel = mel_scale(HIGH_FREQ);
        let delta_mel = (high_mel - low_mel) / (NUM_MEL_BINS as f32 + 1.0);

        let mut filters = Vec::with_capacity(NUM_MEL_BINS);
        for bin in 0..NUM_MEL_BINS {
            let left_mel = low_mel + bin as f32 * delta_mel;
            let center_mel = low_mel + (bin as f32 + 1.0) * delta_mel;
            let right_mel = low_mel + (bin as f32 + 2.0) * delta_mel;

            let mut bin_weights = Vec::new();
            let mut first_idx = None;

            for k in 0..=FFT_SIZE / 2 {
                let freq = k as f32 * SAMPLE_RATE as f32 / FFT_SIZE as f32;
                let mel = mel_scale(freq);

                if mel > left_mel && mel < right_mel {
                    let weight = if mel <= center_mel {
                        (mel - left_mel) / (center_mel - left_mel)
                    } else {
                        (right_mel - mel) / (right_mel - center_mel)
                    };

                    if first_idx.is_none() {
                        first_idx = Some(k);
                    }
                    bin_weights.push(weight);
                } else if first_idx.is_some() {
                    // Filter is triangular and contiguous; exit loop once past right_mel
                    break;
                }
            }

            filters.push(MelFilter {
                offset: first_idx.unwrap_or(0),
                weights: bin_weights,
            });
        }

        // 3. Precompute bit reversal table for 512-point FFT
        let mut bit_rev = [0u16; FFT_SIZE];
        for (i, slot) in bit_rev.iter_mut().enumerate() {
            let mut r = 0u16;
            let mut val = i as u16;
            for _ in 0..9 {
                r = (r << 1) | (val & 1);
                val >>= 1;
            }
            *slot = r;
        }

        Self {
            window,
            filters,
            bit_rev,
        }
    }

    /// Extracts 80 log-mel filterbank energies from a single 400-sample (25ms) frame.
    pub fn extract_frame(&self, window_samples: &[f32]) -> [f32; NUM_MEL_BINS] {
        assert_eq!(
            window_samples.len(),
            FRAME_LENGTH_SAMPLES,
            "Frame length must be exactly 400 samples"
        );

        // 1. Remove DC offset
        let sum: f32 = window_samples.iter().sum();
        let mean = sum / FRAME_LENGTH_SAMPLES as f32;

        let mut buf = [0.0f32; FFT_SIZE];
        for i in 0..FRAME_LENGTH_SAMPLES {
            buf[i] = (window_samples[i] - mean) * 32768.0;
        }
        // 2. Pre-emphasis: y[n] = x[n] - 0.97 * x[n-1]
        for i in (1..FRAME_LENGTH_SAMPLES).rev() {
            buf[i] -= PREEMPHASIS * buf[i - 1];
        }
        buf[0] -= PREEMPHASIS * buf[0];

        // 3. Apply Hamming window
        for (i, slot) in buf[..FRAME_LENGTH_SAMPLES].iter_mut().enumerate() {
            *slot *= self.window[i];
        }

        // 4. In-place 512-point real-to-complex FFT
        let mut re = [0.0f32; FFT_SIZE];
        let mut im = [0.0f32; FFT_SIZE];
        for i in 0..FFT_SIZE {
            re[i] = buf[self.bit_rev[i] as usize];
        }

        let mut m = 2;
        while m <= FFT_SIZE {
            let half_m = m / 2;
            let angle = -2.0 * PI / m as f32;
            let w_step_re = angle.cos();
            let w_step_im = angle.sin();

            let mut w_re = 1.0f32;
            let mut w_im = 0.0f32;

            for j in 0..half_m {
                let mut k = 0;
                while k < FFT_SIZE {
                    let u_re = re[k + j];
                    let u_im = im[k + j];
                    let v_re = re[k + j + half_m];
                    let v_im = im[k + j + half_m];

                    let t_re = w_re * v_re - w_im * v_im;
                    let t_im = w_re * v_im + w_im * v_re;

                    re[k + j] = u_re + t_re;
                    im[k + j] = u_im + t_im;
                    re[k + j + half_m] = u_re - t_re;
                    im[k + j + half_m] = u_im - t_im;

                    k += m;
                }
                let next_w_re = w_re * w_step_re - w_im * w_step_im;
                let next_w_im = w_re * w_step_im + w_im * w_step_re;
                w_re = next_w_re;
                w_im = next_w_im;
            }
            m *= 2;
        }

        // 5. Power spectrum for bins 0..=256
        let mut power = [0.0f32; FFT_SIZE / 2 + 1];
        for k in 0..=FFT_SIZE / 2 {
            power[k] = re[k] * re[k] + im[k] * im[k];
        }

        // 6. Mel filterbank energies
        let mut out = [0.0f32; NUM_MEL_BINS];
        for (bin_idx, filter) in self.filters.iter().enumerate() {
            let mut energy = 0.0f32;
            for (w_idx, &weight) in filter.weights.iter().enumerate() {
                energy += weight * power[filter.offset + w_idx];
            }
            // Log energy with float epsilon floor matching Kaldi
            out[bin_idx] = energy.max(f32::EPSILON).ln();
        }

        out
    }

    /// Extracts 80-dim log-mel filterbanks for all frames in a 16kHz audio slice.
    pub fn extract_all(&self, samples: &[f32]) -> Vec<[f32; NUM_MEL_BINS]> {
        if samples.len() < FRAME_LENGTH_SAMPLES {
            return Vec::new();
        }

        let num_frames = (samples.len() - FRAME_LENGTH_SAMPLES) / FRAME_SHIFT_SAMPLES + 1;
        let mut frames = Vec::with_capacity(num_frames);

        for i in 0..num_frames {
            let start = i * FRAME_SHIFT_SAMPLES;
            let window = &samples[start..start + FRAME_LENGTH_SAMPLES];
            frames.push(self.extract_frame(window));
        }

        frames
    }
}

/// Applies Low Frame Rate (LFR) stacking and CMVN normalization for FSMN VAD.
/// Stacks `lfr_m` (5) consecutive 80-dim frames with stride `lfr_n` (1) into 400-dim frames,
/// then applies CMVN normalization.
pub fn apply_lfr_and_cmvn(
    frames: &[[f32; NUM_MEL_BINS]],
    cmvn: &CmvnParams,
    lfr_m: usize,
    lfr_n: usize,
) -> Vec<[f32; 400]> {
    assert!(lfr_n > 0, "lfr_n must be greater than 0");
    assert_eq!(
        lfr_m * NUM_MEL_BINS,
        400,
        "lfr_m must be 5 to produce 400-dim stacked frames"
    );

    if frames.is_empty() {
        return Vec::new();
    }

    let t = frames.len();
    let t_lfr = t.div_ceil(lfr_n);
    let left_pad = (lfr_m - 1) / 2;
    let mut out = Vec::with_capacity(t_lfr);
    for i in 0..t_lfr {
        let mut stacked = [0.0f32; 400];

        for m in 0..lfr_m {
            // Index in original frames with left/right padding
            let idx = (i * lfr_n + m).saturating_sub(left_pad);
            let clamped_idx = idx.min(t - 1);
            let frame = &frames[clamped_idx];

            let dst_start = m * NUM_MEL_BINS;
            for b in 0..NUM_MEL_BINS {
                // Apply CMVN per 80-dim frame component
                stacked[dst_start + b] = (frame[b] + cmvn.shifts[b]) * cmvn.rescales[b];
            }
        }

        out.push(stacked);
    }

    out
}

#[inline]
fn mel_scale(freq: f32) -> f32 {
    1127.0 * (1.0 + freq / 700.0).ln()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extractor_initializes_with_valid_filters() {
        let extractor = FbankExtractor::new();
        assert_eq!(extractor.filters.len(), NUM_MEL_BINS);
        for filter in &extractor.filters {
            assert!(!filter.weights.is_empty());
            assert!(filter.offset <= FFT_SIZE / 2);
        }
    }

    #[test]
    fn silence_frame_produces_finite_low_energies() {
        let extractor = FbankExtractor::new();
        let silence = [0.0f32; FRAME_LENGTH_SAMPLES];
        let frame = extractor.extract_frame(&silence);

        for &val in &frame {
            assert!(val.is_finite());
            // Since energy is floored to f32::EPSILON, ln(f32::EPSILON) is ~ -15.9
            assert!(val <= -15.0);
        }
    }

    #[test]
    fn sine_wave_energy_peaks_near_center_frequency() {
        let extractor = FbankExtractor::new();
        // 440 Hz sine wave @ 16kHz
        let mut samples = [0.0f32; FRAME_LENGTH_SAMPLES];
        for (i, s) in samples.iter_mut().enumerate() {
            *s = (2.0 * PI * 440.0 * (i as f32 / SAMPLE_RATE as f32)).sin();
        }

        let frame = extractor.extract_frame(&samples);
        // Find peak mel bin
        let (peak_bin, &peak_val) = frame
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap();

        // 440 Hz maps to mel bin ~15-20
        assert!((13..=16).contains(&peak_bin), "Peak bin was {peak_bin}");
        assert!(peak_val > 0.0, "Peak value should be positive");
    }

    #[test]
    fn extract_all_produces_expected_frame_count() {
        let extractor = FbankExtractor::new();
        // 16000 samples = 1.0 second = (16000 - 400) / 160 + 1 = 98 frames
        let samples = vec![0.0f32; 16_000];
        let frames = extractor.extract_all(&samples);
        assert_eq!(frames.len(), 98);
    }

    #[test]
    fn cmvn_normalization_applies_shift_and_rescale() {
        let cmvn = CmvnParams::default();
        let mut feat = [0.0f32; 80];
        cmvn.apply(&mut feat);

        for i in 0..80 {
            let expected = FSMN_CMVN_SHIFTS[i] * FSMN_CMVN_RESCALES[i];
            assert!((feat[i] - expected).abs() < 1e-6);
        }
    }

    #[test]
    fn lfr_splicing_matches_shape_and_padding() {
        let extractor = FbankExtractor::new();
        let samples = vec![0.1f32; 1600]; // 8 frames
        let frames = extractor.extract_all(&samples);
        let cmvn = CmvnParams::default();

        let lfr = apply_lfr_and_cmvn(&frames, &cmvn, 5, 1);
        assert_eq!(lfr.len(), frames.len());
        assert_eq!(lfr[0].len(), 400);
    }
}
