use sona_core::runtime::{
    RuntimeEnvironmentStatus, RuntimePathKind, RuntimePathStatus, resolve_runtime_path_status,
};

#[test]
fn runtime_path_status_detects_file_directory_and_missing_paths() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("sample.txt");
    std::fs::write(&file_path, b"sample").unwrap();
    let missing_path = dir.path().join("missing.txt");

    let file_status = resolve_runtime_path_status(file_path.to_string_lossy().as_ref());
    let directory_status = resolve_runtime_path_status(dir.path().to_string_lossy().as_ref());
    let missing_status = resolve_runtime_path_status(missing_path.to_string_lossy().as_ref());

    assert_eq!(file_status.kind, RuntimePathKind::File);
    assert_eq!(file_status.error, None);
    assert_eq!(directory_status.kind, RuntimePathKind::Directory);
    assert_eq!(directory_status.error, None);
    assert_eq!(missing_status.kind, RuntimePathKind::Missing);
    assert_eq!(missing_status.error, None);
}

#[test]
fn runtime_path_status_serializes_kind_as_frontend_contract_string() {
    let dir = tempfile::tempdir().unwrap();
    let value = serde_json::to_value(resolve_runtime_path_status(
        dir.path().to_string_lossy().as_ref(),
    ))
    .unwrap();

    assert_eq!(value["kind"], "directory");
    assert_eq!(value["path"], dir.path().to_string_lossy().as_ref());
}

#[cfg(feature = "specta")]
#[test]
fn runtime_types_are_specta_exportable() {
    fn assert_specta_type<T: specta::Type>() {}

    assert_specta_type::<RuntimeEnvironmentStatus>();
    assert_specta_type::<RuntimePathKind>();
    assert_specta_type::<RuntimePathStatus>();
}
