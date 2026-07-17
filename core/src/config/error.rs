use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("JSON error: {0}")]
    Json(serde_json::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Configuration validation failed for field '{field}': {reason}")]
    Validation { field: String, reason: String },

    #[error("Unsupported log level: {0}")]
    InvalidLogLevel(String),

    #[error("Configuration repository error: {0}")]
    Repository(String),
}
