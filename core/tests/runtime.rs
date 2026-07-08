use sona_core::runtime::{RuntimeEnvironmentStatus, RuntimePathKind, RuntimePathStatus};

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

#[cfg(feature = "specta")]
#[test]
fn runtime_types_are_specta_exportable() {
    fn assert_specta_type<T: specta::Type>() {}

    assert_specta_type::<RuntimeEnvironmentStatus>();
    assert_specta_type::<RuntimePathKind>();
    assert_specta_type::<RuntimePathStatus>();
}
