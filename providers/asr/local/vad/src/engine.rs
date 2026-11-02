use crate::runners::VadModelRunner;
use crate::runners::fsmn::FsmnVadRunner;
use crate::runners::silero::SileroVadRunner;
use crate::shared::{SUPPORTED_SAMPLE_RATE, reject_unsupported_rate, resolve_model_onnx_path};
use crate::state_machine::UniversalVadStateMachine;
use crate::streaming::NativeStreamingVad;
use sona_core::ports::asr::AsrPortError;
use sona_core::ports::vad::{
    SpeechSpan, StreamingVadPort, VadDetectionOptions, VadEngineKind, VadEnginePort,
};
use std::path::Path;

/// Silero VAD engine running on native ONNX Runtime.
///
/// Supports Silero V4, V5, and V6 models with automatic state node and dimension detection.
/// Acts as the default fallback for custom ONNX models.
#[derive(Debug, Clone, Copy, Default)]
pub struct SileroOnnxEngine;

impl VadEnginePort for SileroOnnxEngine {
    fn engine_kind(&self) -> VadEngineKind {
        VadEngineKind::SileroOnnx
    }

    fn can_handle(&self, model_path: &Path) -> bool {
        !is_fsmn_vad_model_path(model_path)
    }

    fn detect(
        &self,
        samples: &[f32],
        sample_rate: u32,
        options: &VadDetectionOptions,
    ) -> Result<Vec<SpeechSpan>, AsrPortError> {
        reject_unsupported_rate("SileroOnnxEngine", sample_rate)?;
        let onnx_path = resolve_model_onnx_path(&options.model_path)?;
        let mut runner = SileroVadRunner::from_file(&onnx_path)?;

        let probs = runner.predict_probs_batch(samples)?;
        let mut state_machine = UniversalVadStateMachine::new(options, sample_rate);
        Ok(state_machine.batch_detect(&probs, runner.window_size()))
    }

    fn create_stream_detector(
        &self,
        options: &VadDetectionOptions,
    ) -> Result<Box<dyn StreamingVadPort>, AsrPortError> {
        let onnx_path = resolve_model_onnx_path(&options.model_path)?;
        let runner = SileroVadRunner::from_file(&onnx_path)?;
        Ok(Box::new(NativeStreamingVad::new(
            Box::new(runner),
            options,
            SUPPORTED_SAMPLE_RATE,
        )))
    }
}

/// Alibaba FunASR FSMN-VAD engine running on native ONNX Runtime.
///
/// Uses an embedded 80-dim log-mel filterbank frontend and context cache for streaming
/// and single-call vectorized evaluation for batch processing.
#[derive(Debug, Clone, Copy, Default)]
pub struct FsmnVadOnnxEngine;

impl VadEnginePort for FsmnVadOnnxEngine {
    fn engine_kind(&self) -> VadEngineKind {
        VadEngineKind::FsmnVadOnnx
    }

    fn can_handle(&self, model_path: &Path) -> bool {
        is_fsmn_vad_model_path(model_path)
    }

    fn detect(
        &self,
        samples: &[f32],
        sample_rate: u32,
        options: &VadDetectionOptions,
    ) -> Result<Vec<SpeechSpan>, AsrPortError> {
        reject_unsupported_rate("FsmnVadOnnxEngine", sample_rate)?;
        let onnx_path = resolve_model_onnx_path(&options.model_path)?;
        let mut runner = FsmnVadRunner::from_file(&onnx_path)?;

        let probs = runner.predict_probs_batch(samples)?;
        let mut state_machine = UniversalVadStateMachine::new(options, sample_rate);
        Ok(state_machine.batch_detect(&probs, runner.window_size()))
    }

    fn create_stream_detector(
        &self,
        options: &VadDetectionOptions,
    ) -> Result<Box<dyn StreamingVadPort>, AsrPortError> {
        let onnx_path = resolve_model_onnx_path(&options.model_path)?;
        let runner = FsmnVadRunner::from_file(&onnx_path)?;
        Ok(Box::new(NativeStreamingVad::new(
            Box::new(runner),
            options,
            SUPPORTED_SAMPLE_RATE,
        )))
    }
}

pub(crate) fn is_fsmn_vad_model_path(model_path: &Path) -> bool {
    let check_str = |s: &str| s.to_ascii_lowercase().contains("fsmn");

    if model_path
        .file_name()
        .and_then(|f| f.to_str())
        .is_some_and(check_str)
    {
        return true;
    }

    if model_path.is_dir()
        && let Ok(entries) = std::fs::read_dir(model_path)
    {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str()
                && check_str(name)
                && name.ends_with(".onnx")
            {
                return true;
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_kind_and_dispatch_routing() {
        let silero = SileroOnnxEngine;
        let fsmn = FsmnVadOnnxEngine;

        assert_eq!(silero.engine_kind(), VadEngineKind::SileroOnnx);
        assert_eq!(fsmn.engine_kind(), VadEngineKind::FsmnVadOnnx);

        assert!(fsmn.can_handle(Path::new("models/fsmn_vad.onnx")));
        assert!(fsmn.can_handle(Path::new("models/speech_fsmn_vad_zh-cn-16k-common.onnx")));
        assert!(!fsmn.can_handle(Path::new("models/silero_vad.onnx")));

        assert!(silero.can_handle(Path::new("models/silero_vad.onnx")));
        assert!(silero.can_handle(Path::new("models/silero_vad_v5.onnx")));
        assert!(silero.can_handle(Path::new("models/silero_vad_v6.onnx")));
        assert!(!silero.can_handle(Path::new("models/fsmn_vad.onnx")));
        // Fallback for custom onnx models
        assert!(silero.can_handle(Path::new("models/custom_vad_model.onnx")));
    }

    #[test]
    fn detect_rejects_unsupported_sample_rate() {
        let silero = SileroOnnxEngine;
        let opts = VadDetectionOptions::batch_defaults("mock.onnx");
        let err = silero.detect(&[0.0; 1600], 44_100, &opts).unwrap_err();
        assert_eq!(
            err.kind,
            sona_core::ports::asr::AsrPortErrorKind::Unsupported
        );
    }
}
