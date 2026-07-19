use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum HistoryTranscriptError {
    #[error("{subject} must be an array.")]
    ExpectedArray { subject: String },
    #[error("History transcript segment at index {index} must be an object: {reason}")]
    SegmentMustBeObject { index: usize, reason: String },
    #[error("History transcript segment at index {index} must match transcript schema: {reason}")]
    InvalidSegmentSchema { index: usize, reason: String },
}
