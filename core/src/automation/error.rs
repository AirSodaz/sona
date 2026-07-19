use thiserror::Error;

use crate::ports::fs::FileSystemError;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AutomationError {
    #[error("Automation repository error: {0}")]
    Repository(String),

    #[error(transparent)]
    FileSystem(#[from] FileSystemError),
}
