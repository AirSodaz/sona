use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AutomationError {
    #[error("Automation repository error: {0}")]
    Repository(String),
}
