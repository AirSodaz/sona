use serde_json::json;
use sona_core::recovery::types::{QUEUE_RECOVERY_FILE_NAME, RECOVERY_DIR_NAME, RecoverySnapshot};

fn write_recovery_snapshot(app_data_dir: &std::path::Path, source_path: &std::path::Path) {
    let recovery_dir = app_data_dir.join(RECOVERY_DIR_NAME);
    std::fs::create_dir_all(&recovery_dir).unwrap();
    std::fs::write(
        recovery_dir.join(QUEUE_RECOVERY_FILE_NAME),
        serde_json::to_vec_pretty(&json!({
            "version": 1,
            "updatedAt": 42,
            "items": [{
                "id": "recovery-1",
                "filename": "recording.wav",
                "filePath": source_path,
                "source": "batch_import",
                "resolution": "pending",
                "progress": 42.4,
                "segments": [],
                "projectId": null,
                "lastKnownStage": "transcribing",
                "updatedAt": 42,
                "hasSourceFile": true,
                "canResume": true,
                "exportConfig": null,
                "stageConfig": null
            }]
        }))
        .unwrap(),
    )
    .unwrap();
}

#[test]
fn recovery_list_outputs_normalized_snapshot_as_json() {
    let dir = tempfile::tempdir().unwrap();
    let app_data_dir = dir.path().join("app-data");
    let source_path = dir.path().join("recording.wav");
    std::fs::write(&source_path, "audio").unwrap();
    write_recovery_snapshot(&app_data_dir, &source_path);

    let app_data_dir = app_data_dir.to_string_lossy().into_owned();
    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "recovery",
        "list",
        "--app-data-dir",
        app_data_dir.as_str(),
        "--json",
    ])
    .unwrap();
    let snapshot: RecoverySnapshot = serde_json::from_str(&output.stdout).unwrap();

    assert_eq!(output.stderr, "");
    assert_eq!(snapshot.items[0].id, "recovery-1");
    assert!(snapshot.items[0].can_resume);
}

#[test]
fn recovery_json_preserves_control_and_multibyte_fields() {
    let dir = tempfile::tempdir().unwrap();
    let app_data_dir = dir.path().join("app-data");
    let source_path = dir.path().join("recording.wav");
    std::fs::write(&source_path, "audio").unwrap();
    let recovery_dir = app_data_dir.join(RECOVERY_DIR_NAME);
    std::fs::create_dir_all(&recovery_dir).unwrap();
    std::fs::write(
        recovery_dir.join(QUEUE_RECOVERY_FILE_NAME),
        serde_json::to_vec(&json!({
            "version": 1,
            "updatedAt": 42,
            "items": [{
                "id": "row\nid\t\u{1b}",
                "filename": "会议.wav",
                "filePath": source_path,
                "resolution": "pending",
                "segments": []
            }]
        }))
        .unwrap(),
    )
    .unwrap();

    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "recovery",
        "list",
        "--app-data-dir",
        app_data_dir.to_string_lossy().as_ref(),
        "--json",
    ])
    .unwrap();
    let snapshot: RecoverySnapshot = serde_json::from_str(&output.stdout).unwrap();

    assert_eq!(snapshot.items[0].id, "row\nid\t\u{1b}");
    assert_eq!(snapshot.items[0].filename, "会议.wav");
}

#[test]
fn recovery_list_outputs_table_by_default() {
    let dir = tempfile::tempdir().unwrap();
    let app_data_dir = dir.path().join("app-data");
    let source_path = dir.path().join("recording.wav");
    std::fs::write(&source_path, "audio").unwrap();
    write_recovery_snapshot(&app_data_dir, &source_path);

    let app_data_dir = app_data_dir.to_string_lossy().into_owned();
    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "recovery",
        "list",
        "--app-data-dir",
        app_data_dir.as_str(),
    ])
    .unwrap();

    assert_eq!(output.stderr, "");
    for header in ["ID", "FILE", "STAGE", "PROGRESS", "RESUMABLE"] {
        assert!(output.stdout.contains(header));
    }
    assert!(output.stdout.contains("recovery-1"));
    assert!(output.stdout.contains("recording.wav"));
    assert!(output.stdout.contains("42%"));
    assert!(output.stdout.contains("yes"));
}

#[test]
fn recovery_list_initializes_and_outputs_empty_json_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let app_data_dir = dir.path().join("app-data");
    let app_data_dir_arg = app_data_dir.to_string_lossy().into_owned();

    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "recovery",
        "list",
        "--app-data-dir",
        app_data_dir_arg.as_str(),
        "--json",
    ])
    .unwrap();
    let snapshot: RecoverySnapshot = serde_json::from_str(&output.stdout).unwrap();

    assert_eq!(snapshot.version, 1);
    assert_eq!(snapshot.updated_at, None);
    assert!(snapshot.items.is_empty());
    assert!(
        app_data_dir
            .join(RECOVERY_DIR_NAME)
            .join(QUEUE_RECOVERY_FILE_NAME)
            .is_file()
    );
}

#[test]
fn recovery_list_requires_app_data_dir() {
    let error =
        sona_cli::run_cli_from_args(["sona-cli", "recovery", "list", "--json"]).unwrap_err();

    assert!(matches!(error, sona_cli::CliError::Usage(_)));
    assert_eq!(error.exit_code(), 2);
}
