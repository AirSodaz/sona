use thiserror::Error;

use crate::ports::time::ClockError;

#[derive(Debug, Error)]
pub enum TaskLedgerError {
    #[error("Task ledger repository error: {0}")]
    Repository(String),
    #[error("Task ledger serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Task ledger clock error: {0}")]
    Clock(#[from] ClockError),
}
