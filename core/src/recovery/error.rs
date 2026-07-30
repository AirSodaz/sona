use thiserror::Error;

use crate::ports::path::PathPortError;
use crate::ports::time::ClockError;

#[derive(Debug, Error)]
pub enum RecoveryError {
    #[error("Recovery repository error: {0}")]
    Repository(String),
    #[error("Recovery path error: {0}")]
    Path(#[from] PathPortError),
    #[error("Recovery clock error: {0}")]
    Clock(#[from] ClockError),
}
