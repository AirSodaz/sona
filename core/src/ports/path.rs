use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;

/// Path kinds supported by `PathProvider`.
/// Add variants here as needed; match arms in impls will guide you.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PathKind {
    AppData,
    AppLocalData,
    AppLogData,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("Failed to resolve {kind:?} path: {reason}")]
pub struct PathProviderError {
    pub kind: PathKind,
    pub reason: String,
}

impl PathProviderError {
    pub fn new(kind: PathKind, reason: impl Into<String>) -> Self {
        Self {
            kind,
            reason: reason.into(),
        }
    }
}

/// Port for resolving application-specific data directories.
pub trait PathProvider: Send + Sync {
    fn resolve_path(&self, kind: PathKind) -> Result<PathBuf, PathProviderError>;
}

impl<T: PathProvider + ?Sized> PathProvider for Arc<T> {
    fn resolve_path(&self, kind: PathKind) -> Result<PathBuf, PathProviderError> {
        (**self).resolve_path(kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct TestPathProvider {
        entries: HashMap<PathKind, Result<PathBuf, PathProviderError>>,
    }

    impl TestPathProvider {
        fn new() -> Self {
            let mut entries = HashMap::new();
            entries.insert(PathKind::AppData, Ok(PathBuf::from("/sona-test/app_data")));
            entries.insert(
                PathKind::AppLocalData,
                Ok(PathBuf::from("/sona-test/app_local_data")),
            );
            entries.insert(
                PathKind::AppLogData,
                Ok(PathBuf::from("/sona-test/app_log")),
            );
            Self { entries }
        }

        fn from_map(entries: HashMap<PathKind, Result<PathBuf, PathProviderError>>) -> Self {
            Self { entries }
        }
    }

    impl PathProvider for TestPathProvider {
        fn resolve_path(&self, kind: PathKind) -> Result<PathBuf, PathProviderError> {
            self.entries.get(&kind).cloned().unwrap_or_else(|| {
                Err(PathProviderError::new(
                    kind,
                    format!("path kind {kind:?} not configured"),
                ))
            })
        }
    }

    #[test]
    fn provider_resolves_configured_path() {
        let tmp = PathBuf::from("/sona-mock-test");
        let mut map = std::collections::HashMap::new();
        map.insert(PathKind::AppLocalData, Ok(tmp.clone()));
        let provider = TestPathProvider::from_map(map);

        let result = provider.resolve_path(PathKind::AppLocalData);
        assert_eq!(result.unwrap(), tmp);
    }

    #[test]
    fn provider_errors_on_unconfigured_kind() {
        let map = std::collections::HashMap::new();
        let provider = TestPathProvider::from_map(map);

        let result = provider.resolve_path(PathKind::AppData);
        assert!(result.is_err());
    }

    #[test]
    fn provider_new_has_sensible_defaults() {
        let provider = TestPathProvider::new();
        assert!(provider.resolve_path(PathKind::AppData).is_ok());
        assert!(provider.resolve_path(PathKind::AppLocalData).is_ok());
    }
}
