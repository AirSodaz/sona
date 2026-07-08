pub use sona_core::ports::path::{PathKind, PathProvider};
pub use sona_runtime_fs::{
    default_desktop_app_data_roots, default_desktop_models_dir,
    select_desktop_models_dir_from_app_roots,
};

use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, Runtime};

#[cfg(test)]
use std::collections::HashMap;

pub fn models_dir_status(path: &Path) -> sona_core::model_paths::ModelsDirStatus {
    sona_runtime_fs::models_dir_status(path)
}

/// Tauri adapter for the pure `sona-core` path provider port.
///
/// `PathProvider` lives in `sona-core`, so `src-tauri` cannot implement it
/// directly for `tauri::AppHandle<R>` because both the trait and type are
/// external to this crate. This local newtype keeps the dependency direction
/// explicit: platform code adapts Tauri into the core port.
#[derive(Clone)]
pub struct TauriPathProvider<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> TauriPathProvider<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        Self { app }
    }

    pub fn from_app(app: &AppHandle<R>) -> Self {
        Self::new(app.clone())
    }
}

impl<R: Runtime> PathProvider for TauriPathProvider<R> {
    fn resolve_path(&self, kind: PathKind) -> Result<PathBuf, String> {
        match kind {
            PathKind::AppData => self.app.path().app_data_dir().map_err(|e| e.to_string()),
            PathKind::AppLocalData => self
                .app
                .path()
                .app_local_data_dir()
                .map_err(|e| e.to_string()),
            PathKind::AppLogData => self.app.path().app_log_dir().map_err(|e| e.to_string()),
        }
    }
}

#[cfg(test)]
pub struct MockPathProvider {
    entries: HashMap<PathKind, Result<PathBuf, String>>,
}

#[cfg(test)]
impl MockPathProvider {
    pub fn from_map(entries: HashMap<PathKind, Result<PathBuf, String>>) -> Self {
        Self { entries }
    }
}

#[cfg(test)]
impl PathProvider for MockPathProvider {
    fn resolve_path(&self, kind: PathKind) -> Result<PathBuf, String> {
        self.entries
            .get(&kind)
            .cloned()
            .unwrap_or_else(|| Err(format!("path kind {:?} not configured", kind)))
    }
}
