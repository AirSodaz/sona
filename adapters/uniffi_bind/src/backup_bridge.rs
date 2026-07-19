use std::path::{Path, PathBuf};

use serde::Serialize;
use sona_archive::FsBackupAdapter;
use sona_core::backup::{
    BackupApplyResult, BackupDataset, BackupError, BackupExportRequest, BackupImportRequest,
    BackupInspectRequest, BackupRestoreDataset, BackupStateRepository,
};
use sona_runtime_fs::SystemClock;
use sona_sqlite::{SqliteBackupStateRepository, validate_backup_restore_dataset};

use crate::application_context::application_context;
use crate::{SonaCoreBindingError, SonaCoreBindingResult};

pub(crate) async fn export_backup_archive_json(
    app_data_dir: String,
    archive_path: String,
    app_version: String,
) -> SonaCoreBindingResult<String> {
    require_non_empty(&app_data_dir, "export app_data_dir")?;
    require_non_empty(&archive_path, "export archive_path")?;
    require_non_empty(&app_version, "export app_version")?;

    tokio::task::spawn_blocking(move || {
        let app_data_dir =
            std::path::absolute(PathBuf::from(app_data_dir)).map_err(backup_error)?;
        let archive_path =
            std::path::absolute(PathBuf::from(archive_path)).map_err(backup_error)?;
        let archive_path = utf8_path(&archive_path, "Backup export archive")?;
        let adapter = FsBackupAdapter::new(
            SharedContextBackupStateRepository::new(app_data_dir),
            SystemClock,
        );
        adapter
            .export_archive(BackupExportRequest {
                archive_path,
                app_version,
            })
            .map_err(backup_error)
            .and_then(canonical_json)
    })
    .await
    .map_err(backup_error)?
}

pub(crate) async fn inspect_backup_archive_json(
    archive_path: String,
) -> SonaCoreBindingResult<String> {
    require_non_empty(&archive_path, "inspect archive_path")?;

    tokio::task::spawn_blocking(move || {
        let archive_path =
            std::path::absolute(PathBuf::from(archive_path)).map_err(backup_error)?;
        let archive_path = utf8_path(&archive_path, "Backup inspect archive")?;
        let adapter = FsBackupAdapter::new(
            SharedContextBackupStateRepository::new(PathBuf::new()),
            SystemClock,
        );
        adapter
            .inspect_archive(BackupInspectRequest { archive_path })
            .map_err(backup_error)
            .and_then(canonical_json)
    })
    .await
    .map_err(backup_error)?
}

pub(crate) async fn import_backup_archive_json(
    app_data_dir: String,
    archive_path: String,
    default_rule_set_name: String,
    confirm_replace: bool,
) -> SonaCoreBindingResult<String> {
    require_non_empty(&app_data_dir, "import app_data_dir")?;
    require_non_empty(&archive_path, "import archive_path")?;
    require_non_empty(&default_rule_set_name, "import default_rule_set_name")?;

    tokio::task::spawn_blocking(move || {
        let app_data_dir =
            std::path::absolute(PathBuf::from(app_data_dir)).map_err(backup_error)?;
        let archive_path =
            std::path::absolute(PathBuf::from(archive_path)).map_err(backup_error)?;
        let archive_path = utf8_path(&archive_path, "Backup import archive")?;
        let adapter = FsBackupAdapter::new(
            SharedContextBackupStateRepository::new(app_data_dir),
            SystemClock,
        );
        adapter
            .import_archive(BackupImportRequest {
                archive_path,
                default_rule_set_name,
                confirm_replace,
            })
            .map_err(backup_error)
            .and_then(canonical_json)
    })
    .await
    .map_err(backup_error)?
}

fn require_non_empty(value: &str, field: &str) -> SonaCoreBindingResult<()> {
    if value.trim().is_empty() {
        Err(backup_error(format!("Backup {field} is required.")))
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct SharedContextBackupStateRepository {
    app_data_dir: PathBuf,
}

impl SharedContextBackupStateRepository {
    fn new(app_data_dir: PathBuf) -> Self {
        Self { app_data_dir }
    }

    fn repository(&self) -> Result<SqliteBackupStateRepository, BackupError> {
        if !self.app_data_dir.is_dir() {
            return Err(BackupError::State(format!(
                "Application data directory does not exist or is not a directory: {}",
                self.app_data_dir.display()
            )));
        }
        application_context(&self.app_data_dir)
            .map(|context| context.sqlite().backup_state_repository())
            .map_err(|error| BackupError::State(error.to_string()))
    }
}

impl BackupStateRepository for SharedContextBackupStateRepository {
    fn snapshot(&self) -> Result<BackupDataset, BackupError> {
        self.repository()?.snapshot()
    }

    fn replace_all(&self, dataset: BackupRestoreDataset) -> Result<BackupApplyResult, BackupError> {
        validate_backup_restore_dataset(&dataset)?;
        self.repository()?.replace_all(dataset)
    }
}

fn utf8_path(path: &Path, label: &str) -> SonaCoreBindingResult<String> {
    path.to_str().map(ToOwned::to_owned).ok_or_else(|| {
        backup_error(format!(
            "{label} path is not valid UTF-8: {}",
            path.display()
        ))
    })
}

fn canonical_json(value: impl Serialize) -> SonaCoreBindingResult<String> {
    let canonical = serde_json::to_value(value).map_err(backup_error)?;
    serde_json::to_string(&canonical).map_err(backup_error)
}

fn backup_error(reason: impl ToString) -> SonaCoreBindingError {
    SonaCoreBindingError::Backup {
        reason: reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
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
    use sona_core::history::mutation_repository::HistoryMutationRepository;
    use sona_core::history::{
        HistorySaveRecordingRequest, HistorySummaryPayload, TranscriptSummaryRecordPayload,
    };
    use sona_core::history_store::HistoryStore;
    use sona_core::tag::{TagDefaults, TagRecord, TagStore};
    use sona_runtime_fs::{SystemClock, UuidGenerator};
    use sona_sqlite::{
        Database, SqliteAutomationRepository, SqliteBackupStateRepository, SqliteConfigStore,
        SqliteHistoryStore, SqliteTagRepository, llm_usage,
    };

    use super::{
        canonical_json, export_backup_archive_json, import_backup_archive_json,
        inspect_backup_archive_json,
    };
    use crate::SonaCoreBindingError;

    fn path_arg(path: &Path) -> String {
        path.to_string_lossy().into_owned()
    }

    fn canonical_output<T>(output: &str) -> T
    where
        T: DeserializeOwned + Serialize,
    {
        assert!(!output.contains('\n'));
        let parsed = serde_json::from_str::<T>(output).unwrap();
        assert_eq!(
            output,
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

    #[test]
    fn canonical_json_preserves_spaces_inside_string_values() {
        let output = canonical_json(json!({"label": "two  spaces"})).unwrap();

        assert_eq!(output, r#"{"label":"two  spaces"}"#);
        let parsed: Value = canonical_output(&output);
        assert_eq!(parsed["label"], "two  spaces");
    }

    fn tag() -> TagRecord {
        TagRecord {
            id: "backup-project".to_string(),
            name: "Backup Project".to_string(),
            description: "Roundtrip workspace".to_string(),
            icon: "folder".to_string(),
            color: "#2563EB".to_string(),
            sort_order: 0,
            created_at: 100,
            updated_at: 200,
            defaults: TagDefaults {
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
                save_history: true,
                tag_ids: vec!["backup-project".to_string()],
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
        SqliteTagRepository::new(Arc::clone(&database))
            .replace_tags(vec![tag()])
            .unwrap();

        let history_store = SqliteHistoryStore::with_environment(
            app_data_dir.to_path_buf(),
            Arc::clone(&database),
            Arc::new(SystemClock),
            Arc::new(UuidGenerator),
        );
        history_store.ensure_ready().unwrap();
        let history_item = history_store
            .save_recording(HistorySaveRecordingRequest {
                segments: serde_json::from_value(json!([{
                    "id": "backup-segment",
                    "text": "Five scope backup",
                    "start": 0.0,
                    "end": 1.0,
                    "isFinal": true
                }]))
                .unwrap(),
                duration: 1.0,
                tag_ids: vec!["backup-project".to_string()],
                audio_bytes: Some(vec![1, 2, 3]),
                native_audio_path: None,
                audio_extension: None,
            })
            .unwrap();
        history_store
            .save_summary(
                &history_item.id,
                HistorySummaryPayload {
                    active_template_id: "general".to_string(),
                    record: Some(TranscriptSummaryRecordPayload {
                        template_id: "general".to_string(),
                        content: "Backup summary".to_string(),
                        generated_at: "2026-07-14T00:00:00.000Z".to_string(),
                        source_fingerprint: "backup-source".to_string(),
                    }),
                },
            )
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

    fn backup_reason(error: SonaCoreBindingError) -> String {
        let display = error.to_string();
        match error {
            SonaCoreBindingError::Backup { reason } => {
                assert_eq!(display, reason);
                reason
            }
            other => panic!("expected Backup error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn bridge_roundtrips_five_scopes_with_canonical_json_and_config_migration() {
        let root = tempfile::tempdir().unwrap();
        let source_dir = root.path().join("source");
        let destination_dir = root.path().join("destination");
        let archive = root.path().join("roundtrip.sona-backup");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(&destination_dir).unwrap();
        let source = seed_five_scopes(&source_dir);

        let manifest_json = export_backup_archive_json(
            path_arg(&source_dir),
            path_arg(&archive),
            "0.8.0".to_string(),
        )
        .await
        .unwrap();
        let manifest: BackupManifest = canonical_output(&manifest_json);
        assert_eq!(manifest.app_version, "0.8.0");
        assert_eq!(manifest.counts.tags, 1);
        assert_eq!(manifest.counts.history_items, 1);
        assert_eq!(manifest.counts.automation_rules, 1);
        assert_eq!(manifest.counts.automation_processed_entries, 1);
        assert_eq!(manifest.counts.analytics_files, 1);
        assert!(manifest.scopes.config);
        assert!(manifest.scopes.workspace);
        assert!(manifest.scopes.history);
        assert!(manifest.scopes.automation);
        assert!(manifest.scopes.analytics);

        let prepared_before = prepared_workspace_names(&archive);
        let preview_json = inspect_backup_archive_json(path_arg(&archive))
            .await
            .unwrap();
        let preview: PreparedBackupImport = canonical_output(&preview_json);
        assert_eq!(preview.manifest, manifest);
        assert_eq!(preview.tags.len(), 1);
        assert_eq!(preview.automation_rules.len(), 1);
        assert_eq!(preview.automation_processed_entries.len(), 1);
        assert_eq!(prepared_workspace_names(&archive), prepared_before);
        assert!(directory_entries(&destination_dir).is_empty());

        let result_json = import_backup_archive_json(
            path_arg(&destination_dir),
            path_arg(&archive),
            "Imported Rules".to_string(),
            true,
        )
        .await
        .unwrap();
        let result: BackupApplyResult = canonical_output(&result_json);
        assert_eq!(result.manifest, manifest);
        assert_eq!(prepared_workspace_names(&archive), prepared_before);

        let restored = snapshot(&destination_dir);
        assert_eq!(restored.tags, source.tags);
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

    #[tokio::test]
    async fn bridge_confirmation_and_transport_validation_precede_io() {
        let root = tempfile::tempdir().unwrap();
        let missing_app_data = root.path().join("missing-app-data");
        let invalid_archive = root.path().join("unconfirmed-invalid.sona-backup");
        let output = root.path().join("nested").join("backup.sona-backup");
        fs::write(&invalid_archive, b"must not be opened").unwrap();
        let root_before = directory_entries(root.path());
        let prepared_before = prepared_workspace_names(&invalid_archive);

        let confirmation = import_backup_archive_json(
            path_arg(&missing_app_data),
            path_arg(&invalid_archive),
            "Imported Rules".to_string(),
            false,
        )
        .await
        .unwrap_err();
        assert_eq!(
            backup_reason(confirmation),
            "Backup replacement requires explicit confirmation."
        );
        assert!(!missing_app_data.exists());
        assert_eq!(directory_entries(root.path()), root_before);
        assert_eq!(prepared_workspace_names(&invalid_archive), prepared_before);

        let empty_version = export_backup_archive_json(
            path_arg(&missing_app_data),
            path_arg(&output),
            "  ".to_string(),
        )
        .await
        .unwrap_err();
        assert!(backup_reason(empty_version).contains("export app_version"));
        assert!(!missing_app_data.exists());
        assert!(!output.parent().unwrap().exists());

        let empty_archive = inspect_backup_archive_json("  ".to_string())
            .await
            .unwrap_err();
        assert!(backup_reason(empty_archive).contains("inspect archive_path"));

        let empty_default_name = import_backup_archive_json(
            path_arg(&missing_app_data),
            path_arg(&invalid_archive),
            "  ".to_string(),
            true,
        )
        .await
        .unwrap_err();
        assert!(backup_reason(empty_default_name).contains("import default_rule_set_name"));
        assert!(!missing_app_data.exists());
        assert_eq!(prepared_workspace_names(&invalid_archive), prepared_before);
    }

    #[tokio::test]
    async fn bridge_missing_and_invalid_archives_leave_state_and_workspaces_untouched() {
        let root = tempfile::tempdir().unwrap();
        let app_data_dir = root.path().join("empty-destination");
        let missing_archive = root.path().join("missing.sona-backup");
        let invalid_archive = root.path().join("invalid.sona-backup");
        fs::create_dir_all(&app_data_dir).unwrap();
        fs::write(&invalid_archive, b"not an archive").unwrap();

        let missing = import_backup_archive_json(
            path_arg(&app_data_dir),
            path_arg(&missing_archive),
            "Imported Rules".to_string(),
            true,
        )
        .await
        .unwrap_err();
        assert!(backup_reason(missing).contains("Backup archive error:"));
        assert!(directory_entries(&app_data_dir).is_empty());

        let prepared_before = prepared_workspace_names(&invalid_archive);
        let invalid = import_backup_archive_json(
            path_arg(&app_data_dir),
            path_arg(&invalid_archive),
            "Imported Rules".to_string(),
            true,
        )
        .await
        .unwrap_err();
        assert!(backup_reason(invalid).contains("Backup archive error:"));
        assert!(directory_entries(&app_data_dir).is_empty());
        assert_eq!(prepared_workspace_names(&invalid_archive), prepared_before);

        let inspect_error = inspect_backup_archive_json(path_arg(&invalid_archive))
            .await
            .unwrap_err();
        assert!(backup_reason(inspect_error).contains("Backup archive error:"));
        assert!(directory_entries(&app_data_dir).is_empty());
        assert_eq!(prepared_workspace_names(&invalid_archive), prepared_before);
    }

    #[tokio::test]
    async fn bridge_requires_existing_app_data_and_cleans_prepared_state_errors() {
        let root = tempfile::tempdir().unwrap();
        let source_dir = root.path().join("source");
        let missing_export_dir = root.path().join("missing-export-data");
        let missing_import_dir = root.path().join("missing-import-data");
        let archive = root.path().join("valid.sona-backup");
        let unused_archive = root.path().join("unused.sona-backup");
        fs::create_dir_all(&source_dir).unwrap();
        seed_five_scopes(&source_dir);
        export_backup_archive_json(
            path_arg(&source_dir),
            path_arg(&archive),
            "0.8.0".to_string(),
        )
        .await
        .unwrap();

        let export_error = export_backup_archive_json(
            path_arg(&missing_export_dir),
            path_arg(&unused_archive),
            "0.8.0".to_string(),
        )
        .await
        .unwrap_err();
        assert!(backup_reason(export_error).contains("Application data directory does not exist"));
        assert!(!missing_export_dir.exists());
        assert!(!unused_archive.exists());

        let prepared_before = prepared_workspace_names(&archive);
        let import_error = import_backup_archive_json(
            path_arg(&missing_import_dir),
            path_arg(&archive),
            "Imported Rules".to_string(),
            true,
        )
        .await
        .unwrap_err();
        assert!(backup_reason(import_error).contains("Application data directory does not exist"));
        assert!(!missing_import_dir.exists());
        assert_eq!(prepared_workspace_names(&archive), prepared_before);
    }

    #[tokio::test]
    async fn bridge_semantic_validation_precedes_target_directory_and_database_access() {
        let root = tempfile::tempdir().unwrap();
        let source_dir = root.path().join("source");
        let empty_target = root.path().join("empty-target");
        let missing_target = root.path().join("missing-target");
        let archive = root.path().join("invalid-semantic.sona-backup");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(&empty_target).unwrap();
        seed_five_scopes(&source_dir);
        export_backup_archive_json(
            path_arg(&source_dir),
            path_arg(&archive),
            "0.8.0".to_string(),
        )
        .await
        .unwrap();
        rewrite_archive(&archive, |contents| {
            update_json(&contents.join("tags/index.json"), |tags| {
                let tags = tags.as_array_mut().unwrap();
                tags.push(tags[0].clone());
            });
            update_json(&contents.join("manifest.json"), |manifest| {
                manifest["counts"]["tags"] = json!(2);
            });
        });

        for target in [&empty_target, &missing_target] {
            let target_before = directory_entries(target);
            let prepared_before = prepared_workspace_names(&archive);
            let archive_before = fs::read(&archive).unwrap();
            let error = import_backup_archive_json(
                path_arg(target),
                path_arg(&archive),
                "Imported Rules".to_string(),
                true,
            )
            .await
            .unwrap_err();

            assert!(backup_reason(error).contains("Invalid backup:"));
            assert_eq!(directory_entries(target), target_before);
            assert_eq!(fs::read(&archive).unwrap(), archive_before);
            assert_eq!(prepared_workspace_names(&archive), prepared_before);
            for forbidden in [
                "sona.db",
                "sona.db-wal",
                "sona.db-shm",
                "sona-analytics.db",
                "sona-analytics.db-wal",
                "sona-analytics.db-shm",
                ".sona-history.lock",
            ] {
                assert!(!target.join(forbidden).exists());
            }
        }
        assert!(!missing_target.exists());
    }
}
