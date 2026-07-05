use serde_json::Value;

#[test]
fn path_status_command_renders_core_runtime_status_as_json() {
    let dir = tempfile::tempdir().unwrap();
    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "path-status",
        dir.path().to_string_lossy().as_ref(),
    ])
    .unwrap();
    let value: Value = serde_json::from_str(&output.stdout).unwrap();

    assert_eq!(output.stderr, "");
    assert_eq!(value["kind"], "directory");
    assert_eq!(value["path"], dir.path().to_string_lossy().as_ref());
    assert_eq!(value["error"], Value::Null);
}

#[test]
fn path_status_command_reports_missing_path_using_core_contract() {
    let dir = tempfile::tempdir().unwrap();
    let missing_path = dir.path().join("missing-runtime-path");
    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "path-status",
        missing_path.to_string_lossy().as_ref(),
    ])
    .unwrap();
    let value: Value = serde_json::from_str(&output.stdout).unwrap();

    assert_eq!(output.stderr, "");
    assert_eq!(value["kind"], "missing");
    assert_eq!(value["path"], missing_path.to_string_lossy().as_ref());
    assert_eq!(value["error"], Value::Null);
}
