use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ExportError {
    #[error("Unsupported export format: {value}")]
    InvalidFormat { value: String },
    #[error("Unable to infer export format from path: {}", path.display())]
    MissingFormatExtension { path: PathBuf },
    #[error("Unsupported export mode: {value}")]
    InvalidMode { value: String },
    #[error("{reason}")]
    Render { reason: String },
    #[error("{reason}")]
    Repository { reason: String },
}
