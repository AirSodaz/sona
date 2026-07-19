use sona_core::ports::asr::{AsrPortError, AsrPortErrorKind};
use sona_core::ports::fs::FileSystemError;

#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum ApiServerPlatformError {
    #[error(transparent)]
    Transcription(#[from] AsrPortError),

    #[error("{reason}")]
    Information { reason: String },
}

impl ApiServerPlatformError {
    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self::Transcription(AsrPortError::new(AsrPortErrorKind::Unavailable, reason))
    }

    pub fn transcription(reason: impl Into<String>) -> Self {
        Self::Transcription(AsrPortError::runtime(reason))
    }

    pub fn information(reason: impl Into<String>) -> Self {
        Self::Information {
            reason: reason.into(),
        }
    }
}

#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum ApiServerConfigurationError {
    #[error("Invalid IP wildcard format: {rule}")]
    InvalidIpWildcard { rule: String },

    #[error("Invalid IP rule format: {rule}")]
    InvalidIpRule { rule: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiServerBindErrorKind {
    AddressInUse,
    AddressNotAvailable,
    PermissionDenied,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiServerBindError {
    pub address: String,
    pub kind: ApiServerBindErrorKind,
    pub os_code: Option<i32>,
    pub reason: String,
}

impl ApiServerBindError {
    pub fn from_io(error: std::io::Error, address: impl Into<String>) -> Self {
        let kind = match error.kind() {
            std::io::ErrorKind::AddrInUse => ApiServerBindErrorKind::AddressInUse,
            std::io::ErrorKind::AddrNotAvailable => ApiServerBindErrorKind::AddressNotAvailable,
            std::io::ErrorKind::PermissionDenied => ApiServerBindErrorKind::PermissionDenied,
            _ => ApiServerBindErrorKind::Other,
        };
        Self {
            address: address.into(),
            kind,
            os_code: error.raw_os_error(),
            reason: error.to_string(),
        }
    }
}

impl std::fmt::Display for ApiServerBindError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let os_error = self
            .os_code
            .map(|code| format!(" (os error {code})"))
            .unwrap_or_default();
        match self.kind {
            ApiServerBindErrorKind::AddressInUse => write!(
                formatter,
                "Address already in use: {}. Make sure the port is not being used by another process.{os_error}",
                self.address
            ),
            ApiServerBindErrorKind::AddressNotAvailable => {
                write!(
                    formatter,
                    "Address not available: {}.{os_error}",
                    self.address
                )
            }
            ApiServerBindErrorKind::PermissionDenied => write!(
                formatter,
                "Permission denied: Failed to bind to {}.{os_error}",
                self.address
            ),
            ApiServerBindErrorKind::Other => {
                if let Some(code) = self.os_code {
                    write!(
                        formatter,
                        "Failed to bind to {} (os error {code})",
                        self.address
                    )
                } else {
                    write!(
                        formatter,
                        "Failed to bind to {}: {}",
                        self.address, self.reason
                    )
                }
            }
        }
    }
}

impl std::error::Error for ApiServerBindError {}

#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum ApiServerRuntimeError {
    #[error(transparent)]
    FileSystem(#[from] FileSystemError),

    #[error(transparent)]
    Bind(#[from] ApiServerBindError),

    #[error("{reason}")]
    Serve { reason: String },

    #[error("API server task failed: {reason}")]
    TaskJoin { reason: String },

    #[error("API server failed to start: dashboard channel closed before binding completed")]
    DashboardChannelClosed,
}

#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum ApiServerStartError {
    #[error(transparent)]
    Configuration(#[from] ApiServerConfigurationError),

    #[error(transparent)]
    Runtime(#[from] ApiServerRuntimeError),
}

#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum ApiServerStopError {
    #[error("API server shutdown signal could not be delivered")]
    ShutdownSignalClosed,

    #[error(transparent)]
    Runtime(#[from] ApiServerRuntimeError),
}

#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum ApiServerDashboardError {
    #[error(transparent)]
    Platform(#[from] ApiServerPlatformError),
}

#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum ApiServerJobError {
    #[error("API server job queue is closed for job {job_id}")]
    QueueClosed { job_id: String },
}
