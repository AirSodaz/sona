use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;

/// Path kinds supported by `PathPort`.
/// Add variants here as needed; match arms in impls will guide you.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PathKind {
    AppData,
    AppLocalData,
    AppLogData,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("Failed to resolve {kind:?} path: {reason}")]
pub struct PathPortError {
    pub kind: PathKind,
    pub reason: String,
}

impl PathPortError {
    pub fn new(kind: PathKind, reason: impl Into<String>) -> Self {
        Self {
            kind,
            reason: reason.into(),
        }
    }
}

/// Port for resolving application-specific data directories.
pub trait PathPort: Send + Sync {
    fn resolve_path(&self, kind: PathKind) -> Result<PathBuf, PathPortError>;
}

impl<T: PathPort + ?Sized> PathPort for Arc<T> {
    fn resolve_path(&self, kind: PathKind) -> Result<PathBuf, PathPortError> {
        (**self).resolve_path(kind)
    }
}
