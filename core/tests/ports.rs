use sona_core::ports::path::{MockPathProvider, PathKind, PathProvider};

#[test]
fn path_provider_port_is_exposed_from_ports_namespace() {
    let provider = MockPathProvider::new();

    let result = provider.resolve_path(PathKind::AppLocalData);

    assert!(result.is_ok());
}
