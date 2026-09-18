use crate::transcription::transcript::TranscriptSegment;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
#[cfg(feature = "specta")]
use specta::Type;
use std::path::Path;
use std::sync::Arc;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "kebab-case")]
pub enum AlignerPortErrorKind {
    InvalidRequest,
    ModelNotFound,
    AudioTooShort,
    AlignmentFailed,
    Runtime,
}

impl AlignerPortErrorKind {
    pub const fn as_code(self) -> &'static str {
        match self {
            Self::InvalidRequest => "ALIGNER_INVALID_REQUEST",
            Self::ModelNotFound => "ALIGNER_MODEL_NOT_FOUND",
            Self::AudioTooShort => "ALIGNER_AUDIO_TOO_SHORT",
            Self::AlignmentFailed => "ALIGNER_ALIGNMENT_FAILED",
            Self::Runtime => "ALIGNER_RUNTIME_ERROR",
        }
    }
}

#[derive(Clone, Debug, thiserror::Error)]
#[error("{message}")]
pub struct AlignerPortError {
    pub kind: AlignerPortErrorKind,
    pub message: String,
    stable_code: Option<String>,
}

impl PartialEq for AlignerPortError {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.message == other.message
    }
}

impl Eq for AlignerPortError {}

impl AlignerPortError {
    pub fn new(kind: AlignerPortErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            stable_code: None,
        }
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.stable_code = Some(code.into());
        self
    }

    pub fn code(&self) -> &str {
        self.stable_code
            .as_deref()
            .unwrap_or_else(|| self.kind.as_code())
    }

    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(AlignerPortErrorKind::InvalidRequest, message)
    }

    pub fn model_not_found(message: impl Into<String>) -> Self {
        Self::new(AlignerPortErrorKind::ModelNotFound, message)
    }

    pub fn audio_too_short(message: impl Into<String>) -> Self {
        Self::new(AlignerPortErrorKind::AudioTooShort, message)
    }

    pub fn alignment_failed(message: impl Into<String>) -> Self {
        Self::new(AlignerPortErrorKind::AlignmentFailed, message)
    }

    pub fn runtime(message: impl Into<String>) -> Self {
        Self::new(AlignerPortErrorKind::Runtime, message)
    }
}

/// Port for forced alignment engines that compute word/token-level timing
/// for existing transcript segments against raw audio.
#[async_trait]
pub trait SegmentAlignerPort: Send + Sync {
    /// Forces alignment of the given segments against `audio_samples`.
    ///
    /// The returned segments have their `timing` field populated with
    /// `TranscriptTimingLevel::Token` and `TranscriptTimingSource::Model`.
    async fn align_segments(
        &self,
        audio_samples: &[f32],
        sample_rate: u32,
        segments: &[TranscriptSegment],
    ) -> Result<Vec<TranscriptSegment>, AlignerPortError>;
}

/// Implementation family of an aligner engine provider.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AlignerEngineKind {
    #[default]
    CtcTrellisOnnx,
    TestDummy,
}

/// Factory that claims aligner model paths and loads [`SegmentAlignerPort`] instances.
pub trait AlignerEnginePort: Send + Sync {
    fn engine_kind(&self) -> AlignerEngineKind;

    /// Whether this engine can process the model at `model_path`.
    fn can_handle(&self, model_path: &Path) -> bool;

    /// Loads an instance from `model_path`.
    fn load(
        &self,
        model_path: &Path,
        num_threads: i32,
    ) -> Result<Arc<dyn SegmentAlignerPort>, AlignerPortError>;
}

/// Composition-time set of aligner engines, mirroring [`crate::ports::punctuation::PunctuationEngineSet`].
#[derive(Clone, Default)]
pub struct AlignerEngineSet {
    engines: Vec<Arc<dyn AlignerEnginePort>>,
}

impl std::fmt::Debug for AlignerEngineSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AlignerEngineSet")
            .field("engines", &self.engines.len())
            .finish()
    }
}

impl AlignerEngineSet {
    pub fn empty() -> Self {
        Self {
            engines: Vec::new(),
        }
    }

    /// Builder-style registration for composition roots.
    pub fn register(mut self, engine: Arc<dyn AlignerEnginePort>) -> Self {
        self.engines.push(engine);
        self
    }

    pub fn engines(&self) -> &[Arc<dyn AlignerEnginePort>] {
        &self.engines
    }

    /// Resolves the first engine that can handle a usable `model_path`.
    pub fn resolve(&self, model_path: Option<&Path>) -> Option<Arc<dyn AlignerEnginePort>> {
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

/// Loads the configured aligner model for one transcription job.
///
/// Absent or empty paths yield `None`. A configured path that does not exist
/// is a typed [`AlignerPortErrorKind::ModelNotFound`] error; an existing path
/// that no engine claims yields `None`.
pub fn load_configured_aligner(
    engines: &AlignerEngineSet,
    model_path: Option<&Path>,
) -> Result<Option<Arc<dyn SegmentAlignerPort>>, AlignerPortError> {
    let Some(path) = model_path else {
        return Ok(None);
    };
    if path.as_os_str().is_empty() {
        return Ok(None);
    }
    if !path.exists() {
        return Err(AlignerPortError::model_not_found(format!(
            "Aligner model path does not exist: {}",
            path.display()
        )));
    }
    match engines.resolve(Some(path)) {
        Some(engine) => Ok(Some(engine.load(path, 1)?)),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aligner_port_error_codes() {
        let err = AlignerPortError::invalid_request("invalid");
        assert_eq!(err.code(), "ALIGNER_INVALID_REQUEST");
        assert_eq!(err.kind, AlignerPortErrorKind::InvalidRequest);

        let err_override = err.with_code("CUSTOM_CODE");
        assert_eq!(err_override.code(), "CUSTOM_CODE");

        assert_eq!(
            AlignerPortError::model_not_found("not found").code(),
            "ALIGNER_MODEL_NOT_FOUND"
        );
        assert_eq!(
            AlignerPortError::audio_too_short("too short").code(),
            "ALIGNER_AUDIO_TOO_SHORT"
        );
        assert_eq!(
            AlignerPortError::alignment_failed("failed").code(),
            "ALIGNER_ALIGNMENT_FAILED"
        );
        assert_eq!(
            AlignerPortError::runtime("runtime").code(),
            "ALIGNER_RUNTIME_ERROR"
        );
    }

    #[test]
    fn test_aligner_port_error_equality() {
        let err1 = AlignerPortError::invalid_request("bad param");
        let err2 = AlignerPortError::invalid_request("bad param");
        let err3 = AlignerPortError::runtime("bad param");
        assert_eq!(err1, err2);
        assert_ne!(err1, err3);
    }

    struct FakeAligner;

    #[async_trait]
    impl SegmentAlignerPort for FakeAligner {
        async fn align_segments(
            &self,
            _audio_samples: &[f32],
            _sample_rate: u32,
            segments: &[TranscriptSegment],
        ) -> Result<Vec<TranscriptSegment>, AlignerPortError> {
            Ok(segments.to_vec())
        }
    }

    struct FakeAlignerEngine;

    impl AlignerEnginePort for FakeAlignerEngine {
        fn engine_kind(&self) -> AlignerEngineKind {
            AlignerEngineKind::TestDummy
        }

        fn can_handle(&self, model_path: &Path) -> bool {
            model_path.to_string_lossy().contains("claim_me")
        }

        fn load(
            &self,
            _model_path: &Path,
            _num_threads: i32,
        ) -> Result<Arc<dyn SegmentAlignerPort>, AlignerPortError> {
            Ok(Arc::new(FakeAligner))
        }
    }

    #[test]
    fn test_aligner_engine_set_resolution() {
        let set = AlignerEngineSet::empty().register(Arc::new(FakeAlignerEngine));
        assert_eq!(set.engines().len(), 1);

        assert!(set.resolve(None).is_none());
        assert!(set.resolve(Some(Path::new(""))).is_none());
        assert!(set.resolve(Some(Path::new("missing_model.onnx"))).is_none());
    }

    #[test]
    fn test_load_configured_aligner() {
        let set = AlignerEngineSet::empty().register(Arc::new(FakeAlignerEngine));

        // None or empty
        assert!(matches!(load_configured_aligner(&set, None), Ok(None)));
        assert!(matches!(
            load_configured_aligner(&set, Some(Path::new(""))),
            Ok(None)
        ));

        // Missing file
        let missing = Path::new("does_not_exist_aligner.onnx");
        let err = match load_configured_aligner(&set, Some(missing)) {
            Ok(_) => panic!("expected model not found error"),
            Err(err) => err,
        };
        assert_eq!(err.kind, AlignerPortErrorKind::ModelNotFound);

        // Existing directory that contains "claim_me"
        let temp_dir = std::env::temp_dir().join("claim_me_aligner_test");
        std::fs::create_dir_all(&temp_dir).unwrap();
        let loaded = match load_configured_aligner(&set, Some(&temp_dir)) {
            Ok(Some(aligner)) => aligner,
            Ok(None) => panic!("expected an aligner"),
            Err(err) => panic!("unexpected error: {err}"),
        };
        drop(loaded);
        std::fs::remove_dir_all(&temp_dir).unwrap();
    }
}
