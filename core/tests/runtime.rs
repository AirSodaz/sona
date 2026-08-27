use sona_core::runtime::environment::{RuntimePathKind, RuntimePathStatus};

#[test]
fn runtime_path_status_serializes_kind_as_frontend_contract_string() {
    let value = serde_json::to_value(RuntimePathStatus {
        path: "C:/logs".to_string(),
        kind: RuntimePathKind::Directory,
        error: None,
    })
    .unwrap();

    assert_eq!(value["kind"], "directory");
    assert_eq!(value["path"], "C:/logs");
}
