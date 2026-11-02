use sherpa_onnx::{TenVadModelConfig, VadModelConfig};
use sona_core::ports::asr::AsrPortError;
use sona_core::ports::vad::{SpeechSpan, VadDetectionOptions, VadEngineKind, VadEnginePort};
use std::path::Path;

use super::shared::{SUPPORTED_SAMPLE_RATE, detect_with_config, reject_unsupported_rate};

/// TEN VAD window size constant (16ms at 16kHz).
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
        let config = ten_config(&model_path.to_string_lossy(), options);
        detect_with_config(samples, sample_rate, &config, options.buffer_seconds)
    }

    fn create_stream_detector(
        &self,
        options: &VadDetectionOptions,
    ) -> Result<Box<dyn sona_core::ports::vad::StreamingVadPort>, AsrPortError> {
        let model_path = super::shared::resolve_model_onnx_path(&options.model_path)?;
        let config = ten_config(&model_path.to_string_lossy(), options);
        super::shared::create_streaming_detector_with_config(&config, options.buffer_seconds)
    }
}

fn ten_config(model: &str, options: &VadDetectionOptions) -> VadModelConfig {
    VadModelConfig {
        ten_vad: TenVadModelConfig {
            model: Some(model.to_string()),
            threshold: options.threshold,
            min_silence_duration: options.min_silence_duration,
            min_speech_duration: options.min_speech_duration,
            window_size: WINDOW_SIZE,
            max_speech_duration: options.threshold_escalation_duration,
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
    fn ten_config_maps_options_and_sets_window_size() {
        let options = VadDetectionOptions::batch_defaults("unused");
        let config = super::ten_config("models/ten-vad.onnx", &options);

        assert_eq!(config.ten_vad.model.as_deref(), Some("models/ten-vad.onnx"));
        assert_eq!(config.ten_vad.threshold, 0.50);
        assert_eq!(config.ten_vad.min_silence_duration, 1.2);
        assert_eq!(config.ten_vad.min_speech_duration, 0.25);
        assert_eq!(config.ten_vad.max_speech_duration, 25.0);
        assert_eq!(config.ten_vad.window_size, WINDOW_SIZE);
        assert_eq!(WINDOW_SIZE, 256);
        assert_eq!(config.sample_rate, 16_000);
    }
}
