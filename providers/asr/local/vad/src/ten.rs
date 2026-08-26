use sherpa_onnx::{TenVadModelConfig, VadModelConfig};
use sona_core::ports::asr::AsrPortError;
use sona_core::ports::vad::{SpeechSpan, VadDetectionOptions, VadEngineKind, VadEnginePort};
use std::path::Path;

use super::shared::{SUPPORTED_SAMPLE_RATE, detect_with_config, reject_unsupported_rate};

/// TEN VAD tuning constants.
///
/// The generic [`VadDetectionOptions`] fields were calibrated for Silero and
/// are advisory per engine family; this engine keeps the upstream defaults so
/// sensitivity stays predictable regardless of Silero-derived inputs. Only
/// `buffer_seconds` flows through from callers.
const THRESHOLD: f32 = 0.5;
const MIN_SILENCE_DURATION: f32 = 0.5;
const MIN_SPEECH_DURATION: f32 = 0.25;
const WINDOW_SIZE: i32 = 256;

/// TEN VAD ONNX engine backed by sherpa-onnx (k2-fsa metadata build).
#[derive(Debug, Clone, Copy, Default)]
pub struct TenVadOnnxEngine;

impl VadEnginePort for TenVadOnnxEngine {
    fn engine_kind(&self) -> VadEngineKind {
        VadEngineKind::TenVadOnnx
    }

    fn can_handle(&self, model_path: &Path) -> bool {
        super::silero::is_ten_vad_model_path(model_path)
    }

    fn detect(
        &self,
        samples: &[f32],
        sample_rate: u32,
        options: &VadDetectionOptions,
    ) -> Result<Vec<SpeechSpan>, AsrPortError> {
        reject_unsupported_rate("TEN VAD", sample_rate)?;

        let model_path = super::shared::resolve_model_onnx_path(&options.model_path)?;
        let config = ten_config(&model_path.to_string_lossy());
        detect_with_config(samples, sample_rate, &config, options.buffer_seconds)
    }
}

fn ten_config(model: &str) -> VadModelConfig {
    VadModelConfig {
        ten_vad: TenVadModelConfig {
            model: Some(model.to_string()),
            threshold: THRESHOLD,
            min_silence_duration: MIN_SILENCE_DURATION,
            min_speech_duration: MIN_SPEECH_DURATION,
            window_size: WINDOW_SIZE,
            ..Default::default()
        },
        sample_rate: SUPPORTED_SAMPLE_RATE as i32,
        num_threads: super::shared::NUM_THREADS,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::{TenVadOnnxEngine, WINDOW_SIZE};
    use sona_core::ports::asr::AsrPortErrorKind;
    use sona_core::ports::vad::{VadDetectionOptions, VadEngineKind, VadEnginePort};
    use std::path::Path;

    #[test]
    fn engine_kind_is_ten_and_claims_ten_models() {
        let engine = TenVadOnnxEngine;

        assert_eq!(engine.engine_kind(), VadEngineKind::TenVadOnnx);
        assert!(engine.can_handle(Path::new("models/ten-vad.onnx")));
        assert!(!engine.can_handle(Path::new("models/silero_vad.onnx")));
    }

    #[test]
    fn detect_rejects_unsupported_sample_rates() {
        let error = TenVadOnnxEngine
            .detect(
                &[0.0],
                44_100,
                &VadDetectionOptions::batch_defaults("unused"),
            )
            .unwrap_err();

        assert_eq!(error.kind, AsrPortErrorKind::Unsupported);
        assert!(error.message.contains("TEN"));
    }

    #[test]
    fn detect_reports_missing_models_as_model_errors() {
        let missing = std::env::temp_dir().join(format!("sona-ten-missing-{}", std::process::id()));
        let options = VadDetectionOptions::batch_defaults(&missing);

        let error = TenVadOnnxEngine
            .detect(&[0.0; 16_000], 16_000, &options)
            .unwrap_err();

        assert_eq!(error.kind, AsrPortErrorKind::Model);
    }

    #[test]
    fn ten_config_pins_upstream_defaults() {
        let config = super::ten_config("models/ten-vad.onnx");

        assert_eq!(config.ten_vad.model.as_deref(), Some("models/ten-vad.onnx"));
        assert_eq!(config.ten_vad.threshold, 0.5);
        assert_eq!(config.ten_vad.min_silence_duration, 0.5);
        assert_eq!(config.ten_vad.min_speech_duration, 0.25);
        assert_eq!(WINDOW_SIZE, 256);
        assert_eq!(config.sample_rate, 16_000);
    }
}
