pub use sona_core::ports::path::{PathKind, PathPort, PathPortError};
pub use sona_runtime_fs::{
    default_desktop_app_data_roots, default_desktop_models_dir,
    select_desktop_models_dir_from_app_roots,
};

use std::path::{Path, PathBuf};

#[cfg(test)]
use std::collections::HashMap;

pub fn models_dir_status(path: &Path) -> sona_core::models::paths::ModelsDirStatus {
    sona_runtime_fs::models_dir_status(path)
}

/// Pure Rust Desktop implementation of the `PathPort` trait.
#[derive(Clone, Debug)]
pub struct DesktopPathProvider {
    app_local_data_dir: PathBuf,
}

impl DesktopPathProvider {
    pub fn new(app_local_data_dir: PathBuf) -> Self {
        Self { app_local_data_dir }
    }

    pub fn from_default() -> Self {
        let roots = default_desktop_app_data_roots();
        let default_dir = roots
            .into_iter()
            .next()
            .unwrap_or_else(|| PathBuf::from("data"));
        Self::new(default_dir)
    }
}

impl PathPort for DesktopPathProvider {
    fn resolve_path(&self, kind: PathKind) -> Result<PathBuf, PathPortError> {
        match kind {
            PathKind::AppData | PathKind::AppLocalData => {
                Ok(crate::platform::storage_location::resolve_active_data_dir(
                    &self.app_local_data_dir,
                ))
            }
            PathKind::AppLogData => {
                let log_dir = self.app_local_data_dir.join("logs");
                Ok(log_dir)
            }
        }
    }
}

#[cfg(test)]
pub struct MockPathProvider {
    entries: HashMap<PathKind, Result<PathBuf, PathPortError>>,
}

#[cfg(test)]
impl MockPathProvider {
    pub fn from_map(entries: HashMap<PathKind, Result<PathBuf, PathPortError>>) -> Self {
        Self { entries }
    }
}

#[cfg(test)]
impl PathPort for MockPathProvider {
    fn resolve_path(&self, kind: PathKind) -> Result<PathBuf, PathPortError> {
        self.entries.get(&kind).cloned().unwrap_or_else(|| {
            Err(PathPortError::new(
                kind,
                format!("path kind {kind:?} not configured"),
            ))
        })
    }
}
