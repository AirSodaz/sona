use crate::core::paths::{PathKind, PathProvider};
use tauri::{AppHandle, Manager, Runtime};

impl<R: Runtime> PathProvider for AppHandle<R> {
    fn resolve_path(&self, kind: PathKind) -> Result<std::path::PathBuf, String> {
        match kind {
            PathKind::AppData => self.path().app_data_dir().map_err(|e| e.to_string()),
            PathKind::AppLocalData => self.path().app_local_data_dir().map_err(|e| e.to_string()),
            PathKind::AppLogData => self.path().app_log_dir().map_err(|e| e.to_string()),
        }
    }
}
