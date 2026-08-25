use sherpa_onnx::{SileroVadModelConfig, VadModelConfig, VoiceActivityDetector};
use sona_core::ports::asr::{AsrPortError, AsrPortErrorKind};
use sona_core::ports::vad::{SpeechSpan, VadDetectionOptions, VadEngineKind, VadEnginePort};
use std::path::{Path, PathBuf};

const SUPPORTED_SAMPLE_RATE: u32 = 16_000;
const WINDOW_SIZE: i32 = 512;
const DEFAULT_DETECTOR_CAPACITY_SECONDS: f32 = 60.0;
const NUM_THREADS: i32 = 1;

/// Silero ONNX VAD engine backed by sherpa-onnx.
#[derive(Debug, Clone, Copy, Default)]
pub struct SherpaVadEngine;

impl VadEnginePort for SherpaVadEngine {
    fn engine_kind(&self) -> VadEngineKind {
        VadEngineKind::SileroOnnx
    }

    fn can_handle(&self, _model_path: &Path) -> bool {
        true
    }

    fn detect(
        &self,
        samples: &[f32],
        sample_rate: u32,
        options: &VadDetectionOptions,
    ) -> Result<Vec<SpeechSpan>, AsrPortError> {
        if sample_rate != SUPPORTED_SAMPLE_RATE {
            return Err(AsrPortError::new(
                AsrPortErrorKind::Unsupported,
                format!(
                    "Silero VAD supports {SUPPORTED_SAMPLE_RATE} Hz input, got {sample_rate} Hz."
                ),
            ));
        }

        let model_path = resolve_model_onnx_path(&options.model_path)?;
        let config = vad_model_config(&model_path.to_string_lossy(), options);
        let mut detector =
            create_detector(&config, detector_capacity_seconds(options.buffer_seconds))?;

        let window_size = if config.silero_vad.window_size > 0 {
            config.silero_vad.window_size as usize
        } else {
            WINDOW_SIZE as usize
        };
        let mut spans = Vec::new();
        for chunk in samples.chunks(window_size) {
            detector.accept_waveform(chunk);
            extract_spans(&mut detector, sample_rate, &mut spans);
        }
        detector.flush();
        extract_spans(&mut detector, sample_rate, &mut spans);

        Ok(spans)
    }
}

fn vad_model_config(model: &str, options: &VadDetectionOptions) -> VadModelConfig {
    VadModelConfig {
        silero_vad: SileroVadModelConfig {
            model: Some(model.to_string()),
            threshold: options.threshold,
            min_silence_duration: options.min_silence_duration,
            min_speech_duration: options.min_speech_duration,
            window_size: WINDOW_SIZE,
            ..Default::default()
        },
        sample_rate: SUPPORTED_SAMPLE_RATE as i32,
        num_threads: NUM_THREADS,
        ..Default::default()
    }
}

fn detector_capacity_seconds(buffer_seconds: f32) -> f32 {
    if buffer_seconds > 0.0 {
        buffer_seconds
    } else {
        DEFAULT_DETECTOR_CAPACITY_SECONDS
    }
}

fn create_detector(
    config: &VadModelConfig,
    capacity_seconds: f32,
) -> Result<VoiceActivityDetector, AsrPortError> {
    VoiceActivityDetector::create(config, capacity_seconds)
        .ok_or_else(|| AsrPortError::runtime("Failed to create VoiceActivityDetector"))
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

fn resolve_model_onnx_path(path: &Path) -> Result<PathBuf, AsrPortError> {
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
    use super::{SUPPORTED_SAMPLE_RATE, SherpaVadEngine, detector_capacity_seconds};
    use sona_core::ports::asr::AsrPortErrorKind;
    use sona_core::ports::vad::{VadDetectionOptions, VadEnginePort};
    use std::path::Path;

    #[test]
    fn engine_kind_is_silero_onnx_and_handles_any_path() {
        let engine = SherpaVadEngine;

        assert_eq!(
            engine.engine_kind(),
            sona_core::ports::vad::VadEngineKind::SileroOnnx
        );
        assert!(engine.can_handle(Path::new("anything")));
    }

    #[test]
    fn detect_rejects_unsupported_sample_rates() {
        let error = SherpaVadEngine
            .detect(
                &[0.0],
                44_100,
                &VadDetectionOptions::batch_defaults("unused"),
            )
            .unwrap_err();

        assert_eq!(error.kind, AsrPortErrorKind::Unsupported);
        assert!(error.message.contains("16000"));
        assert_eq!(SUPPORTED_SAMPLE_RATE, 16_000);
    }

    #[test]
    fn detect_reports_missing_models_as_model_errors() {
        let missing = std::env::temp_dir().join(format!("sona-vad-missing-{}", std::process::id()));
        let options = VadDetectionOptions::batch_defaults(&missing);

        let error = SherpaVadEngine
            .detect(&[0.0; 16_000], SUPPORTED_SAMPLE_RATE, &options)
            .unwrap_err();

        assert_eq!(error.kind, AsrPortErrorKind::Model);
    }

    #[test]
    fn non_positive_buffer_falls_back_to_default_capacity() {
        assert_eq!(detector_capacity_seconds(0.0), 60.0);
        assert_eq!(detector_capacity_seconds(-1.0), 60.0);
        assert_eq!(detector_capacity_seconds(5.0), 5.0);
    }
}
