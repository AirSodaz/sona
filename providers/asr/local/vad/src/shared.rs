use sherpa_onnx::{VadModelConfig, VoiceActivityDetector};
use sona_core::ports::asr::{AsrPortError, AsrPortErrorKind};
use sona_core::ports::vad::SpeechSpan;
use std::path::{Path, PathBuf};

pub(crate) const SUPPORTED_SAMPLE_RATE: u32 = 16_000;
pub(crate) const DEFAULT_DETECTOR_CAPACITY_SECONDS: f32 = 60.0;
pub(crate) const NUM_THREADS: i32 = 1;

/// Runs one detection pass over `samples` with a fully-built model config.
///
/// Both engine families share this loop; only the config construction and the
/// window size differ.
pub(crate) fn detect_with_config(
    samples: &[f32],
    sample_rate: u32,
    config: &VadModelConfig,
    buffer_seconds: f32,
) -> Result<Vec<SpeechSpan>, AsrPortError> {
    if sample_rate != SUPPORTED_SAMPLE_RATE {
        return Err(AsrPortError::new(
            AsrPortErrorKind::Unsupported,
            format!(
                "This VAD engine supports {SUPPORTED_SAMPLE_RATE} Hz input, got {sample_rate} Hz."
            ),
        ));
    }

    let capacity = detector_capacity_seconds(buffer_seconds);
    let mut detector = VoiceActivityDetector::create(config, capacity)
        .ok_or_else(|| AsrPortError::runtime("Failed to create VoiceActivityDetector"))?;

    let window_size = detector_window_size(config);
    let mut spans = Vec::new();
    for chunk in samples.chunks(window_size) {
        detector.accept_waveform(chunk);
        extract_spans(&mut detector, sample_rate, &mut spans);
    }
    detector.flush();
    extract_spans(&mut detector, sample_rate, &mut spans);

    Ok(spans)
}

fn detector_window_size(config: &VadModelConfig) -> usize {
    // Exactly one family is configured; take whichever declares a size.
    let declared = if config.silero_vad.window_size > 0 {
        config.silero_vad.window_size
    } else {
        config.ten_vad.window_size
    };
    if declared > 0 { declared as usize } else { 512 }
}

fn detector_capacity_seconds(buffer_seconds: f32) -> f32 {
    if buffer_seconds > 0.0 {
        buffer_seconds
    } else {
        DEFAULT_DETECTOR_CAPACITY_SECONDS
    }
}

fn extract_spans(
    detector: &mut VoiceActivityDetector,
    sample_rate: u32,
    spans: &mut Vec<SpeechSpan>,
) {
    while !detector.is_empty() {
        if let Some(segment) = detector.front() {
            let start_sample = segment.start().max(0) as usize;
            let sample_count = segment.samples().len();
            log::debug!(
                "[Sona VAD] span start_sample={} duration={:.2}s",
                start_sample,
                sample_count as f32 / sample_rate.max(1) as f32
            );
            spans.push(SpeechSpan {
                start_sample,
                end_sample: start_sample.saturating_add(sample_count),
                sample_rate,
            });
        }
        detector.pop();
    }
}

pub(crate) fn reject_unsupported_rate(
    engine_label: &str,
    sample_rate: u32,
) -> Result<(), AsrPortError> {
    if sample_rate == SUPPORTED_SAMPLE_RATE {
        Ok(())
    } else {
        Err(AsrPortError::new(
            AsrPortErrorKind::Unsupported,
            format!(
                "{engine_label} supports {SUPPORTED_SAMPLE_RATE} Hz input, got {sample_rate} Hz."
            ),
        ))
    }
}

pub fn resolve_model_onnx_path(path: &Path) -> Result<PathBuf, AsrPortError> {
    if !path.exists() {
        return Err(AsrPortError::new(
            AsrPortErrorKind::Model,
            format!("Model path does not exist: {}", path.display()),
        ));
    }

    if path.is_file() {
        return Ok(path.to_path_buf());
    }

    let entries = std::fs::read_dir(path).map_err(|error| {
        AsrPortError::new(
            AsrPortErrorKind::FileSystem,
            format!("Failed to read model directory {}: {error}", path.display()),
        )
    })?;
    entries
        .flatten()
        .find(|entry| entry.path().extension().is_some_and(|ext| ext == "onnx"))
        .map(|entry| entry.path())
        .ok_or_else(|| {
            AsrPortError::new(
                AsrPortErrorKind::Model,
                format!(
                    "No .onnx file found in VAD model directory {}",
                    path.display()
                ),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_DETECTOR_CAPACITY_SECONDS, detect_with_config, detector_capacity_seconds,
        reject_unsupported_rate,
    };
    use sherpa_onnx::VadModelConfig;
    use sona_core::ports::asr::AsrPortErrorKind;

    #[test]
    fn non_positive_buffer_falls_back_to_default_capacity() {
        assert_eq!(detector_capacity_seconds(0.0), 60.0);
        assert_eq!(
            detector_capacity_seconds(-1.0),
            DEFAULT_DETECTOR_CAPACITY_SECONDS
        );
        assert_eq!(detector_capacity_seconds(5.0), 5.0);
    }

    #[test]
    fn rate_guard_reports_unsupported_rates() {
        assert!(reject_unsupported_rate("Silero", 16_000).is_ok());

        let error = reject_unsupported_rate("Silero", 44_100).unwrap_err();
        assert_eq!(error.kind, AsrPortErrorKind::Unsupported);
        assert!(error.message.contains("Silero"));
        assert!(error.message.contains("16000"));
    }

    #[test]
    fn empty_detection_input_yields_no_spans_without_a_model() {
        // A zeroed config never reaches onnxruntime because the sample-rate
        // guard fires first for non-16k inputs; at 16k the detector creation
        // fails gracefully instead of panicking.
        let error = detect_with_config(&[], 16_000, &VadModelConfig::default(), 5.0)
            .expect_err("detector creation must fail without a model");

        assert_eq!(error.kind, AsrPortErrorKind::Runtime);
    }
}
