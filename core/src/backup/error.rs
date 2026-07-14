use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum BackupError {
    #[error("Invalid backup request: {0}")]
    InvalidRequest(String),
    #[error("Invalid backup: {0}")]
    InvalidBackup(String),
    #[error("Backup replacement requires explicit confirmation.")]
    ConfirmationRequired,
    #[error("Backup archive error: {0}")]
    Archive(String),
    #[error("Backup state error: {0}")]
    State(String),
    #[error("Backup configuration error: {0}")]
    Config(String),
}
