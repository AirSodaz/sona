use sha2::{Digest, Sha256};
use sona_core::task_ledger::service::TaskLedgerService;
use sona_core::task_ledger::types::{
    TaskLedgerKind, TaskLedgerRecord, TaskLedgerSnapshot, TaskLedgerStatus,
};
use sona_runtime_fs::SystemClock;
use sona_sqlite::{Database, SqliteLedgerRepository};
use std::path::Path;
use std::sync::Arc;
use unicode_width::UnicodeWidthStr;

fn record(id: &str, title: &str, progress: f64, updated_at: u64) -> TaskLedgerRecord {
    TaskLedgerRecord {
        id: id.to_string(),
        kind: TaskLedgerKind::LlmPolish,
        status: TaskLedgerStatus::Pending,
        title: title.to_string(),
        progress,
        created_at: updated_at,
        updated_at,
        retryable: false,
        cancelable: true,
        recoverable: false,
        stage: None,
        history_id: None,
        tag_ids: Vec::new(),
        file_path: None,
        automation_rule_id: None,
        source_fingerprint: None,
        error_message: None,
        template_id: None,
        target_language: None,
    }
}

fn seed(app_data_dir: &Path, records: Vec<TaskLedgerRecord>) {
    let database = Arc::new(Database::open(app_data_dir).unwrap());
    let repository = SqliteLedgerRepository::new(database);
    let service = TaskLedgerService::new(&repository, &SystemClock);
    for record in records {
        let now_ms = record.updated_at;
        service.upsert_task_at(record, now_ms).unwrap();
    }
}

fn run_list(app_data_dir: &Path, json: bool) -> sona_cli::CliOutput {
    let app_data_dir = app_data_dir.to_string_lossy().into_owned();
    let mut args = vec![
        "sona-cli",
        "task-ledger",
        "list",
        "--app-data-dir",
        app_data_dir.as_str(),
    ];
    if json {
        args.push("--json");
    }
    sona_cli::run_cli_from_args(args).unwrap()
}

fn directory_files(app_data_dir: &Path) -> Vec<(String, String)> {
    let mut files = std::fs::read_dir(app_data_dir)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (
                entry.file_name().to_string_lossy().into_owned(),
                hex::encode(Sha256::digest(std::fs::read(entry.path()).unwrap())),
            )
        })
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.0.cmp(&right.0));
    files
}

#[test]
fn task_ledger_list_requires_app_data_dir() {
    let error =
        sona_cli::run_cli_from_args(["sona-cli", "task-ledger", "list", "--json"]).unwrap_err();

    assert!(matches!(error, sona_cli::CliError::Usage(_)));
    assert_eq!(error.exit_code(), 2);
}

#[test]
fn task_ledger_list_outputs_empty_canonical_json() {
    let dir = tempfile::tempdir().unwrap();
    seed(dir.path(), Vec::new());

    let output = run_list(dir.path(), true);
    let snapshot: TaskLedgerSnapshot = serde_json::from_str(&output.stdout).unwrap();

    assert_eq!(output.stderr, "");
    assert_eq!(snapshot.version, 2);
    assert_eq!(snapshot.updated_at, None);
    assert!(snapshot.tasks.is_empty());
    assert!(output.stdout.contains("\"updatedAt\": null"));
    assert!(!output.stdout.contains("updated_at"));
}

#[test]
fn task_ledger_list_accepts_relative_app_data_dir() {
    let current_dir = std::env::current_dir().unwrap();
    let dir = tempfile::tempdir_in(&current_dir).unwrap();
    seed(
        dir.path(),
        vec![record("relative-path", "Relative path", 10.0, 1_100)],
    );
    let relative_dir = dir.path().strip_prefix(&current_dir).unwrap();
    let relative_dir = relative_dir.to_string_lossy().into_owned();

    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "task-ledger",
        "list",
        "--app-data-dir",
        relative_dir.as_str(),
        "--json",
    ])
    .unwrap();
    let snapshot: TaskLedgerSnapshot = serde_json::from_str(&output.stdout).unwrap();

    assert_eq!(snapshot.tasks.len(), 1);
    assert_eq!(snapshot.tasks[0].id, "relative-path");
}

#[test]
fn task_ledger_list_does_not_create_missing_app_data() {
    let dir = tempfile::tempdir().unwrap();
    let app_data_dir = dir.path().join("missing-app-data");
    let app_data_dir_arg = app_data_dir.to_string_lossy().into_owned();

    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "task-ledger",
        "list",
        "--app-data-dir",
        app_data_dir_arg.as_str(),
        "--json",
    ])
    .unwrap_err();

    assert!(matches!(error, sona_cli::CliError::Io(_)));
    assert!(!app_data_dir.exists());
}

#[test]
fn task_ledger_list_leaves_database_directory_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    seed(
        dir.path(),
        vec![record("read-only", "Read only", 5.0, 1_000)],
    );
    let before = directory_files(dir.path());

    let output = run_list(dir.path(), true);

    assert!(output.stdout.contains("read-only"));
    assert_eq!(directory_files(dir.path()), before);
}

#[test]
fn task_ledger_list_reads_committed_wal_without_modifying_files() {
    let dir = tempfile::tempdir().unwrap();
    let database = Arc::new(Database::open(dir.path()).unwrap());
    let repository = SqliteLedgerRepository::new(Arc::clone(&database));
    TaskLedgerService::new(&repository, &SystemClock)
        .upsert_task_at(record("wal-task", "Committed in WAL", 15.0, 1_500), 1_500)
        .unwrap();
    let wal_path = dir.path().join("sona.db-wal");
    assert!(wal_path.is_file());
    assert!(std::fs::metadata(&wal_path).unwrap().len() > 0);
    let before = directory_files(dir.path());

    let output = run_list(dir.path(), true);
    let snapshot: TaskLedgerSnapshot = serde_json::from_str(&output.stdout).unwrap();

    assert_eq!(snapshot.tasks.len(), 1);
    assert_eq!(snapshot.tasks[0].id, "wal-task");
    assert_eq!(directory_files(dir.path()), before);
}

#[test]
fn task_ledger_list_rejects_wal_without_shm_without_modifying_files() {
    let source = tempfile::tempdir().unwrap();
    seed(source.path(), Vec::new());
    let database = Arc::new(Database::open(source.path()).unwrap());
    let repository = SqliteLedgerRepository::new(Arc::clone(&database));
    TaskLedgerService::new(&repository, &SystemClock)
        .upsert_task_at(
            record("orphan-wal-task", "Committed in orphan WAL", 20.0, 1_600),
            1_600,
        )
        .unwrap();

    let snapshot_dir = tempfile::tempdir().unwrap();
    std::fs::copy(
        source.path().join("sona.db"),
        snapshot_dir.path().join("sona.db"),
    )
    .unwrap();
    std::fs::copy(
        source.path().join("sona.db-wal"),
        snapshot_dir.path().join("sona.db-wal"),
    )
    .unwrap();
    assert!(!snapshot_dir.path().join("sona.db-shm").exists());
    let before = directory_files(snapshot_dir.path());
    let app_data_dir = snapshot_dir.path().to_string_lossy().into_owned();

    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "task-ledger",
        "list",
        "--app-data-dir",
        app_data_dir.as_str(),
        "--json",
    ])
    .unwrap_err();

    assert!(matches!(error, sona_cli::CliError::Io(_)));
    assert!(error.to_string().contains("incomplete WAL sidecars"));
    assert_eq!(directory_files(snapshot_dir.path()), before);
}

#[test]
fn task_ledger_list_rejects_future_database_schema() {
    let dir = tempfile::tempdir().unwrap();
    seed(dir.path(), Vec::new());
    let database = Database::open(dir.path()).unwrap();
    database
        .with_connection(|connection| {
            connection.execute("INSERT INTO schema_version (version) VALUES (99)", [])?;
            Ok(())
        })
        .unwrap();
    drop(database);
    let app_data_dir = dir.path().to_string_lossy().into_owned();

    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "task-ledger",
        "list",
        "--app-data-dir",
        app_data_dir.as_str(),
        "--json",
    ])
    .unwrap_err();

    assert!(matches!(error, sona_cli::CliError::Io(_)));
    assert!(
        error
            .to_string()
            .contains("Unsupported database schema version 99")
    );
}

#[test]
fn task_ledger_list_outputs_populated_canonical_json() {
    let dir = tempfile::tempdir().unwrap();
    seed(
        dir.path(),
        vec![record("task-1", "Polish transcript", 42.5, 2_000)],
    );

    let output = run_list(dir.path(), true);
    let snapshot: TaskLedgerSnapshot = serde_json::from_str(&output.stdout).unwrap();

    assert_eq!(snapshot.tasks[0].id, "task-1");
    assert_eq!(snapshot.tasks[0].kind, TaskLedgerKind::LlmPolish);
    assert_eq!(snapshot.tasks[0].status, TaskLedgerStatus::Pending);
    assert_eq!(snapshot.tasks[0].updated_at, 2_000);
    assert!(output.stdout.contains("\"kind\": \"llmPolish\""));
    assert!(output.stdout.contains("\"updatedAt\": 2000"));
}

#[test]
fn task_ledger_list_outputs_expected_table_by_default() {
    let dir = tempfile::tempdir().unwrap();
    seed(
        dir.path(),
        vec![record("task-1", "Polish transcript", 42.5, 2_000)],
    );

    let output = run_list(dir.path(), false);

    assert_eq!(output.stderr, "");
    for header in ["ID", "KIND", "STATUS", "TITLE", "PROGRESS", "UPDATED"] {
        assert!(output.stdout.contains(header));
    }
    for value in [
        "task-1",
        "llmPolish",
        "pending",
        "Polish transcript",
        "42.5%",
        "2000",
    ] {
        assert!(output.stdout.contains(value));
    }
}

#[test]
fn task_ledger_table_escapes_control_characters() {
    let dir = tempfile::tempdir().unwrap();
    seed(
        dir.path(),
        vec![record("row\nid\t\u{1b}", "line\rtitle\u{7}", 10.0, 3_000)],
    );

    let output = run_list(dir.path(), false);

    assert_eq!(output.stdout.lines().count(), 3);
    assert!(output.stdout.contains(r"row\nid\t\u{1b}"));
    assert!(output.stdout.contains(r"line\rtitle\u{7}"));
    assert!(
        !output
            .stdout
            .chars()
            .any(|character| character != '\n' && character.is_control())
    );
}

#[test]
fn task_ledger_table_aligns_unicode_titles_by_display_width() {
    let dir = tempfile::tempdir().unwrap();
    seed(
        dir.path(),
        vec![
            record("task-1", "会议记录", 10.0, 4_000),
            record("task-2", "meeting1", 20.0, 4_001),
        ],
    );

    let output = run_list(dir.path(), false);
    let progress_columns = output
        .stdout
        .lines()
        .skip(2)
        .map(|line| {
            let progress = if line.starts_with("task-1") {
                "10%"
            } else {
                "20%"
            };
            let progress_index = line.find(progress).unwrap();
            UnicodeWidthStr::width(&line[..progress_index])
        })
        .collect::<Vec<_>>();

    assert_eq!(progress_columns.len(), 2);
    assert_eq!(progress_columns[0], progress_columns[1]);
}
