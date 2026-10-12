use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AgentControlError {
    #[error("DEVICE_BUSY: {0}")]
    DeviceBusy(String),
    #[error("NO_ACTIVE_RECORDING: {0}")]
    NoActiveRecording(String),
    #[error("RECORDING_ALREADY_ACTIVE: {0}")]
    RecordingAlreadyActive(String),
    #[error("RECORDING_ALREADY_PAUSED: {0}")]
    RecordingAlreadyPaused(String),
    #[error("RECORDING_NOT_PAUSED: {0}")]
    RecordingNotPaused(String),
    #[error("TASK_CANCELLED: {0}")]
    TaskCancelled(String),
    #[error("NOT_FOUND: {0}")]
    NotFound(String),
    #[error("INVALID_PARAMS: {0}")]
    InvalidParams(String),
    #[error("INTERNAL_ERROR: {0}")]
    Internal(String),
}

impl AgentControlError {
    pub fn rpc_code(&self) -> i32 {
        match self {
            Self::TaskCancelled(_) => -32800,
            Self::InvalidParams(_) => -32602,
            _ => -32000,
        }
    }

    pub fn is_cancelled(&self) -> bool {
        matches!(self, Self::TaskCancelled(_))
    }
}

impl From<AgentControlError> for (i32, String) {
    fn from(err: AgentControlError) -> Self {
        (err.rpc_code(), err.to_string())
    }
}

impl From<String> for AgentControlError {
    fn from(s: String) -> Self {
        if let Some(rest) = s.strip_prefix("DEVICE_BUSY: ") {
            Self::DeviceBusy(rest.to_string())
        } else if s.starts_with("DEVICE_BUSY") {
            Self::DeviceBusy(s)
        } else if let Some(rest) = s.strip_prefix("NO_ACTIVE_RECORDING: ") {
            Self::NoActiveRecording(rest.to_string())
        } else if s.starts_with("NO_ACTIVE_RECORDING") {
            Self::NoActiveRecording(s)
        } else if let Some(rest) = s.strip_prefix("RECORDING_ALREADY_ACTIVE: ") {
            Self::RecordingAlreadyActive(rest.to_string())
        } else if s.starts_with("RECORDING_ALREADY_ACTIVE") {
            Self::RecordingAlreadyActive(s)
        } else if let Some(rest) = s.strip_prefix("RECORDING_ALREADY_PAUSED: ") {
            Self::RecordingAlreadyPaused(rest.to_string())
        } else if s.starts_with("RECORDING_ALREADY_PAUSED") {
            Self::RecordingAlreadyPaused(s)
        } else if let Some(rest) = s.strip_prefix("RECORDING_NOT_PAUSED: ") {
            Self::RecordingNotPaused(rest.to_string())
        } else if s.starts_with("RECORDING_NOT_PAUSED") {
            Self::RecordingNotPaused(s)
        } else if let Some(rest) = s.strip_prefix("TASK_CANCELLED: ") {
            Self::TaskCancelled(rest.to_string())
        } else if s.starts_with("TASK_CANCELLED")
            || s.contains("Task cancelled")
            || s.contains("task cancelled")
            || s.contains("cancelled")
            || s.contains("Cancelled")
        {
            Self::TaskCancelled(s)
        } else if let Some(rest) = s.strip_prefix("NOT_FOUND: ") {
            Self::NotFound(rest.to_string())
        } else if s.starts_with("NOT_FOUND") {
            Self::NotFound(s)
        } else if let Some(rest) = s.strip_prefix("INVALID_PARAMS: ") {
            Self::InvalidParams(rest.to_string())
        } else if s.starts_with("INVALID_PARAMS") {
            Self::InvalidParams(s)
        } else if let Some(rest) = s.strip_prefix("INTERNAL_ERROR: ") {
            Self::Internal(rest.to_string())
        } else {
            Self::Internal(s)
        }
    }
}

impl From<&str> for AgentControlError {
    fn from(s: &str) -> Self {
        Self::from(s.to_string())
    }
}
