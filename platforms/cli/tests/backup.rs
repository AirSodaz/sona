use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use sona_core::automation::repository::{
    AutomationProcessedRecord, AutomationRepositoryState, AutomationRuleRecord,
    AutomationRuleRecordExportConfig, AutomationRuleRecordStageConfig, AutomationStore,
};
use sona_core::backup::{
    BackupApplyResult, BackupManifest, BackupStateRepository, PreparedBackupImport,
};
use sona_core::config::service::app_config_stored_state_from_value;
use sona_core::config::{AppConfigStore, CURRENT_CONFIG_VERSION};
use sona_core::history::HistorySaveRecordingRequest;
use sona_core::history::mutation_repository::HistoryMutationRepository;
use sona_core::history_store::HistoryStore;
use sona_core::project::{ProjectDefaults, ProjectRecord, ProjectStore};
use sona_sqlite::{
    Database, SqliteAutomationRepository, SqliteBackupStateRepository, SqliteConfigStore,
    SqliteHistoryStore, SqliteProjectRepository, llm_usage,
};

fn rewrite_archive(archive: &Path, mutate: impl FnOnce(&Path)) {
    let workspace = tempfile::tempdir().unwrap();
    sona_archive::extract_tar_bz2(
        path_arg(archive).as_str(),
        path_arg(workspace.path()).as_str(),
        |_| {},
    )
    .unwrap();
    mutate(workspace.path());
    sona_archive::create_tar_bz2(
        path_arg(workspace.path()).as_str(),
        path_arg(archive).as_str(),
    )
    .unwrap();
}

fn update_json(path: &Path, mutate: impl FnOnce(&mut Value)) {
    let mut value = serde_json::from_slice::<Value>(&fs::read(path).unwrap()).unwrap();
    mutate(&mut value);
    fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
}

fn path_arg(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn canonical_output<T>(output: &sona_cli::CliOutput) -> T
where
    T: DeserializeOwned + Serialize,
{
    assert_eq!(output.stderr, "");
    assert!(!output.stdout.contains('\n'));
    assert!(!output.stdout.contains("  "));
    let parsed = serde_json::from_str::<T>(&output.stdout).unwrap();
    assert_eq!(
        output.stdout,
        serde_json::to_string(&serde_json::to_value(&parsed).unwrap()).unwrap()
    );
    parsed
}

fn legacy_config(label: &str) -> Value {
    json!({
        "configVersion": 1,
        "sourceMarker": label,
        "textReplacements": [{"from": "Sonna", "to": "Sona"}],
        "summaryCustomTemplates": [],
        "polishCustomPresets": [],
        "textReplacementSets": [],
        "hotwordSets": [],
        "polishKeywordSets": [],
        "speakerProfiles": []
    })
}

fn project() -> ProjectRecord {
    ProjectRecord {
        id: "backup-project".to_string(),
        name: "Backup Project".to_string(),
        description: "Roundtrip workspace".to_string(),
        icon: "folder".to_string(),
        created_at: 100,
        updated_at: 200,
        defaults: ProjectDefaults {
            summary_template_id: "general".to_string(),
            translation_language: "en".to_string(),
            polish_preset_id: "general".to_string(),
            polish_scenario: None,
            polish_context: None,
            export_file_name_prefix: "backup-".to_string(),
            enabled_text_replacement_set_ids: Vec::new(),
            enabled_hotword_set_ids: Vec::new(),
            enabled_polish_keyword_set_ids: Vec::new(),
            enabled_speaker_profile_ids: Vec::new(),
        },
    }
}

fn automation_state(history_id: &str) -> AutomationRepositoryState {
    AutomationRepositoryState {
        rules: vec![AutomationRuleRecord {
            id: "backup-rule".to_string(),
            name: "Backup Rule".to_string(),
            project_id: "backup-project".to_string(),
            preset_id: "general".to_string(),
            watch_directory: "C:/backup/watch".to_string(),
            recursive: true,
            enabled: true,
            stage_config: AutomationRuleRecordStageConfig {
                auto_polish: false,
                polish_preset_id: "general".to_string(),
                auto_translate: true,
                translation_language: "en".to_string(),
                export_enabled: true,
            },
            export_config: AutomationRuleRecordExportConfig {
                directory: "C:/backup/export".to_string(),
                format: "srt".to_string(),
                mode: "original".to_string(),
                prefix: "done-".to_string(),
            },
            created_at: 300,
            updated_at: 400,
        }],
        processed_entries: vec![AutomationProcessedRecord {
            id: "backup-entry".to_string(),
            rule_id: "backup-rule".to_string(),
            file_path: "C:/backup/watch/audio.wav".to_string(),
            source_fingerprint: "backup-fingerprint".to_string(),
            size: 42,
            mtime_ms: 500,
            status: "complete".to_string(),
            processed_at: 600,
            history_id: Some(history_id.to_string()),
            export_path: Some("C:/backup/export/audio.srt".to_string()),
            error_message: None,
        }],
    }
}

fn seed_five_scopes(app_data_dir: &Path) -> sona_core::backup::BackupDataset {
    let database = Arc::new(Database::open(app_data_dir).unwrap());
    SqliteConfigStore::new(Arc::clone(&database))
        .replace_state(app_config_stored_state_from_value(&legacy_config("source"), 1).unwrap())
        .unwrap();
    SqliteProjectRepository::new(Arc::clone(&database))
        .replace_projects(vec![project()])
        .unwrap();

    let history_store = SqliteHistoryStore::new(app_data_dir.to_path_buf(), Arc::clone(&database));
    history_store.ensure_ready().unwrap();
    let history_item = history_store
        .save_recording(HistorySaveRecordingRequest {
            segments: json!([{
                "id": "backup-segment",
                "text": "Five scope backup",
                "start": 0.0,
                "end": 1.0,
                "isFinal": true
            }]),
            duration: 1.0,
            project_id: Some("backup-project".to_string()),
            audio_bytes: Some(vec![1, 2, 3]),
            native_audio_path: None,
            audio_extension: None,
        })
        .unwrap();
    history_store
        .save_summary(&history_item.id, json!({"summary": "Backup summary"}))
        .unwrap();

    SqliteAutomationRepository::new(Arc::clone(&database))
        .replace_state(&automation_state(&history_item.id))
        .unwrap();
    llm_usage::replace_raw(
        database.as_ref(),
        &json!([{
            "occurredAt": "2026-07-14T00:00:00Z",
            "provider": "backup-provider",
            "category": "summary",
            "promptTokens": 10,
            "completionTokens": 2,
            "totalTokens": 12
        }])
        .to_string(),
    )
    .unwrap();

    SqliteBackupStateRepository::new(app_data_dir.to_path_buf(), database)
        .snapshot()
        .unwrap()
}

fn snapshot(app_data_dir: &Path) -> sona_core::backup::BackupDataset {
    let database = Arc::new(Database::open(app_data_dir).unwrap());
    SqliteBackupStateRepository::new(app_data_dir.to_path_buf(), database)
        .snapshot()
        .unwrap()
}

fn export(app_data_dir: &Path, archive: &Path, app_version: &str) -> sona_cli::CliOutput {
    sona_cli::run_cli_from_args([
        "sona-cli",
        "backup",
        "export",
        "--app-data-dir",
        path_arg(app_data_dir).as_str(),
        "--output",
        path_arg(archive).as_str(),
        "--app-version",
        app_version,
    ])
    .unwrap()
}

fn inspect(archive: &Path) -> sona_cli::CliOutput {
    sona_cli::run_cli_from_args([
        "sona-cli",
        "backup",
        "inspect",
        "--archive",
        path_arg(archive).as_str(),
    ])
    .unwrap()
}

fn import(app_data_dir: &Path, archive: &Path, default_rule_set_name: &str) -> sona_cli::CliOutput {
    sona_cli::run_cli_from_args([
        "sona-cli",
        "backup",
        "import",
        "--app-data-dir",
        path_arg(app_data_dir).as_str(),
        "--archive",
        path_arg(archive).as_str(),
        "--default-rule-set-name",
        default_rule_set_name,
        "--confirm-replace",
    ])
    .unwrap()
}

fn prepared_workspace_names(archive: &Path) -> Vec<String> {
    let file_name = archive.file_name().unwrap().to_string_lossy();
    let prefix = format!(".{file_name}.sona-prepare-");
    let mut names = fs::read_dir(std::env::temp_dir())
        .unwrap()
        .filter_map(|entry| {
            let name = entry.ok()?.file_name().to_string_lossy().into_owned();
            name.starts_with(&prefix).then_some(name)
        })
        .collect::<Vec<_>>();
    names.sort();
    names
}

fn directory_entries(path: &Path) -> Vec<String> {
    if !path.exists() {
        return Vec::new();
    }
    let mut entries = fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

#[test]
fn backup_export_inspect_import_roundtrips_five_scopes_and_migrates_config() {
    let root = tempfile::tempdir().unwrap();
    let source_dir = root.path().join("source");
    let destination_dir = root.path().join("destination");
    let archive = root.path().join("roundtrip.sona-backup");
    fs::create_dir_all(&source_dir).unwrap();
    fs::create_dir_all(&destination_dir).unwrap();
    let source = seed_five_scopes(&source_dir);

    let manifest: BackupManifest = canonical_output(&export(&source_dir, &archive, "0.8.0"));
    assert_eq!(manifest.app_version, "0.8.0");
    assert_eq!(manifest.counts.projects, 1);
    assert_eq!(manifest.counts.history_items, 1);
    assert_eq!(manifest.counts.automation_rules, 1);
    assert_eq!(manifest.counts.automation_processed_entries, 1);
    assert_eq!(manifest.counts.analytics_files, 1);
    assert!(manifest.scopes.config);
    assert!(manifest.scopes.workspace);
    assert!(manifest.scopes.history);
    assert!(manifest.scopes.automation);
    assert!(manifest.scopes.analytics);

    let before_inspect = prepared_workspace_names(&archive);
    let preview: PreparedBackupImport = canonical_output(&inspect(&archive));
    assert_eq!(preview.manifest, manifest);
    assert_eq!(preview.projects.len(), 1);
    assert_eq!(preview.automation_rules.len(), 1);
    assert_eq!(preview.automation_processed_entries.len(), 1);
    assert_eq!(prepared_workspace_names(&archive), before_inspect);
    assert_eq!(directory_entries(&destination_dir), Vec::<String>::new());

    let result: BackupApplyResult =
        canonical_output(&import(&destination_dir, &archive, "Imported Rules"));
    assert_eq!(result.manifest, manifest);
    assert_eq!(prepared_workspace_names(&archive), before_inspect);

    let restored = snapshot(&destination_dir);
    assert_eq!(restored.projects, source.projects);
    assert_eq!(restored.history.items, source.history.items);
    assert_eq!(
        restored.history.transcript_files,
        source.history.transcript_files
    );
    assert_eq!(restored.history.summary_files, source.history.summary_files);
    assert_eq!(
        restored.history.snapshot_files,
        source.history.snapshot_files
    );
    assert_eq!(restored.automation, source.automation);
    assert_eq!(restored.analytics_content, source.analytics_content);
    assert_eq!(
        restored.config["configVersion"],
        json!(CURRENT_CONFIG_VERSION)
    );
    assert_eq!(restored.config["sourceMarker"], "source");
    assert_eq!(
        restored.config["textReplacementSets"][0]["id"],
        "default-set"
    );
    assert_eq!(
        restored.config["textReplacementSets"][0]["name"],
        "Imported Rules"
    );
}

#[test]
fn backup_export_atomically_replaces_an_existing_regular_file() {
    let root = tempfile::tempdir().unwrap();
    let app_data_dir = root.path().join("source");
    let archive = root.path().join("existing.sona-backup");
    fs::create_dir_all(&app_data_dir).unwrap();
    seed_five_scopes(&app_data_dir);
    fs::write(&archive, b"existing content").unwrap();

    let manifest: BackupManifest = canonical_output(&export(&app_data_dir, &archive, "0.8.1"));
    let preview: PreparedBackupImport = canonical_output(&inspect(&archive));

    assert_ne!(fs::read(&archive).unwrap(), b"existing content");
    assert_eq!(manifest.app_version, "0.8.1");
    assert_eq!(preview.manifest, manifest);
}

#[test]
fn backup_missing_and_invalid_archives_fail_before_database_or_residue() {
    let root = tempfile::tempdir().unwrap();
    let app_data_dir = root.path().join("empty-destination");
    let missing_archive = root.path().join("missing.sona-backup");
    let invalid_archive = root.path().join("invalid.sona-backup");
    fs::create_dir_all(&app_data_dir).unwrap();
    fs::write(&invalid_archive, b"not an archive").unwrap();

    let missing = sona_cli::run_cli_from_args([
        "sona-cli",
        "backup",
        "import",
        "--app-data-dir",
        path_arg(&app_data_dir).as_str(),
        "--archive",
        path_arg(&missing_archive).as_str(),
        "--default-rule-set-name",
        "Imported Rules",
        "--confirm-replace",
    ])
    .unwrap_err();
    assert!(matches!(missing, sona_cli::CliError::Validation(_)));
    assert!(missing.to_string().contains("archive"));
    assert!(directory_entries(&app_data_dir).is_empty());

    let before = prepared_workspace_names(&invalid_archive);
    let invalid = sona_cli::run_cli_from_args([
        "sona-cli",
        "backup",
        "import",
        "--app-data-dir",
        path_arg(&app_data_dir).as_str(),
        "--archive",
        path_arg(&invalid_archive).as_str(),
        "--default-rule-set-name",
        "Imported Rules",
        "--confirm-replace",
    ])
    .unwrap_err();
    assert!(matches!(invalid, sona_cli::CliError::Io(_)));
    assert!(invalid.to_string().contains("Backup archive error:"));
    assert!(directory_entries(&app_data_dir).is_empty());
    assert_eq!(prepared_workspace_names(&invalid_archive), before);

    let inspect_error = sona_cli::run_cli_from_args([
        "sona-cli",
        "backup",
        "inspect",
        "--archive",
        path_arg(&invalid_archive).as_str(),
    ])
    .unwrap_err();
    assert!(matches!(inspect_error, sona_cli::CliError::Io(_)));
    assert_eq!(prepared_workspace_names(&invalid_archive), before);
}

#[test]
fn backup_semantic_validation_fails_before_target_database_or_lock_creation() {
    let root = tempfile::tempdir().unwrap();
    let source_dir = root.path().join("source");
    let destination_dir = root.path().join("empty-destination");
    fs::create_dir_all(&source_dir).unwrap();
    fs::create_dir_all(&destination_dir).unwrap();
    seed_five_scopes(&source_dir);

    let duplicate_projects = root.path().join("duplicate-projects.sona-backup");
    export(&source_dir, &duplicate_projects, "0.8.0");
    rewrite_archive(&duplicate_projects, |contents| {
        update_json(&contents.join("projects/index.json"), |projects| {
            let projects = projects.as_array_mut().unwrap();
            projects.push(projects[0].clone());
        });
        update_json(&contents.join("manifest.json"), |manifest| {
            manifest["counts"]["projects"] = json!(2);
        });
    });

    let unknown_project = root.path().join("unknown-project.sona-backup");
    export(&source_dir, &unknown_project, "0.8.0");
    rewrite_archive(&unknown_project, |contents| {
        update_json(&contents.join("history/index.json"), |history| {
            history[0]["projectId"] = json!("missing-project");
        });
    });

    let duplicate_automation = root.path().join("duplicate-automation.sona-backup");
    export(&source_dir, &duplicate_automation, "0.8.0");
    rewrite_archive(&duplicate_automation, |contents| {
        update_json(&contents.join("automation/rules.json"), |rules| {
            let rules = rules.as_array_mut().unwrap();
            rules.push(rules[0].clone());
        });
        update_json(&contents.join("manifest.json"), |manifest| {
            manifest["counts"]["automationRules"] = json!(2);
        });
    });

    for archive in [&duplicate_projects, &unknown_project, &duplicate_automation] {
        let archive_before = fs::read(&archive).unwrap();
        let target_before = directory_entries(&destination_dir);
        let prepared_before = prepared_workspace_names(&archive);
        let error = sona_cli::run_cli_from_args([
            "sona-cli",
            "backup",
            "import",
            "--app-data-dir",
            path_arg(&destination_dir).as_str(),
            "--archive",
            path_arg(archive).as_str(),
            "--default-rule-set-name",
            "Imported Rules",
            "--confirm-replace",
        ])
        .unwrap_err();

        assert!(matches!(error, sona_cli::CliError::Validation(_)));
        assert!(error.to_string().contains("Invalid backup:"));
        assert_eq!(directory_entries(&destination_dir), target_before);
        assert_eq!(fs::read(archive).unwrap(), archive_before);
        assert_eq!(prepared_workspace_names(archive), prepared_before);
        for forbidden in [
            "sona.db",
            "sona.db-wal",
            "sona.db-shm",
            "sona-analytics.db",
            "sona-analytics.db-wal",
            "sona-analytics.db-shm",
            ".sona-history.lock",
        ] {
            assert!(!destination_dir.join(forbidden).exists());
        }
    }

    let missing_destination = root.path().join("missing-destination");
    let archive_before = fs::read(&duplicate_projects).unwrap();
    let prepared_before = prepared_workspace_names(&duplicate_projects);
    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "backup",
        "import",
        "--app-data-dir",
        path_arg(&missing_destination).as_str(),
        "--archive",
        path_arg(&duplicate_projects).as_str(),
        "--default-rule-set-name",
        "Imported Rules",
        "--confirm-replace",
    ])
    .unwrap_err();
    assert!(matches!(error, sona_cli::CliError::Validation(_)));
    assert!(error.to_string().contains("Invalid backup:"));
    assert!(!missing_destination.exists());
    assert_eq!(fs::read(&duplicate_projects).unwrap(), archive_before);
    assert_eq!(
        prepared_workspace_names(&duplicate_projects),
        prepared_before
    );

    let valid_archive = root.path().join("valid-missing-target.sona-backup");
    export(&source_dir, &valid_archive, "0.8.0");
    let archive_before = fs::read(&valid_archive).unwrap();
    let prepared_before = prepared_workspace_names(&valid_archive);
    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "backup",
        "import",
        "--app-data-dir",
        path_arg(&missing_destination).as_str(),
        "--archive",
        path_arg(&valid_archive).as_str(),
        "--default-rule-set-name",
        "Imported Rules",
        "--confirm-replace",
    ])
    .unwrap_err();
    assert!(matches!(error, sona_cli::CliError::Io(_)));
    assert!(
        error
            .to_string()
            .contains("Application data directory does not exist")
    );
    assert!(!missing_destination.exists());
    assert_eq!(fs::read(&valid_archive).unwrap(), archive_before);
    assert_eq!(prepared_workspace_names(&valid_archive), prepared_before);
}

#[test]
fn backup_confirmation_and_names_validate_before_paths_or_side_effects() {
    let root = tempfile::tempdir().unwrap();
    let missing_app_data = root.path().join("missing-app-data");
    let invalid_archive = root.path().join("unconfirmed-invalid.sona-backup");
    fs::write(&invalid_archive, b"must not be opened").unwrap();
    let before_root = directory_entries(root.path());
    let before_prepared = prepared_workspace_names(&invalid_archive);

    let confirmation = sona_cli::run_cli_from_args([
        "sona-cli",
        "backup",
        "import",
        "--app-data-dir",
        path_arg(&missing_app_data).as_str(),
        "--archive",
        path_arg(&invalid_archive).as_str(),
        "--default-rule-set-name",
        "Imported Rules",
    ])
    .unwrap_err();
    assert!(matches!(confirmation, sona_cli::CliError::Validation(_)));
    assert_eq!(
        confirmation.to_string(),
        "Backup replacement requires explicit confirmation."
    );
    assert!(!missing_app_data.exists());
    assert_eq!(directory_entries(root.path()), before_root);
    assert_eq!(prepared_workspace_names(&invalid_archive), before_prepared);

    let empty_name = sona_cli::run_cli_from_args([
        "sona-cli",
        "backup",
        "import",
        "--app-data-dir",
        path_arg(&missing_app_data).as_str(),
        "--archive",
        path_arg(&invalid_archive).as_str(),
        "--default-rule-set-name",
        "  ",
        "--confirm-replace",
    ])
    .unwrap_err();
    assert!(matches!(empty_name, sona_cli::CliError::Validation(_)));
    assert!(empty_name.to_string().contains("default_rule_set_name"));
    assert!(!missing_app_data.exists());
}

#[test]
fn backup_export_validates_transport_before_database_or_final_archive() {
    let root = tempfile::tempdir().unwrap();
    let missing_app_data = root.path().join("missing-app-data");
    let output = root.path().join("nested").join("backup.sona-backup");

    let empty_version = sona_cli::run_cli_from_args([
        "sona-cli",
        "backup",
        "export",
        "--app-data-dir",
        path_arg(&missing_app_data).as_str(),
        "--output",
        path_arg(&output).as_str(),
        "--app-version",
        "  ",
    ])
    .unwrap_err();
    assert!(matches!(empty_version, sona_cli::CliError::Validation(_)));
    assert!(empty_version.to_string().contains("app_version"));
    assert!(!missing_app_data.exists());
    assert!(!output.parent().unwrap().exists());

    let missing_data = sona_cli::run_cli_from_args([
        "sona-cli",
        "backup",
        "export",
        "--app-data-dir",
        path_arg(&missing_app_data).as_str(),
        "--output",
        path_arg(&output).as_str(),
        "--app-version",
        "0.8.0",
    ])
    .unwrap_err();
    assert!(matches!(missing_data, sona_cli::CliError::Validation(_)));
    assert!(!missing_app_data.exists());
    assert!(!output.parent().unwrap().exists());

    let output_directory = root.path().join("archive-directory");
    fs::create_dir(&output_directory).unwrap();
    let invalid_output = sona_cli::run_cli_from_args([
        "sona-cli",
        "backup",
        "export",
        "--app-data-dir",
        path_arg(&missing_app_data).as_str(),
        "--output",
        path_arg(&output_directory).as_str(),
        "--app-version",
        "0.8.0",
    ])
    .unwrap_err();
    assert!(matches!(invalid_output, sona_cli::CliError::Validation(_)));
    assert!(!missing_app_data.exists());
}

#[test]
fn backup_parser_exposes_only_the_exact_commands_and_boolean_confirmation() {
    let import_with_value = sona_cli::run_cli_from_args([
        "sona-cli",
        "backup",
        "import",
        "--app-data-dir",
        "data",
        "--archive",
        "backup.sona-backup",
        "--default-rule-set-name",
        "Rules",
        "--confirm-replace=false",
    ])
    .unwrap_err();
    assert!(matches!(import_with_value, sona_cli::CliError::Usage(_)));

    let prepared_command = sona_cli::run_cli_from_args([
        "sona-cli",
        "backup",
        "prepare-import",
        "--archive",
        "backup.sona-backup",
    ])
    .unwrap_err();
    assert!(matches!(prepared_command, sona_cli::CliError::Usage(_)));

    let inspect_with_app_data = sona_cli::run_cli_from_args([
        "sona-cli",
        "backup",
        "inspect",
        "--archive",
        "backup.sona-backup",
        "--app-data-dir",
        "data",
    ])
    .unwrap_err();
    assert!(matches!(
        inspect_with_app_data,
        sona_cli::CliError::Usage(_)
    ));

    let help = sona_cli::run_cli_from_args(["sona-cli", "backup", "--help"]).unwrap_err();
    let help = help.to_string();
    for command in ["export", "inspect", "import"] {
        assert!(help.contains(command));
    }
    assert!(help.contains(
        "sona-cli backup export --app-data-dir ./sona-data --output ./sona-backup.sona-backup --app-version 0.8.0"
    ));
    assert!(help.contains("sona-cli backup inspect --archive ./sona-backup.sona-backup"));
    assert!(help.contains(
        "sona-cli backup import --app-data-dir ./sona-data --archive ./sona-backup.sona-backup --default-rule-set-name \"Default Rules\" --confirm-replace"
    ));
    assert!(help.contains("atomically replaces"));
    for scope in ["config", "workspace", "history", "automation", "analytics"] {
        assert!(help.contains(scope));
    }
    assert!(help.contains("never opens an interactive prompt"));
}

#[test]
fn backup_relative_paths_are_supported() {
    let current_dir = std::env::current_dir().unwrap();
    let root = tempfile::tempdir_in(&current_dir).unwrap();
    let source = root.path().join("source");
    let destination = root.path().join("destination");
    let archive = root.path().join("relative.sona-backup");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&destination).unwrap();
    seed_five_scopes(&source);
    let relative = |path: &Path| path.strip_prefix(&current_dir).unwrap().to_path_buf();

    let manifest: BackupManifest =
        canonical_output(&export(&relative(&source), &relative(&archive), "0.8.0"));
    let preview: PreparedBackupImport = canonical_output(&inspect(&relative(&archive)));
    let result: BackupApplyResult = canonical_output(&import(
        &relative(&destination),
        &relative(&archive),
        "Imported Rules",
    ));

    assert_eq!(preview.manifest, manifest);
    assert_eq!(result.manifest, manifest);
    assert!(destination.join("sona.db").is_file());
}

#[test]
fn backup_rejects_output_without_a_file_name() {
    let root = tempfile::tempdir().unwrap();
    let app_data_dir = root.path().join("source");
    fs::create_dir_all(&app_data_dir).unwrap();
    seed_five_scopes(&app_data_dir);
    let output = PathBuf::from(format!(
        "{}{}",
        path_arg(root.path()),
        std::path::MAIN_SEPARATOR
    ));

    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "backup",
        "export",
        "--app-data-dir",
        path_arg(&app_data_dir).as_str(),
        "--output",
        path_arg(&output).as_str(),
        "--app-version",
        "0.8.0",
    ])
    .unwrap_err();

    assert!(matches!(error, sona_cli::CliError::Validation(_)));
}
