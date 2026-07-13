use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ExportError {
    #[error("{reason}")]
    Render { reason: String },
    #[error("{reason}")]
    Repository { reason: String },
}
