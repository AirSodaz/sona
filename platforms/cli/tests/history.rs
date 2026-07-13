use std::fs;
use std::path::Path;
use std::sync::Arc;

use serde_json::{Value, json};
use sona_core::history::mutation_repository::{
    HistoryCreateTranscriptSnapshotRequest, HistoryMutationRepository,
};
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
        .create_transcript_snapshot(HistoryCreateTranscriptSnapshotRequest {
            history_id: item.id.clone(),
            reason: TranscriptSnapshotReason::Polish,
            segments,
        })
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

fn write_json(path: &Path, value: Value) {
    fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
}

fn run_owned(args: Vec<String>) -> sona_cli::CliOutput {
    sona_cli::run_cli_from_args(args).unwrap()
}

#[test]
fn history_mutation_commands_share_policy_and_persist_files() {
    let dir = tempfile::tempdir().unwrap();
    let app_data = dir.path().to_string_lossy().into_owned();
    let segments_path = dir.path().join("segments.json");
    write_json(
        &segments_path,
        json!([{
            "id": "segment-1",
            "text": "CLI mutation",
            "start": 0.0,
            "end": 2.0,
            "isFinal": true
        }]),
    );

    let draft = run_owned(vec![
        "sona-cli".into(),
        "history".into(),
        "create-live-draft".into(),
        "--app-data-dir".into(),
        app_data.clone(),
        "--id".into(),
        "cli-draft".into(),
        "--audio-extension".into(),
        "wav".into(),
        "--json".into(),
    ]);
    let draft_json: Value = serde_json::from_str(&draft.stdout).unwrap();
    assert_eq!(draft_json["item"]["id"], "cli-draft");
    assert_eq!(draft_json["item"]["status"], "draft");

    let completed = run_owned(vec![
        "sona-cli".into(),
        "history".into(),
        "complete-live-draft".into(),
        "--app-data-dir".into(),
        app_data.clone(),
        "--history-id".into(),
        "cli-draft".into(),
        "--segments".into(),
        segments_path.to_string_lossy().into_owned(),
        "--duration".into(),
        "2".into(),
        "--json".into(),
    ]);
    assert_eq!(
        serde_json::from_str::<Value>(&completed.stdout).unwrap()["status"],
        "complete"
    );

    let audio_path = dir.path().join("recording.wav");
    fs::write(&audio_path, [1, 2, 3, 4]).unwrap();
    let recording_input = dir.path().join("recording.json");
    write_json(
        &recording_input,
        json!({
            "segments": serde_json::from_slice::<Value>(&fs::read(&segments_path).unwrap()).unwrap(),
            "duration": 2.0,
            "projectId": null,
            "audioExtension": "wav"
        }),
    );
    let recording = run_owned(vec![
        "sona-cli".into(),
        "history".into(),
        "save-recording".into(),
        "--app-data-dir".into(),
        app_data.clone(),
        "--input".into(),
        recording_input.to_string_lossy().into_owned(),
        "--audio".into(),
        audio_path.to_string_lossy().into_owned(),
        "--json".into(),
    ]);
    let recording_json: Value = serde_json::from_str(&recording.stdout).unwrap();
    let recording_id = recording_json["id"].as_str().unwrap().to_string();
    let persisted_audio = dir
        .path()
        .join("history")
        .join(recording_json["audioPath"].as_str().unwrap());
    assert_eq!(fs::read(&persisted_audio).unwrap(), vec![1, 2, 3, 4]);

    let import_source = dir.path().join("import.wav");
    fs::write(&import_source, [5, 6, 7]).unwrap();
    let import_input = dir.path().join("import.json");
    write_json(
        &import_input,
        json!({
            "id": "cli-import",
            "sourcePath": import_source,
            "segments": serde_json::from_slice::<Value>(&fs::read(&segments_path).unwrap()).unwrap(),
            "duration": 2.0,
            "projectId": null,
            "convertedSourcePath": null
        }),
    );
    run_owned(vec![
        "sona-cli".into(),
        "history".into(),
        "import-file".into(),
        "--app-data-dir".into(),
        app_data.clone(),
        "--input".into(),
        import_input.to_string_lossy().into_owned(),
        "--json".into(),
    ]);

    let updated_segments = dir.path().join("updated-segments.json");
    write_json(
        &updated_segments,
        json!([{"id": "segment-1", "text": "Updated CLI", "start": 0.0, "end": 2.0, "isFinal": true}]),
    );
    let updated = run_owned(vec![
        "sona-cli".into(),
        "history".into(),
        "update-transcript".into(),
        "--app-data-dir".into(),
        app_data.clone(),
        "--history-id".into(),
        "cli-import".into(),
        "--segments".into(),
        updated_segments.to_string_lossy().into_owned(),
        "--json".into(),
    ]);
    assert_eq!(
        serde_json::from_str::<Value>(&updated.stdout).unwrap()["previewText"],
        "Updated CLI..."
    );

    let snapshot = run_owned(vec![
        "sona-cli".into(),
        "history".into(),
        "create-snapshot".into(),
        "--app-data-dir".into(),
        app_data.clone(),
        "--history-id".into(),
        "cli-import".into(),
        "--reason".into(),
        "polish".into(),
        "--segments".into(),
        updated_segments.to_string_lossy().into_owned(),
        "--json".into(),
    ]);
    assert_eq!(
        serde_json::from_str::<Value>(&snapshot.stdout).unwrap()["reason"],
        "polish"
    );

    let meta_path = dir.path().join("meta.json");
    write_json(&meta_path, json!({"title": "CLI renamed", "icon": "mic"}));
    run_owned(vec![
        "sona-cli".into(),
        "history".into(),
        "update-meta".into(),
        "--app-data-dir".into(),
        app_data.clone(),
        "--history-id".into(),
        "cli-import".into(),
        "--updates".into(),
        meta_path.to_string_lossy().into_owned(),
    ]);

    let database = Database::open(dir.path()).unwrap();
    database
        .with_write_connection(|connection| {
            connection.execute(
                "INSERT INTO projects (id, name, icon, color, sort_order, created_at, updated_at) VALUES ('team:alpha', 'CLI', '', '', 0, 1, 1)",
                [],
            )?;
            Ok(())
        })
        .unwrap();
    drop(database);
    run_owned(vec![
        "sona-cli".into(),
        "history".into(),
        "assign-project".into(),
        "--app-data-dir".into(),
        app_data.clone(),
        "--history-id".into(),
        "cli-import".into(),
        "--project-id".into(),
        "team:alpha".into(),
    ]);
    run_owned(vec![
        "sona-cli".into(),
        "history".into(),
        "reassign-project".into(),
        "--app-data-dir".into(),
        app_data.clone(),
        "--current-project-id".into(),
        "team:alpha".into(),
    ]);
    run_owned(vec![
        "sona-cli".into(),
        "history".into(),
        "delete".into(),
        "--app-data-dir".into(),
        app_data.clone(),
        "--history-id".into(),
        recording_id,
    ]);
    assert!(!persisted_audio.exists());

    let list = run_owned(vec![
        "sona-cli".into(),
        "history".into(),
        "list".into(),
        "--app-data-dir".into(),
        app_data,
        "--json".into(),
    ]);
    let items: Value = serde_json::from_str(&list.stdout).unwrap();
    let imported = items
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == "cli-import")
        .unwrap();
    assert_eq!(imported["title"], "CLI renamed");
    assert_eq!(imported["projectId"], Value::Null);
}

#[test]
fn history_mutation_validation_precedes_lazy_database_open_and_maps_not_found() {
    let invalid_dir = tempfile::tempdir().unwrap();
    let invalid_segments = invalid_dir.path().join("segments.json");
    write_json(&invalid_segments, json!([]));

    let invalid = sona_cli::run_cli_from_args([
        "sona-cli",
        "history",
        "update-transcript",
        "--app-data-dir",
        invalid_dir.path().to_string_lossy().as_ref(),
        "--history-id",
        "",
        "--segments",
        invalid_segments.to_string_lossy().as_ref(),
    ])
    .unwrap_err();
    assert!(matches!(invalid, sona_cli::CliError::Validation(_)));
    assert!(!invalid_dir.path().join("sona.db").exists());

    let missing_dir = tempfile::tempdir().unwrap();
    let missing_segments = missing_dir.path().join("segments.json");
    write_json(&missing_segments, json!([]));
    let missing = sona_cli::run_cli_from_args([
        "sona-cli",
        "history",
        "update-transcript",
        "--app-data-dir",
        missing_dir.path().to_string_lossy().as_ref(),
        "--history-id",
        "missing-history",
        "--segments",
        missing_segments.to_string_lossy().as_ref(),
    ])
    .unwrap_err();
    assert!(matches!(missing, sona_cli::CliError::Io(_)));
    assert!(missing.to_string().contains("missing-history"));
}
