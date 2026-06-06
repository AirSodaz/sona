use std::path::PathBuf;
use tauri::{AppHandle, Manager, Runtime};

/// Resolves the app local data directory.
pub fn resolve_app_local_data_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    app.path()
        .app_local_data_dir()
        .map_err(|error| error.to_string())
}

/// Resolves the app data directory.
pub fn resolve_app_data_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|error| error.to_string())
}
