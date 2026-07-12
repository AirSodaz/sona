use sha2::{Digest, Sha256};
use sona_core::project::{
    ProjectClock, ProjectCreateInput, ProjectDefaultsInput, ProjectIdGenerator,
    ProjectRepositoryService, ProjectRepositorySnapshot,
};
use sona_sqlite::{Database, SqliteProjectRepository};
use std::path::Path;
use std::sync::{Arc, Mutex};
use unicode_width::UnicodeWidthStr;

struct SequenceIds(Mutex<Vec<String>>);

impl ProjectIdGenerator for SequenceIds {
    fn generate_id(&self) -> String {
        self.0.lock().unwrap().remove(0)
    }
}

struct SequenceClock(Mutex<Vec<u64>>);

impl ProjectClock for SequenceClock {
    fn now_ms(&self) -> Result<u64, String> {
        Ok(self.0.lock().unwrap().remove(0))
    }
}

fn project_input(name: &str, linked_defaults: bool) -> ProjectCreateInput {
    ProjectCreateInput {
        name: name.to_string(),
        description: Some(format!("Description for {name}")),
        icon: Some("folder".to_string()),
        defaults: ProjectDefaultsInput {
            enabled_text_replacement_set_ids: linked_defaults
                .then(|| vec!["replacement-1".to_string()]),
            enabled_hotword_set_ids: linked_defaults.then(|| vec!["hotword-1".to_string()]),
            enabled_polish_keyword_set_ids: linked_defaults.then(|| vec!["keyword-1".to_string()]),
            enabled_speaker_profile_ids: linked_defaults.then(|| vec!["speaker-1".to_string()]),
            ..ProjectDefaultsInput::default()
        },
    }
}

fn seed_empty(app_data_dir: &Path) {
    let database = Arc::new(Database::open(app_data_dir).unwrap());
    let repository = SqliteProjectRepository::new(database);
    let ids = SequenceIds(Mutex::new(Vec::new()));
    let clock = SequenceClock(Mutex::new(Vec::new()));
    ProjectRepositoryService::new(&repository, &ids, &clock)
        .replace_projects_json(Vec::new())
        .unwrap();
}

fn seed_projects(app_data_dir: &Path, names: [&str; 2]) -> Arc<Database> {
    let database = Arc::new(Database::open(app_data_dir).unwrap());
    let repository = SqliteProjectRepository::new(Arc::clone(&database));
    let ids = SequenceIds(Mutex::new(vec!["project-1".into(), "project-2".into()]));
    let clock = SequenceClock(Mutex::new(vec![100, 200]));
    let service = ProjectRepositoryService::new(&repository, &ids, &clock);
    service
        .create_project(project_input(names[0], true))
        .unwrap();
    service
        .create_project(project_input(names[1], false))
        .unwrap();
    service
        .set_active_project_id(Some("project-2".to_string()))
        .unwrap();
    database
}

fn run_list(app_data_dir: &Path, json: bool) -> sona_cli::CliOutput {
    let app_data_dir = app_data_dir.to_string_lossy().into_owned();
    let mut args = vec![
        "sona-cli",
        "projects",
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
fn projects_list_requires_app_data_dir() {
    let error =
        sona_cli::run_cli_from_args(["sona-cli", "projects", "list", "--json"]).unwrap_err();

    assert!(matches!(error, sona_cli::CliError::Usage(_)));
    assert_eq!(error.exit_code(), 2);
}

#[test]
fn projects_list_outputs_empty_canonical_json() {
    let dir = tempfile::tempdir().unwrap();
    seed_empty(dir.path());

    let output = run_list(dir.path(), true);

    assert_eq!(output.stderr, "");
    assert_eq!(
        output.stdout,
        "{\n  \"projects\": [],\n  \"activeProjectId\": null\n}"
    );
}

#[test]
fn projects_list_outputs_populated_canonical_json_with_active_project() {
    let dir = tempfile::tempdir().unwrap();
    let _database = seed_projects(dir.path(), ["First", "Second"]);

    let output = run_list(dir.path(), true);
    let state: ProjectRepositorySnapshot = serde_json::from_str(&output.stdout).unwrap();

    assert_eq!(state.projects.len(), 2);
    assert_eq!(state.projects[0].id, "project-1");
    assert_eq!(state.projects[1].updated_at, 200);
    assert_eq!(state.active_project_id.as_deref(), Some("project-2"));
    assert_eq!(output.stdout, serde_json::to_string_pretty(&state).unwrap());
    assert!(output.stdout.contains("\"activeProjectId\": \"project-2\""));
    assert!(!output.stdout.contains("active_project_id"));
}

#[test]
fn projects_list_outputs_headers_values_and_selected_active_marker() {
    let dir = tempfile::tempdir().unwrap();
    let _database = seed_projects(dir.path(), ["First", "Second"]);

    let output = run_list(dir.path(), false);
    let lines = output.stdout.lines().collect::<Vec<_>>();

    assert_eq!(output.stderr, "");
    for header in ["ID", "NAME", "ACTIVE", "UPDATED"] {
        assert!(lines[0].contains(header));
    }
    let first = lines
        .iter()
        .find(|line| line.contains("project-1"))
        .unwrap();
    let second = lines
        .iter()
        .find(|line| line.contains("project-2"))
        .unwrap();
    for value in ["project-1", "First", "false", "100"] {
        assert!(first.contains(value));
    }
    for value in ["project-2", "Second", "true", "200"] {
        assert!(second.contains(value));
    }
    assert!(!first.contains("true"));
    assert!(!second.contains("false"));
}

#[test]
fn projects_table_escapes_control_characters() {
    let dir = tempfile::tempdir().unwrap();
    let _database = seed_projects(dir.path(), ["row\nname\t\u{1b}", "plain\rname\u{7}"]);

    let output = run_list(dir.path(), false);

    assert_eq!(output.stdout.lines().count(), 4);
    assert!(output.stdout.contains(r"row\nname\t\u{1b}"));
    assert!(output.stdout.contains(r"plain\rname\u{7}"));
    assert!(
        !output
            .stdout
            .chars()
            .any(|character| character != '\n' && character.is_control())
    );
}

#[test]
fn projects_table_aligns_unicode_names_by_display_width() {
    let dir = tempfile::tempdir().unwrap();
    let _database = seed_projects(dir.path(), ["会议项目", "meeting1"]);

    let output = run_list(dir.path(), false);
    let active_columns = output
        .stdout
        .lines()
        .skip(2)
        .map(|line| {
            let marker = if line.contains("false") {
                "false"
            } else {
                "true"
            };
            UnicodeWidthStr::width(&line[..line.find(marker).unwrap()])
        })
        .collect::<Vec<_>>();

    assert_eq!(active_columns.len(), 2);
    assert_eq!(active_columns[0], active_columns[1]);
}

#[test]
fn projects_list_accepts_relative_app_data_dir() {
    let current_dir = std::env::current_dir().unwrap();
    let dir = tempfile::tempdir_in(&current_dir).unwrap();
    let _database = seed_projects(dir.path(), ["Relative", "Second"]);
    let relative_dir = dir.path().strip_prefix(&current_dir).unwrap();
    let relative_dir = relative_dir.to_string_lossy().into_owned();

    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "projects",
        "list",
        "--app-data-dir",
        relative_dir.as_str(),
        "--json",
    ])
    .unwrap();
    let state: ProjectRepositorySnapshot = serde_json::from_str(&output.stdout).unwrap();

    assert_eq!(state.projects[0].id, "project-1");
}

#[test]
fn projects_list_does_not_create_missing_app_data() {
    let dir = tempfile::tempdir().unwrap();
    let app_data_dir = dir.path().join("missing-app-data");
    let app_data_dir_arg = app_data_dir.to_string_lossy().into_owned();

    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "projects",
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
fn projects_list_reads_active_wal_rows_links_and_setting_without_changes() {
    let dir = tempfile::tempdir().unwrap();
    let _database = seed_projects(dir.path(), ["Committed in WAL", "Second"]);
    let wal_path = dir.path().join("sona.db-wal");
    assert!(wal_path.is_file());
    assert!(std::fs::metadata(&wal_path).unwrap().len() > 0);
    let before = directory_files(dir.path());

    let output = run_list(dir.path(), true);
    let state: ProjectRepositorySnapshot = serde_json::from_str(&output.stdout).unwrap();

    assert_eq!(state.projects[0].name, "Committed in WAL");
    assert_eq!(
        state.projects[0].defaults.enabled_text_replacement_set_ids,
        ["replacement-1"]
    );
    assert_eq!(
        state.projects[0].defaults.enabled_hotword_set_ids,
        ["hotword-1"]
    );
    assert_eq!(
        state.projects[0].defaults.enabled_polish_keyword_set_ids,
        ["keyword-1"]
    );
    assert_eq!(
        state.projects[0].defaults.enabled_speaker_profile_ids,
        ["speaker-1"]
    );
    assert_eq!(state.active_project_id.as_deref(), Some("project-2"));
    assert_eq!(directory_files(dir.path()), before);
}

#[test]
fn projects_list_rejects_future_schema_without_modifying_source_files() {
    let dir = tempfile::tempdir().unwrap();
    seed_empty(dir.path());
    let database = Database::open(dir.path()).unwrap();
    database
        .with_connection(|connection| {
            connection.execute("INSERT INTO schema_version (version) VALUES (2)", [])?;
            Ok(())
        })
        .unwrap();
    drop(database);
    let before = directory_files(dir.path());
    let app_data_dir = dir.path().to_string_lossy().into_owned();

    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "projects",
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
            .contains("Unsupported database schema version 2")
    );
    assert_eq!(directory_files(dir.path()), before);
}

#[test]
fn projects_list_leaves_directory_filenames_and_hashes_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let _database = seed_projects(dir.path(), ["Read only", "Second"]);
    std::fs::write(dir.path().join("unrelated.txt"), b"source data").unwrap();
    let before = directory_files(dir.path());

    let output = run_list(dir.path(), false);

    assert!(output.stdout.contains("project-1"));
    assert_eq!(directory_files(dir.path()), before);
}
