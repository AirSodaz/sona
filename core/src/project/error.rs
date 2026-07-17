use thiserror::Error;

use crate::ports::time::ClockError;

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("Project repository error: {0}")]
    Repository(String),
    #[error("Project serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Project clock error: {0}")]
    Clock(#[from] ClockError),
}
