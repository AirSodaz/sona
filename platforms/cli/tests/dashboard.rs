use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sona_core::history::HistorySaveRecordingRequest;
use sona_core::history::mutation_repository::HistoryMutationRepository;
use sona_core::history_store::HistoryStore;
use sona_core::llm::usage::{LlmUsageCategory, TokenUsage, UsageRecord};
use sona_core::project::{
    DEFAULT_POLISH_PRESET_ID, DEFAULT_SUMMARY_TEMPLATE_ID, DEFAULT_TRANSLATION_LANGUAGE,
    ProjectDefaults, ProjectRecord, ProjectStore,
};
use sona_sqlite::llm_usage::record_usage;
use sona_sqlite::{Database, SqliteHistoryStore, SqliteProjectRepository};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn project() -> ProjectRecord {
    ProjectRecord {
        id: "project-dashboard".to_string(),
        name: "Dashboard".to_string(),
        description: String::new(),
        icon: String::new(),
        created_at: 1,
        updated_at: 1,
        defaults: ProjectDefaults {
            summary_template_id: DEFAULT_SUMMARY_TEMPLATE_ID.to_string(),
            translation_language: DEFAULT_TRANSLATION_LANGUAGE.to_string(),
            polish_preset_id: DEFAULT_POLISH_PRESET_ID.to_string(),
            polish_scenario: None,
            polish_context: None,
            export_file_name_prefix: String::new(),
            enabled_text_replacement_set_ids: Vec::new(),
            enabled_hotword_set_ids: Vec::new(),
            enabled_polish_keyword_set_ids: Vec::new(),
            enabled_speaker_profile_ids: Vec::new(),
        },
    }
}

fn seed(app_data_dir: &Path, transcript_text: &str) -> Arc<Database> {
    let database = Arc::new(Database::open(app_data_dir).unwrap());
    let history = SqliteHistoryStore::new(app_data_dir.to_path_buf(), Arc::clone(&database));
    history.ensure_ready().unwrap();
    ProjectStore::insert_project(
        &SqliteProjectRepository::new(Arc::clone(&database)),
        project(),
    )
    .unwrap();
    history
        .save_recording(HistorySaveRecordingRequest {
            segments: json!([{
                "id": "segment-dashboard",
                "text": transcript_text,
                "start": 0.0,
                "end": 2.5,
                "isFinal": true
            }]),
            duration: 2.5,
            project_id: Some("project-dashboard".to_string()),
            audio_bytes: Some(vec![1, 2, 3]),
            native_audio_path: None,
            audio_extension: Some("wav".to_string()),
        })
        .unwrap();
    record_usage(
        database.as_ref(),
        &UsageRecord {
            occurred_at: "2026-07-13T07:00:00Z".to_string(),
            provider: "cli-test".to_string(),
            category: LlmUsageCategory::Summary,
            usage: Some(TokenUsage {
                prompt_tokens: 20,
                completion_tokens: 5,
                total_tokens: 25,
                ..TokenUsage::default()
            }),
        },
    )
    .unwrap();
    database
}

fn seed_empty(app_data_dir: &Path) {
    drop(Database::open(app_data_dir).unwrap());
}

fn run_show(app_data_dir: &str, deep: bool, json: bool) -> sona_cli::CliOutput {
    let mut args = vec![
        "sona-cli",
        "dashboard",
        "show",
        "--app-data-dir",
        app_data_dir,
    ];
    if deep {
        args.push("--deep");
    }
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
                    hex::encode(Sha256::digest(fs::read(path).unwrap())),
                );
            }
        }
    }

    let mut files = BTreeMap::new();
    visit(root, root, &mut files);
    files
}

#[test]
fn dashboard_show_requires_app_data_dir() {
    let error =
        sona_cli::run_cli_from_args(["sona-cli", "dashboard", "show", "--json"]).unwrap_err();

    assert!(matches!(error, sona_cli::CliError::Usage(_)));
    assert_eq!(error.exit_code(), 2);
}

#[test]
fn dashboard_show_does_not_create_missing_app_data() {
    let root = tempfile::tempdir().unwrap();
    let missing = root.path().join("missing");
    let missing_arg = missing.to_string_lossy().into_owned();

    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "dashboard",
        "show",
        "--app-data-dir",
        missing_arg.as_str(),
    ])
    .unwrap_err();

    assert!(matches!(error, sona_cli::CliError::Io(_)));
    assert!(!missing.exists());
}

#[test]
fn dashboard_show_renders_exact_table_columns() {
    let dir = tempfile::tempdir().unwrap();
    seed_empty(dir.path());
    let path = dir.path().to_string_lossy();

    let output = run_show(&path, false, false);
    let lines = output.stdout.lines().collect::<Vec<_>>();

    assert_eq!(output.stderr, "");
    assert_eq!(lines.len(), 3);
    assert_eq!(
        lines[0].split_whitespace().collect::<Vec<_>>(),
        [
            "GENERATED",
            "ITEMS",
            "PROJECTS",
            "DURATION",
            "TOKENS",
            "DEEP"
        ]
    );
    assert!(
        lines[1]
            .chars()
            .all(|character| character == '-' || character == ' ')
    );
    let values = lines[2].split_whitespace().collect::<Vec<_>>();
    assert_eq!(&values[1..], ["0", "0", "0m", "0", "false"]);
}

#[test]
fn dashboard_show_outputs_complete_pretty_json() {
    let dir = tempfile::tempdir().unwrap();
    seed_empty(dir.path());
    let path = dir.path().to_string_lossy();

    let output = run_show(&path, false, true);
    let snapshot: Value = serde_json::from_str(&output.stdout).unwrap();

    assert!(output.stdout.starts_with("{\n"));
    assert!(output.stdout.contains("\n  \"content\""));
    assert!(output.stdout.contains("\n  \"llmUsage\""));
    assert!(output.stdout.contains("\n  \"generatedAt\""));
    assert_eq!(snapshot["content"]["overview"]["itemCount"], 0);
    assert_eq!(snapshot["content"]["overview"]["isDeepLoaded"], false);
}

#[test]
fn dashboard_show_accepts_relative_path_and_deep_unicode() {
    let current = std::env::current_dir().unwrap();
    let dir = tempfile::tempdir_in(&current).unwrap();
    let text = "你好 CLI 🌍";
    let _writer = seed(dir.path(), text);
    let relative = dir.path().strip_prefix(&current).unwrap().to_string_lossy();

    let output = run_show(&relative, true, true);
    let snapshot: Value = serde_json::from_str(&output.stdout).unwrap();

    assert_eq!(snapshot["content"]["overview"]["itemCount"], 1);
    assert_eq!(snapshot["content"]["overview"]["projectCount"], 1);
    assert_eq!(snapshot["content"]["overview"]["isDeepLoaded"], true);
    assert_eq!(
        snapshot["content"]["overview"]["transcriptCharacterCount"],
        text.encode_utf16().count() as u64
    );
    assert_eq!(snapshot["llmUsage"]["totals"]["totalTokens"], 25);
}

#[test]
fn dashboard_show_reads_active_wal_without_modifying_source_files() {
    let dir = tempfile::tempdir().unwrap();
    let _writer = seed(dir.path(), "active WAL");
    for sidecar in [
        "sona.db-wal",
        "sona.db-shm",
        "sona-analytics.db-wal",
        "sona-analytics.db-shm",
    ] {
        assert!(dir.path().join(sidecar).is_file(), "missing {sidecar}");
    }
    let before = file_hashes(dir.path());
    let path = dir.path().to_string_lossy();

    let output = run_show(&path, true, true);
    let snapshot: Value = serde_json::from_str(&output.stdout).unwrap();

    assert_eq!(snapshot["content"]["overview"]["itemCount"], 1);
    assert_eq!(file_hashes(dir.path()), before);
}

#[test]
fn dashboard_show_rejects_future_schema_without_modifying_files() {
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
    let path = dir.path().to_string_lossy();

    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "dashboard",
        "show",
        "--app-data-dir",
        path.as_ref(),
    ])
    .unwrap_err();

    assert!(matches!(error, sona_cli::CliError::Io(_)));
    assert!(error.to_string().contains("99"));
    assert_eq!(file_hashes(dir.path()), before);
}
