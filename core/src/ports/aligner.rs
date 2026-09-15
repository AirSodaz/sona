use crate::transcription::transcript::TranscriptSegment;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
#[cfg(feature = "specta")]
use specta::Type;

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
}
