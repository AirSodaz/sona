use std::fs;
use std::path::Path;
use std::sync::Arc;

use serde_json::{Value, json};
use sona_core::history::{
    HistorySaveRecordingRequest, TranscriptSnapshotMetadata, TranscriptSnapshotReason,
};
use sona_core::history_store::HistoryStore;
use sona_sqlite::{Database, SqliteHistoryStore};

struct HistoryFixture {
    history_id: String,
    snapshot: TranscriptSnapshotMetadata,
}

fn create_history_fixture(app_data_dir: &Path) -> HistoryFixture {
    let db = Arc::new(Database::open(app_data_dir).unwrap());
    let store = SqliteHistoryStore::new(app_data_dir.to_path_buf(), db);
    store.ensure_ready().unwrap();
    let segments = json!([{
        "id": "segment-1",
        "text": "Hello history",
        "start": 0.0,
        "end": 1.25,
        "isFinal": true
    }]);
    let item = store
        .save_recording(HistorySaveRecordingRequest {
            segments: segments.clone(),
            duration: 1.25,
            project_id: None,
            audio_bytes: Some(vec![1, 2, 3]),
            native_audio_path: None,
            audio_extension: Some("wav".to_string()),
        })
        .unwrap();
    let snapshot = store
        .create_transcript_snapshot(&item.id, TranscriptSnapshotReason::Polish, segments)
        .unwrap();
    HistoryFixture {
        history_id: item.id,
        snapshot,
    }
}

fn run(args: &[&str]) -> sona_cli::CliOutput {
    sona_cli::run_cli_from_args(args.iter().copied()).unwrap()
}

#[test]
fn history_list_and_workspace_query_return_canonical_json() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = create_history_fixture(dir.path());
    let request_path = dir.path().join("query.json");
    fs::write(
        &request_path,
        serde_json::to_vec(&json!({
            "scope": {"kind": "all"},
            "query": "history",
            "filterType": "all",
            "dateFilter": "all",
            "sortOrder": "newest",
            "limit": 20,
            "offset": 0
        }))
        .unwrap(),
    )
    .unwrap();

    let list = run(&[
        "sona-cli",
        "history",
        "list",
        "--app-data-dir",
        dir.path().to_string_lossy().as_ref(),
        "--limit",
        "20",
        "--json",
    ]);
    let query = run(&[
        "sona-cli",
        "history",
        "query",
        "--app-data-dir",
        dir.path().to_string_lossy().as_ref(),
        "--input",
        request_path.to_string_lossy().as_ref(),
        "--json",
    ]);
    let list_json: Value = serde_json::from_str(&list.stdout).unwrap();
    let query_json: Value = serde_json::from_str(&query.stdout).unwrap();

    assert_eq!(list_json[0]["id"], fixture.history_id);
    assert_eq!(query_json["filteredItems"][0]["id"], fixture.history_id);
    assert_eq!(query_json["filteredItemCount"], 1);
}

#[test]
fn history_transcript_and_snapshot_commands_expose_persisted_content() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = create_history_fixture(dir.path());
    let app_data = dir.path().to_string_lossy();

    let transcript = run(&[
        "sona-cli",
        "history",
        "transcript",
        "--app-data-dir",
        app_data.as_ref(),
        "--history-id",
        &fixture.history_id,
        "--json",
    ]);
    let snapshots = run(&[
        "sona-cli",
        "history",
        "snapshots",
        "--app-data-dir",
        app_data.as_ref(),
        "--history-id",
        &fixture.history_id,
        "--json",
    ]);
    let snapshot = run(&[
        "sona-cli",
        "history",
        "snapshot",
        "--app-data-dir",
        app_data.as_ref(),
        "--history-id",
        &fixture.history_id,
        "--snapshot-id",
        &fixture.snapshot.id,
        "--json",
    ]);
    let transcript_json: Value = serde_json::from_str(&transcript.stdout).unwrap();
    let snapshots_json: Value = serde_json::from_str(&snapshots.stdout).unwrap();
    let snapshot_json: Value = serde_json::from_str(&snapshot.stdout).unwrap();

    assert_eq!(transcript_json[0]["text"], "Hello history");
    assert_eq!(snapshots_json[0]["id"], fixture.snapshot.id);
    assert_eq!(snapshot_json["metadata"]["id"], fixture.snapshot.id);
    assert_eq!(snapshot_json["segments"][0]["text"], "Hello history");
}

#[test]
fn history_table_outputs_have_stable_headers() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = create_history_fixture(dir.path());
    let output = run(&[
        "sona-cli",
        "history",
        "list",
        "--app-data-dir",
        dir.path().to_string_lossy().as_ref(),
    ]);

    assert_eq!(output.stdout.lines().count(), 3);
    let header = output.stdout.lines().next().unwrap();
    for column in ["ID", "TITLE", "KIND", "STATUS", "DURATION", "PROJECT"] {
        assert!(header.contains(column));
    }
    assert!(output.stdout.contains(&fixture.history_id));
}

#[test]
fn history_queries_reject_missing_directories_and_invalid_json_without_creation() {
    let root = tempfile::tempdir().unwrap();
    let missing = root.path().join("missing");
    let invalid = root.path().join("invalid.json");
    fs::write(&invalid, [0xff_u8, 0xfe]).unwrap();

    let missing_error = sona_cli::run_cli_from_args([
        "sona-cli",
        "history",
        "list",
        "--app-data-dir",
        missing.to_string_lossy().as_ref(),
    ])
    .unwrap_err();
    let invalid_error = sona_cli::run_cli_from_args([
        "sona-cli",
        "history",
        "query",
        "--app-data-dir",
        root.path().to_string_lossy().as_ref(),
        "--input",
        invalid.to_string_lossy().as_ref(),
    ])
    .unwrap_err();

    assert!(matches!(missing_error, sona_cli::CliError::Io(_)));
    assert!(matches!(invalid_error, sona_cli::CliError::Validation(_)));
    assert!(!missing.exists());
    assert!(!root.path().join("sona.db").exists());
}
