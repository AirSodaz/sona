// Re-export PathKind from sona-core (the pure data type)
pub use sona_core::paths::PathKind;

// PathProvider trait stays local to src-tauri so we can implement
// it for tauri::AppHandle<R> without violating orphan rules.
pub trait PathProvider: Send + Sync {
    fn resolve_path(&self, kind: PathKind) -> Result<std::path::PathBuf, String>;
}

impl<T: PathProvider + ?Sized> PathProvider for std::sync::Arc<T> {
    fn resolve_path(&self, kind: PathKind) -> Result<std::path::PathBuf, String> {
        (**self).resolve_path(kind)
    }
}

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

// MockPathProvider is #[cfg(test)]-only.
// We keep a local copy here (the canonical version lives in sona-core for
// crates that don't depend on tauri).  This implements the *local* PathProvider
// trait so callers can use &dyn PathProvider in tests.
#[cfg(test)]
pub use mock::MockPathProvider;

#[cfg(test)]
mod mock {
    use super::{PathKind, PathProvider};
    use std::collections::HashMap;
    use std::path::PathBuf;

    pub struct MockPathProvider {
        entries: HashMap<PathKind, Result<PathBuf, String>>,
    }

    impl MockPathProvider {
        pub fn new() -> Self {
            let tmp = std::env::temp_dir();
            let mut entries = HashMap::new();
            entries.insert(PathKind::AppData, Ok(tmp.join("sona-test/app_data")));
            entries.insert(
                PathKind::AppLocalData,
                Ok(tmp.join("sona-test/app_local_data")),
            );
            entries.insert(PathKind::AppLogData, Ok(tmp.join("sona-test/app_log")));
            Self { entries }
        }

        pub fn from_map(entries: HashMap<PathKind, Result<PathBuf, String>>) -> Self {
            Self { entries }
        }
    }

    impl Default for MockPathProvider {
        fn default() -> Self {
            Self::new()
        }
    }

    impl PathProvider for MockPathProvider {
        fn resolve_path(&self, kind: PathKind) -> Result<PathBuf, String> {
            self.entries
                .get(&kind)
                .cloned()
                .unwrap_or_else(|| Err(format!("path kind {:?} not configured", kind)))
        }
    }
}
