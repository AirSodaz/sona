use thiserror::Error;

use crate::ports::time::ClockError;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum SyncError {
    #[error("Invalid sync operation: {0}")]
    InvalidOperation(String),
    #[error("Invalid sync object key: {0}")]
    InvalidObjectKey(String),
    #[error("Sync object store error: {0}")]
    ObjectStore(String),
    #[error("Sync local repository error: {0}")]
    LocalRepository(String),
    #[error("Sync secret store error: {0}")]
    SecretStore(String),
    #[error("Sync protocol error: {0}")]
    Protocol(String),
    #[error("Sync cryptography error: {0}")]
    Crypto(String),
    #[error(transparent)]
    Clock(#[from] ClockError),
}
