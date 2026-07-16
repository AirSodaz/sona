use sha2::{Digest, Sha256};
use sona_core::storage_usage::StorageUsageSnapshot;
use sona_sqlite::Database;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn seed(app_data_dir: &Path) -> Database {
    let database = Database::open(app_data_dir).unwrap();
    database
        .with_write_connection(|connection| {
            connection.execute_batch(
                "CREATE TABLE storage_probe (id INTEGER PRIMARY KEY, value TEXT NOT NULL);
                 CREATE INDEX idx_storage_probe_value ON storage_probe(value);
                 INSERT INTO storage_probe (value) VALUES ('active wal');",
            )?;
            Ok(())
        })
        .unwrap();
    fs::create_dir_all(app_data_dir.join("history")).unwrap();
    fs::write(app_data_dir.join("history").join("录音.wav"), [1_u8; 9]).unwrap();
    database
}

fn run_usage(app_data_dir: &str, json: bool) -> sona_cli::CliOutput {
    let mut args = vec![
        "sona-cli",
        "storage",
        "usage",
        "--app-data-dir",
        app_data_dir,
    ];
    if json {
        args.push("--json");
    }
    sona_cli::run_cli_from_args(args).unwrap()
}

fn file_hashes(root: &Path) -> BTreeMap<PathBuf, String> {
    fn visit(root: &Path, current: &Path, files: &mut BTreeMap<PathBuf, String>) {
        let mut entries = fs::read_dir(current)
            .unwrap()
            .map(|entry| entry.unwrap())
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                visit(root, &path, files);
            } else {
                files.insert(
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    hex::encode(Sha256::digest(fs::read(&path).unwrap())),
                );
            }
        }
    }

    let mut files = BTreeMap::new();
    visit(root, root, &mut files);
    files
}

#[test]
fn storage_usage_requires_app_data_dir() {
    let error =
        sona_cli::run_cli_from_args(["sona-cli", "storage", "usage", "--json"]).unwrap_err();

    assert!(matches!(error, sona_cli::CliError::Usage(_)));
    assert_eq!(error.exit_code(), 2);
}

#[test]
fn storage_usage_does_not_create_missing_app_data() {
    let root = tempfile::tempdir().unwrap();
    let missing = root.path().join("missing");

    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "storage",
        "usage",
        "--app-data-dir",
        missing.to_string_lossy().as_ref(),
    ])
    .unwrap_err();

    assert!(matches!(error, sona_cli::CliError::Io(_)));
    assert!(!missing.exists());
}

#[test]
fn storage_usage_renders_exact_table_columns() {
    let dir = tempfile::tempdir().unwrap();
    let _writer = seed(dir.path());

    let output = run_usage(dir.path().to_string_lossy().as_ref(), false);
    let lines = output.stdout.lines().collect::<Vec<_>>();

    assert_eq!(output.stderr, "");
    assert_eq!(lines.len(), 3);
    assert_eq!(
        lines[0].split_whitespace().collect::<Vec<_>>(),
        [
            "GENERATED",
            "TOTAL",
            "AUDIO",
            "DATABASE",
            "MODELS",
            "TEMPORARY",
            "WEBVIEW",
            "OTHER"
        ]
    );
    let values = lines[2].split_whitespace().collect::<Vec<_>>();
    assert_eq!(values.len(), 8);
    assert!(values[1].parse::<u64>().unwrap() >= 9);
    assert_eq!(values[2], "9");
}

#[test]
fn storage_usage_outputs_complete_pretty_json() {
    let dir = tempfile::tempdir().unwrap();
    let _writer = seed(dir.path());

    let output = run_usage(dir.path().to_string_lossy().as_ref(), true);
    let snapshot: StorageUsageSnapshot = serde_json::from_str(&output.stdout).unwrap();

    assert!(output.stdout.starts_with("{\n"));
    assert!(output.stdout.contains("\n  \"generatedAt\""));
    assert!(output.stdout.contains("\n  \"categories\""));
    assert_eq!(snapshot.categories.audio.history_audio_bytes, 9);
    assert!(snapshot.categories.database.sqlite.dbstat_available);
    assert!(snapshot.total_bytes >= 9);
}

#[test]
fn storage_usage_accepts_relative_unicode_path_and_does_not_modify_active_wal() {
    let current = std::env::current_dir().unwrap();
    let parent = tempfile::tempdir_in(&current).unwrap();
    let app_data_dir = parent.path().join("存储-CLI-🌍");
    let _writer = seed(&app_data_dir);
    let relative = app_data_dir.strip_prefix(&current).unwrap();
    let before = file_hashes(&app_data_dir);

    let output = run_usage(relative.to_string_lossy().as_ref(), true);
    let snapshot: StorageUsageSnapshot = serde_json::from_str(&output.stdout).unwrap();

    assert_eq!(snapshot.categories.audio.history_audio_bytes, 9);
    assert!(snapshot.categories.database.sqlite.index_bytes > 0);
    assert_eq!(file_hashes(&app_data_dir), before);
}

#[test]
fn storage_usage_rejects_future_schema_without_modifying_source_files() {
    let dir = tempfile::tempdir().unwrap();
    let database = Database::open(dir.path()).unwrap();
    database
        .with_write_connection(|connection| {
            connection.execute("INSERT INTO schema_version (version) VALUES (99)", [])?;
            Ok(())
        })
        .unwrap();
    drop(database);
    let before = file_hashes(dir.path());

    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "storage",
        "usage",
        "--app-data-dir",
        dir.path().to_string_lossy().as_ref(),
    ])
    .unwrap_err();

    assert!(matches!(error, sona_cli::CliError::Io(_)));
    assert!(error.to_string().contains("99"));
    assert_eq!(file_hashes(dir.path()), before);
}
