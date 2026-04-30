use chrono::{SecondsFormat, Utc};
use serde_json::Value;
use std::fs;
use std::path::Path;
use uuid::Uuid;

use super::fs_utils::{
    copy_directory_recursive, create_tar_bz2_archive, create_temp_directory,
    ensure_json_array_value, ensure_json_object_value, ensure_safe_file_name,
    extract_tar_bz2_archive, read_json_value, remove_path_if_exists, write_json_pretty_atomic,
};
use super::repository::{normalize_history_item_value, HistoryRepository};
use super::types::PreparedBackupImportSnapshot;
use super::{
    BackupManifest, BackupManifestCounts, BackupManifestScopes, ExportBackupArchiveRequest,
    HistoryItemStatus, PreparedBackupImport, ANALYTICS_DIR_NAME, ANALYTICS_USAGE_FILE_NAME,
    AUTOMATION_DIR_NAME, AUTOMATION_PROCESSED_FILE_NAME, AUTOMATION_RULES_FILE_NAME,
    BACKUP_HISTORY_MODE, BACKUP_SCHEMA_VERSION, CONFIG_DIR_NAME, CONFIG_FILE_NAME,
    HISTORY_DIR_NAME, PROJECTS_DIR_NAME, PROJECTS_INDEX_FILE_NAME, SUMMARY_FILE_SUFFIX,
};

pub(super) fn build_backup_manifest(
    app_version: String,
    project_count: usize,
    history_item_count: usize,
    transcript_file_count: usize,
    summary_file_count: usize,
    automation_rule_count: usize,
    automation_processed_entry_count: usize,
) -> BackupManifest {
    BackupManifest {
        schema_version: BACKUP_SCHEMA_VERSION,
        created_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        app_version,
        history_mode: BACKUP_HISTORY_MODE.to_string(),
        scopes: BackupManifestScopes {
            config: true,
            workspace: true,
            history: true,
            automation: true,
            analytics: true,
        },
        counts: BackupManifestCounts {
            projects: project_count as u64,
            history_items: history_item_count as u64,
            transcript_files: transcript_file_count as u64,
            summary_files: summary_file_count as u64,
            automation_rules: automation_rule_count as u64,
            automation_processed_entries: automation_processed_entry_count as u64,
            analytics_files: 1,
        },
    }
}

fn validate_backup_manifest(value: &Value) -> Result<BackupManifest, String> {
    let manifest: BackupManifest =
        serde_json::from_value(value.clone()).map_err(|error| error.to_string())?;
    if manifest.schema_version != BACKUP_SCHEMA_VERSION {
        return Err(format!(
            "Unsupported backup schema version: {}",
            manifest.schema_version
        ));
    }
    if manifest.history_mode != BACKUP_HISTORY_MODE {
        return Err(format!(
            "Unsupported backup history mode: {}",
            manifest.history_mode
        ));
    }
    if !manifest.scopes.config
        || !manifest.scopes.workspace
        || !manifest.scopes.history
        || !manifest.scopes.automation
        || !manifest.scopes.analytics
    {
        return Err("Backup manifest is missing one or more required scopes.".to_string());
    }
    Ok(manifest)
}

pub(super) fn export_backup_archive_inner(
    app_local_data_dir: &Path,
    request: ExportBackupArchiveRequest,
) -> Result<BackupManifest, String> {
    let config = ensure_json_object_value(request.config, "Backup config")?;
    let analytics_json: Value =
        serde_json::from_str(&request.analytics_content).map_err(|error| error.to_string())?;
    ensure_json_object_value(analytics_json, "Backup analytics")?;

    let repository = HistoryRepository::new(app_local_data_dir.to_path_buf());
    let history = repository.history_snapshot_for_backup()?;
    let manifest = build_backup_manifest(
        request.app_version,
        request.projects.len(),
        history.items.len(),
        history.transcript_files.len(),
        history.summary_files.len(),
        request.automation_rules.len(),
        request.automation_processed_entries.len(),
    );

    let staging_dir = create_temp_directory("backup-export")?;
    let result = (|| -> Result<(), String> {
        let config_dir = staging_dir.join(CONFIG_DIR_NAME);
        let projects_dir = staging_dir.join(PROJECTS_DIR_NAME);
        let history_dir = staging_dir.join(HISTORY_DIR_NAME);
        let automation_dir = staging_dir.join(AUTOMATION_DIR_NAME);
        let analytics_dir = staging_dir.join(ANALYTICS_DIR_NAME);

        for dir in [
            &config_dir,
            &projects_dir,
            &history_dir,
            &automation_dir,
            &analytics_dir,
        ] {
            fs::create_dir_all(dir).map_err(|error| error.to_string())?;
        }

        write_json_pretty_atomic(&staging_dir.join("manifest.json"), &manifest)?;
        write_json_pretty_atomic(&config_dir.join(CONFIG_FILE_NAME), &config)?;
        write_json_pretty_atomic(
            &projects_dir.join(PROJECTS_INDEX_FILE_NAME),
            &request.projects,
        )?;
        write_json_pretty_atomic(&history_dir.join(PROJECTS_INDEX_FILE_NAME), &history.items)?;
        write_json_pretty_atomic(
            &automation_dir.join(AUTOMATION_RULES_FILE_NAME),
            &request.automation_rules,
        )?;
        write_json_pretty_atomic(
            &automation_dir.join(AUTOMATION_PROCESSED_FILE_NAME),
            &request.automation_processed_entries,
        )?;
        fs::write(
            analytics_dir.join(ANALYTICS_USAGE_FILE_NAME),
            request.analytics_content,
        )
        .map_err(|error| error.to_string())?;

        for (file_name, transcript) in history.transcript_files {
            write_json_pretty_atomic(&history_dir.join(file_name), &transcript)?;
        }
        for (history_id, summary) in history.summary_files {
            write_json_pretty_atomic(
                &history_dir.join(format!("{history_id}{SUMMARY_FILE_SUFFIX}")),
                &summary,
            )?;
        }

        create_tar_bz2_archive(&staging_dir, Path::new(&request.archive_path))?;
        Ok(())
    })();

    let cleanup_error = remove_path_if_exists(&staging_dir).err();
    match (result, cleanup_error) {
        (Ok(()), None) => Ok(manifest),
        (Ok(()), Some(error)) => Err(error),
        (Err(error), _) => Err(error),
    }
}

pub(super) fn prepare_backup_import_inner(
    archive_path: &Path,
) -> Result<(PreparedBackupImport, PreparedBackupImportSnapshot), String> {
    let extraction_dir = create_temp_directory("backup-import")?;
    let result = (|| -> Result<(PreparedBackupImport, PreparedBackupImportSnapshot), String> {
        extract_tar_bz2_archive(archive_path, &extraction_dir)?;

        let manifest =
            validate_backup_manifest(&read_json_value(&extraction_dir.join("manifest.json"))?)?;
        let config = ensure_json_object_value(
            read_json_value(&extraction_dir.join(CONFIG_DIR_NAME).join(CONFIG_FILE_NAME))?,
            "Backup config",
        )?;
        let projects = read_json_value(
            &extraction_dir
                .join(PROJECTS_DIR_NAME)
                .join(PROJECTS_INDEX_FILE_NAME),
        )?
        .as_array()
        .cloned()
        .ok_or_else(|| "Backup projects must be an array.".to_string())?;

        let history_items_value = read_json_value(
            &extraction_dir
                .join(HISTORY_DIR_NAME)
                .join(PROJECTS_INDEX_FILE_NAME),
        )?;
        let history_items = history_items_value
            .as_array()
            .ok_or_else(|| "Backup history index must be an array.".to_string())?
            .iter()
            .map(normalize_history_item_value)
            .collect::<Vec<_>>();

        let mut transcript_count = 0usize;
        let mut summary_count = 0usize;
        for item in &history_items {
            if item.status == HistoryItemStatus::Draft {
                return Err(format!(
                    "Backup history item \"{}\" is a draft and cannot be imported.",
                    item.id
                ));
            }

            let transcript_file_name = ensure_safe_file_name(
                &item.transcript_path,
                &format!("History transcript path for {}", item.id),
            )?;
            let transcript_path = extraction_dir
                .join(HISTORY_DIR_NAME)
                .join(transcript_file_name);
            ensure_json_array_value(
                read_json_value(&transcript_path)?,
                &format!("Transcript for history item {}", item.id),
            )?;
            transcript_count += 1;

            let summary_path = extraction_dir
                .join(HISTORY_DIR_NAME)
                .join(format!("{}{}", item.id, SUMMARY_FILE_SUFFIX));
            if summary_path.exists() {
                ensure_json_object_value(
                    read_json_value(&summary_path)?,
                    &format!("Summary for history item {}", item.id),
                )?;
                summary_count += 1;
            }
        }

        let automation_rules = read_json_value(
            &extraction_dir
                .join(AUTOMATION_DIR_NAME)
                .join(AUTOMATION_RULES_FILE_NAME),
        )?
        .as_array()
        .cloned()
        .ok_or_else(|| "Backup automation rules must be an array.".to_string())?;
        let automation_processed_entries = read_json_value(
            &extraction_dir
                .join(AUTOMATION_DIR_NAME)
                .join(AUTOMATION_PROCESSED_FILE_NAME),
        )?
        .as_array()
        .cloned()
        .ok_or_else(|| "Backup automation processed entries must be an array.".to_string())?;

        let analytics_content = fs::read_to_string(
            extraction_dir
                .join(ANALYTICS_DIR_NAME)
                .join(ANALYTICS_USAGE_FILE_NAME),
        )
        .map_err(|error| error.to_string())?;
        let analytics_json: Value =
            serde_json::from_str(&analytics_content).map_err(|error| error.to_string())?;
        ensure_json_object_value(analytics_json, "Backup analytics")?;

        if manifest.counts.projects != projects.len() as u64 {
            return Err("Backup project count does not match the manifest.".to_string());
        }
        if manifest.counts.history_items != history_items.len() as u64 {
            return Err("Backup history count does not match the manifest.".to_string());
        }
        if manifest.counts.transcript_files != transcript_count as u64 {
            return Err("Backup transcript count does not match the manifest.".to_string());
        }
        if manifest.counts.summary_files != summary_count as u64 {
            return Err("Backup summary count does not match the manifest.".to_string());
        }
        if manifest.counts.automation_rules != automation_rules.len() as u64 {
            return Err("Backup automation-rule count does not match the manifest.".to_string());
        }
        if manifest.counts.automation_processed_entries != automation_processed_entries.len() as u64
        {
            return Err("Backup processed-entry count does not match the manifest.".to_string());
        }
        if manifest.counts.analytics_files != 1 {
            return Err("Backup analytics count does not match the manifest.".to_string());
        }

        let import_id = Uuid::new_v4().to_string();
        let response = PreparedBackupImport {
            import_id: import_id.clone(),
            archive_path: archive_path.to_string_lossy().into_owned(),
            manifest,
            config,
            projects,
            automation_rules,
            automation_processed_entries,
            analytics_content,
        };
        let snapshot = PreparedBackupImportSnapshot {
            archive_path: response.archive_path.clone(),
            extraction_dir: extraction_dir.clone(),
        };

        Ok((response, snapshot))
    })();

    if result.is_err() {
        let _ = remove_path_if_exists(&extraction_dir);
    }

    result
}

pub(super) fn apply_prepared_history_import_inner(
    app_local_data_dir: &Path,
    import_id: &str,
    extraction_dir: &Path,
) -> Result<(), String> {
    let extracted_history_dir = extraction_dir.join(HISTORY_DIR_NAME);
    if !extracted_history_dir.is_dir() {
        return Err("Prepared backup import is missing the history directory.".to_string());
    }

    let repository = HistoryRepository::new(app_local_data_dir.to_path_buf());
    repository.ensure_ready()?;

    let target_history_dir = repository.history_dir();
    let staged_history_dir = app_local_data_dir.join(format!("history.importing-{import_id}"));
    let previous_history_dir = app_local_data_dir.join(format!("history.previous-{import_id}"));

    remove_path_if_exists(&staged_history_dir)?;
    remove_path_if_exists(&previous_history_dir)?;

    copy_directory_recursive(&extracted_history_dir, &staged_history_dir)?;

    let had_existing = target_history_dir.exists();
    if had_existing {
        fs::rename(&target_history_dir, &previous_history_dir)
            .map_err(|error| error.to_string())?;
    }

    match fs::rename(&staged_history_dir, &target_history_dir) {
        Ok(()) => {
            if had_existing {
                remove_path_if_exists(&previous_history_dir)?;
            }
            Ok(())
        }
        Err(error) => {
            if had_existing && !target_history_dir.exists() {
                let _ = fs::rename(&previous_history_dir, &target_history_dir);
            }
            let _ = remove_path_if_exists(&staged_history_dir);
            Err(error.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history_repository::fs_utils::{
        create_tar_bz2_archive, extract_tar_bz2_archive, read_json_value, remove_path_if_exists,
        write_json_pretty_atomic,
    };
    use crate::history_repository::repository::HistoryRepository;
    use crate::history_repository::state::PreparedBackupImportState;
    use crate::history_repository::test_support::{
        create_valid_backup_archive, sample_history_item,
    };
    use crate::history_repository::types::HistoryItemStatus;
    use crate::history_repository::{
        ANALYTICS_DIR_NAME, ANALYTICS_USAGE_FILE_NAME, AUTOMATION_DIR_NAME,
        AUTOMATION_PROCESSED_FILE_NAME, AUTOMATION_RULES_FILE_NAME, CONFIG_DIR_NAME,
        CONFIG_FILE_NAME, HISTORY_DIR_NAME, PROJECTS_DIR_NAME, PROJECTS_INDEX_FILE_NAME,
    };
    use serde_json::{json, Value};
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn export_backup_archive_skips_draft_items_and_preserves_manifest() {
        let root = tempdir().unwrap();
        let repository = HistoryRepository::new(root.path().to_path_buf());
        repository.ensure_ready().unwrap();

        let keep_item = sample_history_item("keep", HistoryItemStatus::Complete);
        let draft_item = sample_history_item("draft", HistoryItemStatus::Draft);
        repository
            .write_index(&vec![keep_item.clone(), draft_item])
            .unwrap();
        write_json_pretty_atomic(
            &repository
                .transcript_path(&keep_item.transcript_path)
                .unwrap(),
            &json!([{ "id": "seg-1" }]),
        )
        .unwrap();
        write_json_pretty_atomic(
            &repository.summary_path(&keep_item.id).unwrap(),
            &json!({ "activeTemplateId": "general" }),
        )
        .unwrap();

        let archive_dir = tempdir().unwrap();
        let archive_path = archive_dir.path().join("backup.tar.bz2");
        let manifest = export_backup_archive_inner(
            root.path(),
            ExportBackupArchiveRequest {
                archive_path: archive_path.to_string_lossy().into_owned(),
                app_version: "0.6.4".to_string(),
                config: json!({ "theme": "auto" }),
                projects: vec![json!({ "id": "project-1" })],
                automation_rules: vec![json!({ "id": "rule-1" })],
                automation_processed_entries: vec![json!({ "ruleId": "rule-1" })],
                analytics_content: r#"{"schemaVersion":1}"#.to_string(),
            },
        )
        .unwrap();

        assert_eq!(manifest.history_mode, "light");
        assert_eq!(manifest.counts.history_items, 1);
        assert_eq!(manifest.counts.transcript_files, 1);
        assert_eq!(manifest.counts.summary_files, 1);

        let extract_dir = tempdir().unwrap();
        extract_tar_bz2_archive(&archive_path, extract_dir.path()).unwrap();
        let exported_items = read_json_value(
            &extract_dir
                .path()
                .join(HISTORY_DIR_NAME)
                .join(PROJECTS_INDEX_FILE_NAME),
        )
        .unwrap();
        assert_eq!(exported_items.as_array().unwrap().len(), 1);
        assert_eq!(
            exported_items.as_array().unwrap()[0]["id"]
                .as_str()
                .unwrap(),
            "keep"
        );
    }

    #[test]
    fn prepare_backup_import_rejects_missing_transcript_before_mutation() {
        let archive_dir = tempdir().unwrap();
        let archive_path = archive_dir.path().join("invalid-backup.tar.bz2");
        let staging_dir = tempdir().unwrap();
        fs::create_dir_all(staging_dir.path().join(HISTORY_DIR_NAME)).unwrap();
        fs::create_dir_all(staging_dir.path().join(CONFIG_DIR_NAME)).unwrap();
        fs::create_dir_all(staging_dir.path().join(PROJECTS_DIR_NAME)).unwrap();
        fs::create_dir_all(staging_dir.path().join(AUTOMATION_DIR_NAME)).unwrap();
        fs::create_dir_all(staging_dir.path().join(ANALYTICS_DIR_NAME)).unwrap();
        write_json_pretty_atomic(
            &staging_dir.path().join("manifest.json"),
            &build_backup_manifest("0.6.4".to_string(), 0, 1, 1, 0, 0, 0),
        )
        .unwrap();
        write_json_pretty_atomic(
            &staging_dir
                .path()
                .join(CONFIG_DIR_NAME)
                .join(CONFIG_FILE_NAME),
            &json!({}),
        )
        .unwrap();
        write_json_pretty_atomic(
            &staging_dir
                .path()
                .join(PROJECTS_DIR_NAME)
                .join(PROJECTS_INDEX_FILE_NAME),
            &Vec::<Value>::new(),
        )
        .unwrap();
        write_json_pretty_atomic(
            &staging_dir
                .path()
                .join(HISTORY_DIR_NAME)
                .join(PROJECTS_INDEX_FILE_NAME),
            &vec![json!({
                "id": "history-1",
                "audioPath": "history-1.webm",
                "transcriptPath": "history-1.json",
                "title": "Broken",
                "projectId": null,
                "status": "complete"
            })],
        )
        .unwrap();
        write_json_pretty_atomic(
            &staging_dir
                .path()
                .join(AUTOMATION_DIR_NAME)
                .join(AUTOMATION_RULES_FILE_NAME),
            &Vec::<Value>::new(),
        )
        .unwrap();
        write_json_pretty_atomic(
            &staging_dir
                .path()
                .join(AUTOMATION_DIR_NAME)
                .join(AUTOMATION_PROCESSED_FILE_NAME),
            &Vec::<Value>::new(),
        )
        .unwrap();
        fs::write(
            staging_dir
                .path()
                .join(ANALYTICS_DIR_NAME)
                .join(ANALYTICS_USAGE_FILE_NAME),
            r#"{"schemaVersion":1}"#,
        )
        .unwrap();
        create_tar_bz2_archive(staging_dir.path(), &archive_path).unwrap();

        let err = prepare_backup_import_inner(&archive_path).unwrap_err();
        assert!(
            err.contains("os error 2")
                || err.contains("No such file")
                || err.contains("找不到指定的文件")
        );
    }

    #[test]
    fn apply_prepared_history_import_replaces_history_and_dispose_cleans_snapshot() {
        let app_data_dir = tempdir().unwrap();
        let history_dir = app_data_dir.path().join(HISTORY_DIR_NAME);
        fs::create_dir_all(&history_dir).unwrap();
        write_json_pretty_atomic(
            &history_dir.join(PROJECTS_INDEX_FILE_NAME),
            &vec![json!({ "id": "old-history" })],
        )
        .unwrap();

        let archive_dir = tempdir().unwrap();
        let archive_path = archive_dir.path().join("valid-backup.tar.bz2");
        create_valid_backup_archive(&archive_path);

        let (prepared, snapshot) = prepare_backup_import_inner(&archive_path).unwrap();
        let state = PreparedBackupImportState::default();
        state
            .insert(prepared.import_id.clone(), snapshot.clone())
            .unwrap();

        apply_prepared_history_import_inner(
            app_data_dir.path(),
            &prepared.import_id,
            &snapshot.extraction_dir,
        )
        .unwrap();

        let replaced_items = read_json_value(&history_dir.join(PROJECTS_INDEX_FILE_NAME)).unwrap();
        assert_eq!(
            replaced_items.as_array().unwrap()[0]["id"]
                .as_str()
                .unwrap(),
            "history-1"
        );

        let removed = state.remove(&prepared.import_id).unwrap().unwrap();
        remove_path_if_exists(&removed.extraction_dir).unwrap();
        assert!(!removed.extraction_dir.exists());
    }
}
