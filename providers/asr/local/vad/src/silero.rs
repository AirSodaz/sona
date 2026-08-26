use sherpa_onnx::{SileroVadModelConfig, VadModelConfig};
use sona_core::ports::asr::AsrPortError;
use sona_core::ports::vad::{SpeechSpan, VadDetectionOptions, VadEngineKind, VadEnginePort};
use std::path::Path;

use super::shared::{SUPPORTED_SAMPLE_RATE, detect_with_config, reject_unsupported_rate};

/// Silero ONNX VAD engine backed by sherpa-onnx.
///
/// Acts as the dispatch fallback: any model file that no other engine claims
/// is handled here, preserving the historical behavior for custom model
/// directories.
#[derive(Debug, Clone, Copy, Default)]
pub struct SileroOnnxEngine;

impl VadEnginePort for SileroOnnxEngine {
    fn engine_kind(&self) -> VadEngineKind {
        VadEngineKind::SileroOnnx
    }

    fn can_handle(&self, model_path: &Path) -> bool {
        !is_ten_vad_model_path(model_path)
    }

    fn detect(
        &self,
        samples: &[f32],
        sample_rate: u32,
        options: &VadDetectionOptions,
    ) -> Result<Vec<SpeechSpan>, AsrPortError> {
        reject_unsupported_rate("Silero VAD", sample_rate)?;

        let model_path = super::shared::resolve_model_onnx_path(&options.model_path)?;
        let config = silero_config(&model_path.to_string_lossy(), options);
        detect_with_config(samples, sample_rate, &config, options.buffer_seconds)
    }
}

fn silero_config(model: &str, options: &VadDetectionOptions) -> VadModelConfig {
    VadModelConfig {
        silero_vad: SileroVadModelConfig {
            model: Some(model.to_string()),
            threshold: options.threshold,
            min_silence_duration: options.min_silence_duration,
            min_speech_duration: options.min_speech_duration,
            window_size: 512,
            ..Default::default()
        },
        sample_rate: SUPPORTED_SAMPLE_RATE as i32,
        num_threads: super::shared::NUM_THREADS,
        ..Default::default()
    }
}

pub(super) fn is_ten_vad_model_path(model_path: &Path) -> bool {
    if model_path.is_dir() {
        // Directories are judged by the single contained .onnx file.
        return resolve_display_stem(model_path)
            .map(|stem| stem.starts_with("ten"))
            .unwrap_or(false);
    }

    model_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| stem.starts_with("ten"))
        .unwrap_or(false)
}

fn resolve_display_stem(model_path: &Path) -> Option<String> {
    if model_path.is_file() {
        return model_path.file_stem().and_then(|s| s.to_str()).map(String::from);
    }

    // Directories fall back to the single contained .onnx file when readable.
    std::fs::read_dir(model_path)
        .ok()?
        .flatten()
        .find(|entry| entry.path().extension().is_some_and(|ext| ext == "onnx"))
        .and_then(|entry| entry.path().file_stem().and_then(|s| s.to_str()).map(String::from))
}

#[cfg(test)]
mod tests {
    use super::{SileroOnnxEngine, is_ten_vad_model_path};
    use sona_core::ports::asr::AsrPortErrorKind;
    use sona_core::ports::vad::{VadDetectionOptions, VadEngineKind, VadEnginePort};
    use std::path::Path;

    fn unique_temp_name(prefix: &str) -> String {
        format!(
            "sona-vad-{}-{}",
            prefix,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )
    }

    #[test]
    fn engine_kind_is_silero_and_claims_non_ten_models() {
        let engine = SileroOnnxEngine;

        assert_eq!(engine.engine_kind(), VadEngineKind::SileroOnnx);
        assert!(engine.can_handle(Path::new("models/silero_vad.onnx")));
        assert!(!engine.can_handle(Path::new("models/ten-vad.onnx")));
    }

    #[test]
    fn ten_detection_sniffs_file_and_directory_names() {
        assert!(is_ten_vad_model_path(Path::new("models/ten-vad.onnx")));
        assert!(is_ten_vad_model_path(Path::new("models/ten_vad.int8.onnx")));
        assert!(!is_ten_vad_model_path(Path::new("models/silero_vad.onnx")));

        let root = std::env::temp_dir().join(unique_temp_name("ten-dir"));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("ten-vad.onnx"), b"stub").unwrap();
        assert!(is_ten_vad_model_path(&root));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn detect_rejects_unsupported_sample_rates() {
        let error = SileroOnnxEngine
            .detect(&[0.0], 44_100, &VadDetectionOptions::batch_defaults("unused"))
            .unwrap_err();

        assert_eq!(error.kind, AsrPortErrorKind::Unsupported);
        assert!(error.message.contains("Silero"));
    }

    #[test]
    fn detect_reports_missing_models_as_model_errors() {
        let missing = std::env::temp_dir().join(unique_temp_name("silero-missing"));
        let options = VadDetectionOptions::batch_defaults(&missing);

        let error = SileroOnnxEngine
            .detect(&[0.0; 16_000], 16_000, &options)
            .unwrap_err();

        assert_eq!(error.kind, AsrPortErrorKind::Model);
    }
}
