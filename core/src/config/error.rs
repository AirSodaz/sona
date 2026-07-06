use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Configuration validation failed for field '{field}': {reason}")]
    Validation { field: String, reason: String },

    #[error("Unsupported log level: {0}")]
    InvalidLogLevel(String),
}
