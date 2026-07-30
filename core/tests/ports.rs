use std::path::PathBuf;

use sona_core::ports::path::{PathKind, PathPort, PathPortError};

struct TestPathProvider;

impl PathPort for TestPathProvider {
    fn resolve_path(&self, kind: PathKind) -> Result<PathBuf, PathPortError> {
        match kind {
            PathKind::AppLocalData => Ok(PathBuf::from("/sona-test/app-local-data")),
            _ => Err(PathPortError::new(
                kind,
                format!("path kind {kind:?} not configured"),
            )),
        }
    }
}

#[test]
fn path_provider_port_is_exposed_from_ports_namespace() {
    let provider = TestPathProvider;

    let result = provider.resolve_path(PathKind::AppLocalData);

    assert!(result.is_ok());
}
