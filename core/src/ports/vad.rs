use crate::ports::asr::AsrPortError;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Implementation family of a VAD engine provider.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VadEngineKind {
    #[default]
    SileroOnnx,
    TenVadOnnx,
}

/// Tuning parameters for one voice activity detection pass.
#[derive(Clone, Debug)]
pub struct VadDetectionOptions {
    /// Model directory or file backing this detection pass.
    pub model_path: PathBuf,
    pub threshold: f32,
    pub min_silence_duration: f32,
    pub min_speech_duration: f32,
    /// Detector buffer capacity in seconds; non-positive values fall back to
    /// the engine default.
    pub buffer_seconds: f32,
}

impl VadDetectionOptions {
    /// Defaults used by batch speech segmentation; mirrors the historical
    /// sherpa-onnx batch tuning.
    pub fn batch_defaults(model_path: impl Into<PathBuf>) -> Self {
        Self {
            model_path: model_path.into(),
            threshold: 0.35,
            min_silence_duration: 1.0,
            min_speech_duration: 0.25,
            buffer_seconds: 5.0,
        }
    }
}

/// A detected speech region as half-open sample indices of the audio slice
/// passed to [`VadEnginePort::detect`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpeechSpan {
    pub start_sample: usize,
    /// Exclusive end sample index.
    pub end_sample: usize,
    pub sample_rate: u32,
}

impl SpeechSpan {
    fn rate(&self) -> f32 {
        self.sample_rate.max(1) as f32
    }

    pub fn start_time(&self) -> f32 {
        self.start_sample as f32 / self.rate()
    }

    pub fn duration(&self) -> f32 {
        self.end_sample.saturating_sub(self.start_sample) as f32 / self.rate()
    }

    pub fn end_time(&self) -> f32 {
        self.end_sample as f32 / self.rate()
    }

    /// Borrows the span's samples out of `samples`, clamped to its length.
    pub fn slice_samples<'a>(&self, samples: &'a [f32]) -> &'a [f32] {
        let start = self.start_sample.min(samples.len());
        let end = self.end_sample.clamp(start, samples.len());
        &samples[start..end]
    }
}

/// Engine-neutral voice activity detection capability.
///
/// Implementations live in provider crates and are composed into a
/// [`VadEngineSet`]. ASR adapters resolve an engine from the set and hand it
/// to Core segmentation helpers, so speech segmentation never depends on a
/// specific ASR engine.
pub trait VadEnginePort: Send + Sync {
    fn engine_kind(&self) -> VadEngineKind;

    /// Whether this engine can process the model at `model_path`.
    fn can_handle(&self, model_path: &Path) -> bool;

    /// Detects speech spans in mono PCM `samples` at `sample_rate`.
    ///
    /// Implementations may return an empty vec for silent input; an `Err`
    /// asks callers to apply their own fallback segmentation.
    fn detect(
        &self,
        samples: &[f32],
        sample_rate: u32,
        options: &VadDetectionOptions,
    ) -> Result<Vec<SpeechSpan>, AsrPortError>;
}

/// Composition-time set of VAD engines available on this host, mirroring the
/// `LocalAsrRegistry` builder style.
#[derive(Clone, Default)]
pub struct VadEngineSet {
    engines: Vec<Arc<dyn VadEnginePort>>,
}

impl VadEngineSet {
    pub fn empty() -> Self {
        Self {
            engines: Vec::new(),
        }
    }

    /// Builder-style registration for composition roots.
    pub fn register(mut self, engine: Arc<dyn VadEnginePort>) -> Self {
        self.engines.push(engine);
        self
    }

    pub fn engines(&self) -> &[Arc<dyn VadEnginePort>] {
        &self.engines
    }

    /// Resolves the first engine that can handle a usable `model_path`.
    ///
    /// Returns `None` when the path is absent or nonexistent so callers fall
    /// back to fixed-size chunk segmentation.
    pub fn resolve(&self, model_path: Option<&Path>) -> Option<Arc<dyn VadEnginePort>> {
        let path = model_path?;
        if path.as_os_str().is_empty() || !path.exists() {
            return None;
        }
        self.engines
            .iter()
            .find(|engine| engine.can_handle(path))
            .cloned()
    }
}

impl fmt::Debug for VadEngineSet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Engine handles are `dyn` trait objects, so derive(Debug) cannot
        // apply; report the composition shape instead.
        formatter
            .debug_struct("VadEngineSet")
            .field("engine_count", &self.engines.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::{SpeechSpan, VadDetectionOptions, VadEngineKind, VadEnginePort, VadEngineSet};
    use crate::ports::asr::{AsrPortError, AsrPortErrorKind};
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    struct FakeVad;

    impl VadEnginePort for FakeVad {
        fn engine_kind(&self) -> VadEngineKind {
            VadEngineKind::SileroOnnx
        }

        fn can_handle(&self, _model_path: &Path) -> bool {
            true
        }

        fn detect(
            &self,
            _samples: &[f32],
            _sample_rate: u32,
            _options: &VadDetectionOptions,
        ) -> Result<Vec<SpeechSpan>, AsrPortError> {
            Ok(vec![SpeechSpan {
                start_sample: 0,
                end_sample: 1,
                sample_rate: 16_000,
            }])
        }
    }

    #[test]
    fn batch_defaults_match_historical_batch_tuning() {
        let options = VadDetectionOptions::batch_defaults("models/silero-vad");

        assert_eq!(options.model_path, PathBuf::from("models/silero-vad"));
        assert_eq!(options.threshold, 0.35);
        assert_eq!(options.min_silence_duration, 1.0);
        assert_eq!(options.min_speech_duration, 0.25);
        assert_eq!(options.buffer_seconds, 5.0);
    }

    #[test]
    fn speech_span_reports_seconds_from_sample_indices() {
        let span = SpeechSpan {
            start_sample: 16_000,
            end_sample: 48_000,
            sample_rate: 16_000,
        };

        assert_eq!(span.start_time(), 1.0);
        assert_eq!(span.duration(), 2.0);
        assert_eq!(span.end_time(), 3.0);
        assert_eq!(span.slice_samples(&vec![0.0; 40_000]).len(), 24_000);
    }

    #[test]
    fn speech_span_slice_clamps_out_of_bounds_indices() {
        let span = SpeechSpan {
            start_sample: 10,
            end_sample: 100,
            sample_rate: 16_000,
        };
        let samples = [0.0f32; 20];

        assert_eq!(span.slice_samples(&samples).len(), 10);
    }

    #[test]
    fn empty_set_resolves_nothing() {
        assert!(
            VadEngineSet::empty()
                .resolve(Some(Path::new("missing")))
                .is_none()
        );
    }

    #[test]
    fn resolve_rejects_missing_and_empty_paths() {
        let set = VadEngineSet::empty().register(Arc::new(FakeVad));

        assert!(set.resolve(None).is_none());
        assert!(set.resolve(Some(Path::new(""))).is_none());
        assert!(
            set.resolve(Some(Path::new("definitely/not/a/model.onnx")))
                .is_none()
        );
    }

    #[test]
    fn resolve_returns_the_first_engine_that_handles_the_model() {
        let set = VadEngineSet::empty().register(Arc::new(FakeVad));
        let existing = std::env::temp_dir();

        let resolved = set.resolve(Some(&existing)).expect("engine resolves");

        assert_eq!(resolved.engine_kind(), VadEngineKind::SileroOnnx);
    }

    #[test]
    fn detect_errors_carry_the_port_error_contract() {
        let error = AsrPortError::new(AsrPortErrorKind::Unsupported, "rate");

        assert_eq!(error.kind, AsrPortErrorKind::Unsupported);
        assert_eq!(error.code(), "UNSUPPORTED");
    }
}
