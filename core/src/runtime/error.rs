use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum RuntimeConfigError {
    #[error("Failed to parse config file {source_label}: {reason}")]
    Parse {
        source_label: String,
        reason: String,
    },
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("{message}")]
pub struct RuntimeValidationError {
    pub subject: String,
    pub message: String,
}

impl RuntimeValidationError {
    pub fn new(subject: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            subject: subject.into(),
            message: message.into(),
        }
    }
}
