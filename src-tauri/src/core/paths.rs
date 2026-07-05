pub use sona_core::ports::path::{PathKind, PathProvider};

use tauri::{AppHandle, Manager, Runtime};

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
    fn resolve_path(&self, kind: PathKind) -> Result<std::path::PathBuf, String> {
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
pub use sona_core::ports::path::MockPathProvider;
