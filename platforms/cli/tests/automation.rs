use serde_json::json;
use sha2::{Digest, Sha256};
use sona_core::automation::repository::{AutomationRepositoryInput, AutomationRepositoryState};
use sona_core::automation::service::{AutomationIdGenerator, AutomationRepositoryService};
use sona_sqlite::{Database, SqliteAutomationRepository};
use std::path::Path;
use std::sync::{Arc, Mutex};
use unicode_width::UnicodeWidthStr;

struct SequenceIds(Mutex<Vec<String>>);

impl AutomationIdGenerator for SequenceIds {
    fn generate_id(&self) -> String {
        self.0.lock().unwrap().remove(0)
    }
}

fn state_json(rule_name: &str, watch_directory: &str) -> AutomationRepositoryInput {
    serde_json::from_value(json!({
        "rules": [{
            "name": rule_name,
            "tagIds": [],
            "presetId": "preset-1",
            "watchDirectory": watch_directory,
            "recursive": true,
            "enabled": true,
            "stageConfig": {
                "autoPolish": true,
                "polishPresetId": "polish-1",
                "autoTranslate": false,
                "translationLanguage": "en",
                "exportEnabled": true
            },
            "exportConfig": {
                "directory": "C:\\export",
                "format": "srt",
                "mode": "polished",
                "prefix": "done-"
            },
            "createdAt": 100,
            "updatedAt": 200
        }],
        "processedEntries": [{
            "ruleId": "rule-1",
            "filePath": "C:\\watch\\audio.wav",
            "sourceFingerprint": "fingerprint",
            "size": 42,
            "mtimeMs": 300,
            "status": "complete",
            "processedAt": 400,
            "historyId": "history-1",
            "exportPath": "C:\\export\\audio.srt"
        }]
    }))
    .unwrap()
}

fn seed(app_data_dir: &Path, rule_name: &str, watch_directory: &str) -> Arc<Database> {
    let database = Arc::new(Database::open(app_data_dir).unwrap());
    let repository = SqliteAutomationRepository::new(Arc::clone(&database));
    let ids = SequenceIds(Mutex::new(vec!["rule-1".into(), "entry-1".into()]));
    AutomationRepositoryService::new(&repository, &ids)
        .replace_state(state_json(rule_name, watch_directory))
        .unwrap();
    database
}

fn seed_empty(app_data_dir: &Path) {
    let database = Arc::new(Database::open(app_data_dir).unwrap());
    let repository = SqliteAutomationRepository::new(database);
    let ids = SequenceIds(Mutex::new(Vec::new()));
    AutomationRepositoryService::new(&repository, &ids)
        .replace_state(AutomationRepositoryInput::default())
        .unwrap();
}

fn run_list(app_data_dir: &Path, json: bool) -> sona_cli::CliOutput {
    let app_data_dir = app_data_dir.to_string_lossy().into_owned();
    let mut args = vec![
        "sona-cli",
        "automation",
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
        .filter_map(|entry| {
            let entry = entry.unwrap();
            entry.file_type().unwrap().is_file().then(|| {
                (
                    entry.file_name().to_string_lossy().into_owned(),
                    hex::encode(Sha256::digest(std::fs::read(entry.path()).unwrap())),
                )
            })
        })
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.0.cmp(&right.0));
    files
}

#[test]
fn automation_list_requires_app_data_dir() {
    let error =
        sona_cli::run_cli_from_args(["sona-cli", "automation", "list", "--json"]).unwrap_err();

    assert!(matches!(error, sona_cli::CliError::Usage(_)));
    assert_eq!(error.exit_code(), 2);
}

#[test]
fn automation_list_outputs_empty_canonical_json() {
    let dir = tempfile::tempdir().unwrap();
    seed_empty(dir.path());

    let output = run_list(dir.path(), true);
    let state: AutomationRepositoryState = serde_json::from_str(&output.stdout).unwrap();

    assert_eq!(output.stderr, "");
    assert!(state.rules.is_empty());
    assert!(state.processed_entries.is_empty());
    assert_eq!(
        output.stdout,
        "{\n  \"rules\": [],\n  \"processedEntries\": []\n}"
    );
    assert!(!output.stdout.contains("processed_entries"));
}

#[test]
fn automation_list_outputs_populated_canonical_json() {
    let dir = tempfile::tempdir().unwrap();
    let _database = seed(dir.path(), "Morning import", "C:\\watch");

    let output = run_list(dir.path(), true);
    let state: AutomationRepositoryState = serde_json::from_str(&output.stdout).unwrap();

    assert_eq!(state.rules[0].id, "rule-1");
    assert_eq!(state.rules[0].name, "Morning import");
    assert_eq!(state.processed_entries[0].id, "entry-1");
    assert_eq!(state.processed_entries[0].rule_id, "rule-1");
    assert!(
        output
            .stdout
            .contains("\"watchDirectory\": \"C:\\\\watch\"")
    );
    assert!(output.stdout.contains("\"processedEntries\""));
    assert!(!output.stdout.contains("processed_entries"));
}

#[test]
fn automation_list_outputs_rules_table_and_processed_summary() {
    let dir = tempfile::tempdir().unwrap();
    let _database = seed(dir.path(), "Morning import", "C:\\watch");

    let output = run_list(dir.path(), false);

    assert_eq!(output.stderr, "");
    for header in ["ID", "NAME", "TAGS", "ENABLED", "WATCH"] {
        assert!(output.stdout.contains(header));
    }
    for value in ["rule-1", "Morning import", "true", "C:\\watch"] {
        assert!(output.stdout.contains(value));
    }
    assert!(output.stdout.ends_with("Processed entries: 1\n"));
}

#[test]
fn automation_list_empty_table_has_processed_summary() {
    let dir = tempfile::tempdir().unwrap();
    seed_empty(dir.path());

    let output = run_list(dir.path(), false);

    assert_eq!(output.stdout.lines().count(), 3);
    assert!(output.stdout.ends_with("Processed entries: 0\n"));
}

#[test]
fn automation_table_escapes_control_characters() {
    let dir = tempfile::tempdir().unwrap();
    let _database = seed(dir.path(), "row\nname\t\u{1b}", "watch\rdir\u{7}");

    let output = run_list(dir.path(), false);

    assert_eq!(output.stdout.lines().count(), 4);
    assert!(output.stdout.contains(r"row\nname\t\u{1b}"));
    assert!(output.stdout.contains(r"watch\rdir\u{7}"));
    assert!(
        !output
            .stdout
            .chars()
            .any(|character| character != '\n' && character.is_control())
    );
}

#[test]
fn automation_table_aligns_unicode_names_by_display_width() {
    let dir = tempfile::tempdir().unwrap();
    let database = seed(dir.path(), "会议规则", "C:\\watch-one");
    let repository = SqliteAutomationRepository::new(Arc::clone(&database));
    let ids = SequenceIds(Mutex::new(vec!["rule-2".into(), "entry-2".into()]));
    let rules = state_json("meeting1", "C:\\watch-two").rules;
    AutomationRepositoryService::new(&repository, &ids)
        .replace_rules(vec![
            state_json("会议规则", "C:\\watch-one").rules.remove(0),
            rules.into_iter().next().unwrap(),
        ])
        .unwrap();

    let output = run_list(dir.path(), false);
    let tag_columns = output
        .stdout
        .lines()
        .skip(2)
        .take(2)
        .map(|line| UnicodeWidthStr::width(&line[..line.find("true").unwrap()]))
        .collect::<Vec<_>>();

    assert_eq!(tag_columns.len(), 2);
    assert_eq!(tag_columns[0], tag_columns[1]);
}

#[test]
fn automation_list_accepts_relative_app_data_dir() {
    let current_dir = std::env::current_dir().unwrap();
    let dir = tempfile::tempdir_in(&current_dir).unwrap();
    let _database = seed(dir.path(), "Relative", "relative-watch");
    let relative_dir = dir.path().strip_prefix(&current_dir).unwrap();
    let relative_dir = relative_dir.to_string_lossy().into_owned();

    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "automation",
        "list",
        "--app-data-dir",
        relative_dir.as_str(),
        "--json",
    ])
    .unwrap();
    let state: AutomationRepositoryState = serde_json::from_str(&output.stdout).unwrap();

    assert_eq!(state.rules[0].id, "rule-1");
}

#[test]
fn automation_list_does_not_create_missing_app_data() {
    let dir = tempfile::tempdir().unwrap();
    let app_data_dir = dir.path().join("missing-app-data");
    let app_data_dir_arg = app_data_dir.to_string_lossy().into_owned();

    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "automation",
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
fn automation_list_leaves_database_directory_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let _database = seed(dir.path(), "Read only", "C:\\watch");
    let before = directory_files(dir.path());

    let output = run_list(dir.path(), true);

    assert!(output.stdout.contains("rule-1"));
    assert_eq!(directory_files(dir.path()), before);
}

#[test]
fn automation_list_reads_active_wal_without_modifying_files() {
    let dir = tempfile::tempdir().unwrap();
    let _database = seed(dir.path(), "Committed in WAL", "C:\\watch");
    let wal_path = dir.path().join("sona.db-wal");
    assert!(wal_path.is_file());
    assert!(std::fs::metadata(&wal_path).unwrap().len() > 0);
    let before = directory_files(dir.path());

    let output = run_list(dir.path(), true);
    let state: AutomationRepositoryState = serde_json::from_str(&output.stdout).unwrap();

    assert_eq!(state.rules[0].name, "Committed in WAL");
    assert_eq!(directory_files(dir.path()), before);
}

#[test]
fn automation_list_rejects_future_database_schema_without_modifying_files() {
    let dir = tempfile::tempdir().unwrap();
    seed_empty(dir.path());
    let database = Database::open(dir.path()).unwrap();
    let future_version = database
        .with_connection(|connection| {
            let current_version =
                connection.query_row("SELECT MAX(version) FROM schema_version", [], |row| {
                    row.get::<_, i64>(0)
                })?;
            let future_version = current_version + 1;
            connection.execute(
                "INSERT INTO schema_version (version) VALUES (?1)",
                [future_version],
            )?;
            Ok(future_version)
        })
        .unwrap();
    drop(database);
    let before = directory_files(dir.path());
    let app_data_dir = dir.path().to_string_lossy().into_owned();

    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "automation",
        "list",
        "--app-data-dir",
        app_data_dir.as_str(),
        "--json",
    ])
    .unwrap_err();

    assert!(matches!(error, sona_cli::CliError::Io(_)));
    assert!(error.to_string().contains(&format!(
        "Unsupported database schema version {future_version}"
    )));
    assert_eq!(directory_files(dir.path()), before);
}
