use std::path::PathBuf;
use std::sync::Arc;

/// Path kinds supported by PathProvider.
/// Add variants here as needed; match arms in impls will guide you.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PathKind {
    AppData,
    AppLocalData,
    AppLogData,
}

/// Trait for resolving application-specific data directories,
/// abstracting away Tauri's `AppHandle<R>`.
pub trait PathProvider: Send + Sync {
    fn resolve_path(&self, kind: PathKind) -> Result<PathBuf, String>;
}

impl<T: PathProvider + ?Sized> PathProvider for Arc<T> {
    fn resolve_path(&self, kind: PathKind) -> Result<PathBuf, String> {
        (**self).resolve_path(kind)
    }
}

// --- Test (mock) implementation ---

#[cfg(test)]
use std::collections::HashMap;

#[cfg(test)]
pub struct MockPathProvider {
    entries: HashMap<PathKind, Result<PathBuf, String>>,
}

#[cfg(test)]
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

#[cfg(test)]
impl Default for MockPathProvider {
    fn default() -> Self {
        Self::new()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_provider_resolves_configured_path() {
        let tmp = std::env::temp_dir().join("sona-mock-test");
        let mut map = std::collections::HashMap::new();
        map.insert(PathKind::AppLocalData, Ok(tmp.clone()));
        let provider = MockPathProvider::from_map(map);

        let result = provider.resolve_path(PathKind::AppLocalData);
        assert_eq!(result.unwrap(), tmp);
    }

    #[test]
    fn mock_provider_errors_on_unconfigured_kind() {
        let map = std::collections::HashMap::new();
        let provider = MockPathProvider::from_map(map);

        let result = provider.resolve_path(PathKind::AppData);
        assert!(result.is_err());
    }

    #[test]
    fn mock_provider_new_has_sensible_defaults() {
        let provider = MockPathProvider::new();
        assert!(provider.resolve_path(PathKind::AppData).is_ok());
        assert!(provider.resolve_path(PathKind::AppLocalData).is_ok());
    }
}
