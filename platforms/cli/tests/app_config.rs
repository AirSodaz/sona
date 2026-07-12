use sha2::{Digest, Sha256};
use sona_core::config::{AppConfigRepositoryService, AppConfigRepositorySnapshot};
use sona_core::ports::time::UnixMillisClock;
use sona_sqlite::{Database, SqliteConfigStore};
use std::path::Path;
use std::sync::Arc;

struct FixedClock(u64);

impl UnixMillisClock for FixedClock {
    fn now_ms(&self) -> Result<u64, String> {
        Ok(self.0)
    }
}

fn seed_empty(app_data_dir: &Path) {
    drop(Database::open(app_data_dir).unwrap());
}

fn seed_config(app_data_dir: &Path) -> Arc<Database> {
    let database = Arc::new(Database::open(app_data_dir).unwrap());
    let store = SqliteConfigStore::new(Arc::clone(&database));
    let config = serde_json::json!({
        "sona-config": {
            "configVersion": 7,
            "theme": "深色",
            "summaryCustomTemplates": [{
                "id": "summary-1",
                "name": "会议纪要",
                "instructions": "保留行动项"
            }],
            "polishCustomPresets": [
                {"id": "polish-1", "name": "简洁", "context": "工作"},
                {"id": "polish-2", "name": "正式", "context": "客户"}
            ],
            "textReplacementSets": [{
                "id": "replacement-1",
                "name": "产品名",
                "enabled": true,
                "ignoreCase": false,
                "rules": [{"id": "replacement-rule-1", "from": "索娜", "to": "Sona"}]
            }],
            "hotwordSets": [{
                "id": "hotword-1",
                "name": "术语",
                "enabled": true,
                "rules": [{"id": "hotword-rule-1", "text": "六边形架构"}]
            }],
            "polishKeywordSets": [{
                "id": "keyword-1",
                "name": "关键词",
                "enabled": true,
                "keywords": "端口, 适配器"
            }],
            "speakerProfiles": [{
                "id": "speaker-1",
                "name": "张三",
                "enabled": true,
                "samples": [{
                    "id": "sample-1",
                    "filePath": "音频/样本.wav",
                    "sourceName": "访谈",
                    "durationSeconds": 12.5
                }]
            }]
        }
    });
    AppConfigRepositoryService::new(&store, &FixedClock(1_234_567))
        .save_config(&config)
        .unwrap();
    database
}

fn run_show(app_data_dir: &Path, json: bool) -> sona_cli::CliOutput {
    let app_data_dir = app_data_dir.to_string_lossy().into_owned();
    let mut args = vec![
        "sona-cli",
        "app-config",
        "show",
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
fn app_config_show_requires_app_data_dir() {
    let error =
        sona_cli::run_cli_from_args(["sona-cli", "app-config", "show", "--json"]).unwrap_err();

    assert!(matches!(error, sona_cli::CliError::Usage(_)));
    assert_eq!(error.exit_code(), 2);
}

#[test]
fn app_config_show_does_not_create_missing_app_data() {
    let dir = tempfile::tempdir().unwrap();
    let app_data_dir = dir.path().join("missing-app-data");
    let app_data_dir_arg = app_data_dir.to_string_lossy().into_owned();

    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "app-config",
        "show",
        "--app-data-dir",
        app_data_dir_arg.as_str(),
        "--json",
    ])
    .unwrap_err();

    assert!(matches!(error, sona_cli::CliError::Io(_)));
    assert!(!app_data_dir.exists());
}

#[test]
fn app_config_show_outputs_empty_state() {
    let dir = tempfile::tempdir().unwrap();
    seed_empty(dir.path());

    let json = run_show(dir.path(), true);
    let table = run_show(dir.path(), false);

    assert_eq!(json.stderr, "");
    assert_eq!(json.stdout, "null");
    assert_eq!(table.stdout.lines().count(), 2);
    for header in [
        "VERSION",
        "UPDATED",
        "TEMPLATES",
        "PRESETS",
        "VOCABULARY_SETS",
        "SPEAKERS",
    ] {
        assert!(table.stdout.lines().next().unwrap().contains(header));
    }
}

#[test]
fn app_config_show_outputs_complete_pretty_json_with_unicode_values() {
    let dir = tempfile::tempdir().unwrap();
    let _database = seed_config(dir.path());

    let output = run_show(dir.path(), true);
    let snapshot: AppConfigRepositorySnapshot = serde_json::from_str(&output.stdout).unwrap();

    assert_eq!(output.stderr, "");
    assert_eq!(
        output.stdout,
        serde_json::to_string_pretty(&snapshot).unwrap()
    );
    assert_eq!(snapshot.config_version, 7);
    assert_eq!(snapshot.updated_at, 1_234_567);
    assert_eq!(snapshot.summary_template_count, 1);
    assert_eq!(snapshot.polish_preset_count, 2);
    assert_eq!(snapshot.vocabulary_set_count, 3);
    assert_eq!(snapshot.speaker_profile_count, 1);
    assert_eq!(snapshot.config["sona-config"]["theme"], "深色");
    assert_eq!(
        snapshot.config["sona-config"]["speakerProfiles"][0]["samples"][0]["filePath"],
        "音频/样本.wav"
    );
    for key in [
        "config",
        "configVersion",
        "updatedAt",
        "summaryTemplateCount",
        "polishPresetCount",
        "vocabularySetCount",
        "speakerProfileCount",
    ] {
        assert!(
            serde_json::from_str::<serde_json::Value>(&output.stdout)
                .unwrap()
                .get(key)
                .is_some()
        );
    }
}

#[test]
fn app_config_show_outputs_summary_headers_and_counts() {
    let dir = tempfile::tempdir().unwrap();
    let _database = seed_config(dir.path());

    let output = run_show(dir.path(), false);
    let lines = output.stdout.lines().collect::<Vec<_>>();

    assert_eq!(output.stderr, "");
    assert_eq!(lines.len(), 3);
    for header in [
        "VERSION",
        "UPDATED",
        "TEMPLATES",
        "PRESETS",
        "VOCABULARY_SETS",
        "SPEAKERS",
    ] {
        assert!(lines[0].contains(header));
    }
    assert_eq!(
        lines[2].split_whitespace().collect::<Vec<_>>(),
        ["7", "1234567", "1", "2", "3", "1"]
    );
}

#[test]
fn app_config_show_accepts_relative_app_data_dir() {
    let current_dir = std::env::current_dir().unwrap();
    let dir = tempfile::tempdir_in(&current_dir).unwrap();
    let _database = seed_config(dir.path());
    let relative_dir = dir.path().strip_prefix(&current_dir).unwrap();
    let relative_dir = relative_dir.to_string_lossy().into_owned();

    let output = sona_cli::run_cli_from_args([
        "sona-cli",
        "app-config",
        "show",
        "--app-data-dir",
        relative_dir.as_str(),
        "--json",
    ])
    .unwrap();
    let snapshot: AppConfigRepositorySnapshot = serde_json::from_str(&output.stdout).unwrap();

    assert_eq!(snapshot.updated_at, 1_234_567);
}

#[test]
fn app_config_show_reads_active_wal_base_and_library_without_changes() {
    let dir = tempfile::tempdir().unwrap();
    let _database = seed_config(dir.path());
    let wal_path = dir.path().join("sona.db-wal");
    assert!(wal_path.is_file());
    assert!(std::fs::metadata(&wal_path).unwrap().len() > 0);
    let before = directory_files(dir.path());

    let output = run_show(dir.path(), true);
    let snapshot: AppConfigRepositorySnapshot = serde_json::from_str(&output.stdout).unwrap();

    assert_eq!(snapshot.config["sona-config"]["theme"], "深色");
    assert_eq!(snapshot.summary_template_count, 1);
    assert_eq!(snapshot.polish_preset_count, 2);
    assert_eq!(snapshot.vocabulary_set_count, 3);
    assert_eq!(snapshot.speaker_profile_count, 1);
    assert_eq!(directory_files(dir.path()), before);
}

#[test]
fn app_config_show_rejects_future_schema_without_modifying_source_files() {
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
        "app-config",
        "show",
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
fn app_config_show_leaves_directory_filenames_and_hashes_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let _database = seed_config(dir.path());
    std::fs::write(dir.path().join("unrelated.txt"), b"source data").unwrap();
    let before = directory_files(dir.path());

    let output = run_show(dir.path(), false);

    assert!(output.stdout.contains("1234567"));
    assert_eq!(directory_files(dir.path()), before);
}
