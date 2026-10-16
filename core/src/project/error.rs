use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("Project repository error: {0}")]
    Repository(String),
    #[error("Project serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Invalid project input: {0}")]
    InvalidInput(String),
}
