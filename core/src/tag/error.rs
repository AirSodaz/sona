use thiserror::Error;

use crate::ports::time::ClockError;

#[derive(Debug, Error)]
pub enum TagError {
    #[error("Tag repository error: {0}")]
    Repository(String),
    #[error("Tag serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Tag clock error: {0}")]
    Clock(#[from] ClockError),
}
