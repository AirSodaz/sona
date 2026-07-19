use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TranscriptPostprocessError {
    #[error("{reason}")]
    RuleCompilation { pattern: String, reason: String },
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum SpeakerCorrectionError {
    #[error("Speaker correction requires a source speaker id.")]
    MissingGroupId,
    #[error("Speaker profile not found: {profile_id}")]
    ProfileNotFound { profile_id: String },
}
