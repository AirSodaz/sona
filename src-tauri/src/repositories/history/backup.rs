use chrono::{SecondsFormat, Utc};
use serde_json::{Map, Number, Value};
use std::fs;
use std::path::Path;
use uuid::Uuid;

use crate::repositories::project::normalize_project_record_for_import;

use super::fs_utils::{
    create_tar_bz2_archive, create_temp_directory, ensure_json_array_value,
    ensure_json_object_value, ensure_safe_file_name, extract_tar_bz2_archive, read_json_value,
    remove_path_if_exists, write_json_pretty_atomic,
};
use super::repository::{HistoryRepository, normalize_history_item_value};
use super::sqlite_store::SqliteHistoryStore;
use super::types::PreparedBackupImportSnapshot;
use super::{
    ANALYTICS_DIR_NAME, ANALYTICS_USAGE_FILE_NAME, AUTOMATION_DIR_NAME,
    AUTOMATION_PROCESSED_FILE_NAME, AUTOMATION_RULES_FILE_NAME, BACKUP_HISTORY_MODE,
    BACKUP_SCHEMA_VERSION, BackupManifest, BackupManifestCounts, BackupManifestScopes,
    CONFIG_DIR_NAME, CONFIG_FILE_NAME, ExportBackupArchiveRequest, HISTORY_DIR_NAME,
    HISTORY_VERSIONS_DIR_NAME, HistoryDraftSource, HistoryItemKind, HistoryItemRecord,
    HistoryItemStatus, PROJECTS_DIR_NAME, PROJECTS_INDEX_FILE_NAME, PreparedBackupImport,
    SUMMARY_FILE_SUFFIX, TranscriptSnapshotMetadata, TranscriptSnapshotReason,
    TranscriptSnapshotRecord,
};
use crate::core::history_store::HistoryStore;

pub(crate) fn build_backup_manifest(
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

fn normalize_backup_projects(value: Value) -> Result<Vec<Value>, String> {
    let projects = value
        .as_array()
        .ok_or_else(|| "Backup projects must be an array.".to_string())?;

    projects
        .iter()
        .map(normalize_project_record_for_import)
        .collect()
}

fn normalize_automation_rules(value: Value) -> Result<Vec<Value>, String> {
    let rules = value
        .as_array()
        .ok_or_else(|| "Backup automation rules must be an array.".to_string())?;

    rules.iter().map(normalize_automation_rule).collect()
}

fn normalize_automation_rule(input: &Value) -> Result<Value, String> {
    let source = input
        .as_object()
        .ok_or_else(|| "Automation rule must be an object.".to_string())?;

    let stage_config = normalize_automation_stage_config(source.get("stageConfig"))?;
    let export_config = normalize_automation_export_config(source.get("exportConfig"))?;

    let mut normalized = Map::new();
    normalized.insert(
        "id".to_string(),
        Value::String(string_field(source, "id").unwrap_or_default()),
    );
    normalized.insert(
        "name".to_string(),
        Value::String(string_field(source, "name").unwrap_or_default()),
    );
    normalized.insert(
        "projectId".to_string(),
        Value::String(string_field(source, "projectId").unwrap_or_default()),
    );
    normalized.insert(
        "presetId".to_string(),
        Value::String(
            string_field(source, "presetId")
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "custom".to_string()),
        ),
    );
    normalized.insert(
        "watchDirectory".to_string(),
        Value::String(string_field(source, "watchDirectory").unwrap_or_default()),
    );
    normalized.insert(
        "recursive".to_string(),
        Value::Bool(js_truthy(source.get("recursive"))),
    );
    normalized.insert(
        "enabled".to_string(),
        Value::Bool(js_truthy(source.get("enabled"))),
    );
    normalized.insert("stageConfig".to_string(), stage_config);
    normalized.insert("exportConfig".to_string(), export_config);
    normalized.insert(
        "createdAt".to_string(),
        Value::Number(number_field_or_zero(source, "createdAt")),
    );
    normalized.insert(
        "updatedAt".to_string(),
        Value::Number(number_field_or_zero(source, "updatedAt")),
    );

    Ok(Value::Object(normalized))
}

fn normalize_automation_stage_config(value: Option<&Value>) -> Result<Value, String> {
    let source = object_or_empty(value, "Automation stage config")?;
    let mut normalized = Map::new();
    normalized.insert(
        "autoPolish".to_string(),
        Value::Bool(bool_field(source, "autoPolish")),
    );
    normalized.insert(
        "polishPresetId".to_string(),
        Value::String(
            string_field_optional(source, "polishPresetId")
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "general".to_string()),
        ),
    );
    normalized.insert(
        "autoTranslate".to_string(),
        Value::Bool(bool_field(source, "autoTranslate")),
    );
    normalized.insert(
        "translationLanguage".to_string(),
        Value::String(
            string_field_optional(source, "translationLanguage")
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "en".to_string()),
        ),
    );
    normalized.insert(
        "exportEnabled".to_string(),
        Value::Bool(bool_field(source, "exportEnabled")),
    );

    Ok(Value::Object(normalized))
}

fn normalize_automation_export_config(value: Option<&Value>) -> Result<Value, String> {
    let source = object_or_empty(value, "Automation export config")?;
    let mut normalized = Map::new();
    normalized.insert(
        "directory".to_string(),
        Value::String(string_field_optional(source, "directory").unwrap_or_default()),
    );
    normalized.insert(
        "format".to_string(),
        Value::String(
            string_field_optional(source, "format")
                .filter(|value| matches!(value.as_str(), "srt" | "json" | "txt" | "vtt"))
                .unwrap_or_else(|| "txt".to_string()),
        ),
    );
    normalized.insert(
        "mode".to_string(),
        Value::String(
            string_field_optional(source, "mode")
                .filter(|value| matches!(value.as_str(), "original" | "translation" | "bilingual"))
                .unwrap_or_else(|| "original".to_string()),
        ),
    );
    normalized.insert(
        "prefix".to_string(),
        Value::String(string_field_optional(source, "prefix").unwrap_or_default()),
    );

    Ok(Value::Object(normalized))
}

fn normalize_automation_processed_entries(value: Value) -> Result<Vec<Value>, String> {
    let entries = value
        .as_array()
        .ok_or_else(|| "Backup automation processed entries must be an array.".to_string())?;

    entries
        .iter()
        .map(normalize_automation_processed_entry)
        .collect()
}

fn normalize_automation_processed_entry(input: &Value) -> Result<Value, String> {
    let source = input
        .as_object()
        .ok_or_else(|| "Automation processed entry must be an object.".to_string())?;

    let mut normalized = Map::new();
    normalized.insert(
        "ruleId".to_string(),
        Value::String(string_field(source, "ruleId").unwrap_or_default()),
    );
    normalized.insert(
        "filePath".to_string(),
        Value::String(string_field(source, "filePath").unwrap_or_default()),
    );
    normalized.insert(
        "sourceFingerprint".to_string(),
        Value::String(string_field(source, "sourceFingerprint").unwrap_or_default()),
    );
    normalized.insert(
        "size".to_string(),
        Value::Number(number_field_or_zero(source, "size")),
    );
    normalized.insert(
        "mtimeMs".to_string(),
        Value::Number(number_field_or_zero(source, "mtimeMs")),
    );
    normalized.insert(
        "status".to_string(),
        Value::String(match source.get("status").and_then(Value::as_str) {
            Some("error") => "error".to_string(),
            Some("discarded") => "discarded".to_string(),
            _ => "complete".to_string(),
        }),
    );
    normalized.insert(
        "processedAt".to_string(),
        Value::Number(number_field_or_zero(source, "processedAt")),
    );

    insert_optional_string(&mut normalized, source, "historyId");
    insert_optional_string(&mut normalized, source, "exportPath");
    insert_optional_string(&mut normalized, source, "errorMessage");

    Ok(Value::Object(normalized))
}

fn object_or_empty<'a>(
    value: Option<&'a Value>,
    label: &str,
) -> Result<Option<&'a Map<String, Value>>, String> {
    match value {
        Some(Value::Object(source)) => Ok(Some(source)),
        Some(Value::Null) | None => Ok(None),
        _ => Err(format!("{label} must be an object.")),
    }
}

fn string_field(source: &Map<String, Value>, key: &str) -> Option<String> {
    source
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn string_field_optional(source: Option<&Map<String, Value>>, key: &str) -> Option<String> {
    source
        .and_then(|object| object.get(key))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn bool_field(source: Option<&Map<String, Value>>, key: &str) -> bool {
    source
        .and_then(|object| object.get(key))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn number_field_or_zero(source: &Map<String, Value>, key: &str) -> Number {
    source
        .get(key)
        .and_then(Value::as_number)
        .cloned()
        .unwrap_or_else(|| 0.into())
}

fn insert_optional_string(target: &mut Map<String, Value>, source: &Map<String, Value>, key: &str) {
    if let Some(value) = string_field(source, key) {
        target.insert(key.to_string(), Value::String(value));
    }
}

fn js_truthy(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Bool(value)) => *value,
        Some(Value::Number(value)) => value.as_f64().is_some_and(|number| number != 0.0),
        Some(Value::String(value)) => !value.is_empty(),
        Some(Value::Array(_)) | Some(Value::Object(_)) => true,
        Some(Value::Null) | None => false,
    }
}

pub fn export_backup_archive_inner(
    app_local_data_dir: &Path,
    request: ExportBackupArchiveRequest,
) -> Result<BackupManifest, String> {
    let config = ensure_json_object_value(request.config, "Backup config")?;
    let analytics_json: Value =
        serde_json::from_str(&request.analytics_content).map_err(|error| error.to_string())?;
    ensure_json_object_value(analytics_json, "Backup analytics")?;

    let store = SqliteHistoryStore::new(app_local_data_dir.to_path_buf());
    let history = store
        .history_snapshot_for_backup()
        .map_err(|e| e.to_string())?;
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
        for (relative_path, snapshot) in history.snapshot_files {
            write_json_pretty_atomic(&history_dir.join(relative_path), &snapshot)?;
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

pub fn prepare_backup_import_inner(
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
        let projects = normalize_backup_projects(read_json_value(
            &extraction_dir
                .join(PROJECTS_DIR_NAME)
                .join(PROJECTS_INDEX_FILE_NAME),
        )?)?;

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
        let extraction_repository = HistoryRepository::new(extraction_dir.clone());

        let mut transcript_count = 0usize;
        let mut summary_count = 0usize;
        for item in &history_items {
            if item.status == HistoryItemStatus::Draft {
                return Err(format!(
                    "Backup history item \"{}\" is a draft and cannot be imported.",
                    item.id
                ));
            }

            let safe_id =
                ensure_safe_file_name(&item.id, &format!("History item ID for {}", item.id))?;
            let transcript_path = extraction_dir
                .join(HISTORY_DIR_NAME)
                .join(format!("{safe_id}.json"));
            ensure_json_array_value(
                read_json_value(&transcript_path)?,
                &format!("Transcript for history item {}", item.id),
            )?;
            transcript_count += 1;

            let summary_path = extraction_dir
                .join(HISTORY_DIR_NAME)
                .join(format!("{safe_id}{SUMMARY_FILE_SUFFIX}"));
            if summary_path.exists() {
                ensure_json_object_value(
                    read_json_value(&summary_path)?,
                    &format!("Summary for history item {}", item.id),
                )?;
                summary_count += 1;
            }

            let snapshot_dir = extraction_dir
                .join(HISTORY_DIR_NAME)
                .join(HISTORY_VERSIONS_DIR_NAME)
                .join(ensure_safe_file_name(
                    &item.id,
                    &format!("Transcript snapshot history id for {}", item.id),
                )?);
            if snapshot_dir.exists() {
                let snapshots = extraction_repository.list_transcript_snapshots(&item.id)?;
                for snapshot in snapshots {
                    let Some(record) =
                        extraction_repository.load_transcript_snapshot(&item.id, &snapshot.id)?
                    else {
                        return Err(format!(
                            "Transcript snapshot \"{}\" for history item \"{}\" is missing.",
                            snapshot.id, item.id
                        ));
                    };
                    if record.metadata.id != snapshot.id {
                        return Err(format!(
                            "Transcript snapshot \"{}\" for history item \"{}\" has mismatched metadata.",
                            snapshot.id, item.id
                        ));
                    }
                }
            }
        }

        let automation_rules = normalize_automation_rules(read_json_value(
            &extraction_dir
                .join(AUTOMATION_DIR_NAME)
                .join(AUTOMATION_RULES_FILE_NAME),
        )?)?;
        let automation_processed_entries =
            normalize_automation_processed_entries(read_json_value(
                &extraction_dir
                    .join(AUTOMATION_DIR_NAME)
                    .join(AUTOMATION_PROCESSED_FILE_NAME),
            )?)?;

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

pub fn apply_prepared_history_import_inner(
    _app_local_data_dir: &Path,
    _import_id: &str,
    extraction_dir: &Path,
) -> Result<(), String> {
    let extracted_history_dir = extraction_dir.join(HISTORY_DIR_NAME);
    if !extracted_history_dir.is_dir() {
        return Err("Prepared backup import is missing the history directory.".to_string());
    }

    let db = crate::core::database::Database::global().map_err(|e| e.to_string())?;

    let items: Vec<HistoryItemRecord> =
        read_json_value(&extracted_history_dir.join(PROJECTS_INDEX_FILE_NAME))
            .and_then(|v| ensure_json_array_value(v, "History index"))
            .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()))?;

    let mut transcript_entries: Vec<(String, String)> = Vec::with_capacity(items.len());
    let mut summary_entries: Vec<(String, String)> = Vec::new();
    let mut snapshot_data: Vec<(String, String, String, i64, i64, String)> = Vec::new();

    for item in &items {
        let safe_id = ensure_safe_file_name(&item.id, &format!("History item ID for {}", item.id))?;
        let transcript_path = extracted_history_dir.join(format!("{safe_id}.json"));
        let transcript_val = read_json_value(&transcript_path)?;
        let segments_str = serde_json::to_string(&transcript_val).map_err(|e| e.to_string())?;
        transcript_entries.push((item.id.clone(), segments_str));

        let summary_path = extracted_history_dir.join(format!("{safe_id}{SUMMARY_FILE_SUFFIX}"));
        if summary_path.exists() {
            let summary_val = read_json_value(&summary_path)?;
            let payload_str = serde_json::to_string(&summary_val).map_err(|e| e.to_string())?;
            summary_entries.push((item.id.clone(), payload_str));
        }

        let snapshot_dir = extracted_history_dir
            .join(HISTORY_VERSIONS_DIR_NAME)
            .join(&safe_id);
        if snapshot_dir.exists() {
            let snapshot_index_path = snapshot_dir.join(PROJECTS_INDEX_FILE_NAME);
            let snapshot_metadatas: Vec<TranscriptSnapshotMetadata> =
                read_json_value(&snapshot_index_path)
                    .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()))?;

            for meta in &snapshot_metadatas {
                let snapshot_path = snapshot_dir.join(format!("{}.json", meta.id));
                if snapshot_path.exists() {
                    let record_val = read_json_value(&snapshot_path)?;
                    let record: TranscriptSnapshotRecord =
                        serde_json::from_value(record_val).map_err(|e| e.to_string())?;

                    let reason_str = match record.metadata.reason {
                        TranscriptSnapshotReason::Polish => "polish",
                        TranscriptSnapshotReason::Translate => "translate",
                        TranscriptSnapshotReason::Retranscribe => "retranscribe",
                        TranscriptSnapshotReason::Restore => "restore",
                    };
                    let seg_str =
                        serde_json::to_string(&record.segments).map_err(|e| e.to_string())?;

                    snapshot_data.push((
                        record.metadata.id,
                        record.metadata.history_id,
                        reason_str.to_string(),
                        record.metadata.created_at as i64,
                        record.metadata.segment_count as i64,
                        seg_str,
                    ));
                }
            }
        }
    }

    db.with_transaction(|tx| {
        tx.execute("DELETE FROM transcript_snapshots", [])?;
        tx.execute("DELETE FROM history_summaries", [])?;
        tx.execute("DELETE FROM history_transcripts", [])?;
        tx.execute("DELETE FROM history_items", [])?;

        for item in &items {
            let kind_str = match item.kind {
                HistoryItemKind::Batch => "batch",
                HistoryItemKind::Recording => "recording",
            };
            let status_str = match item.status {
                HistoryItemStatus::Draft => "draft",
                HistoryItemStatus::Complete => "complete",
            };
            let draft_source_str = item.draft_source.map(|s| match s {
                HistoryDraftSource::LiveRecord => "live_record",
            });

            tx.execute(
                "INSERT INTO history_items (id, timestamp, duration, audio_path, transcript_path, title, preview_text, icon, kind, search_content, project_id, status, draft_source)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                rusqlite::params![
                    item.id,
                    item.timestamp as i64,
                    item.duration,
                    item.audio_path,
                    format!("{}.json", item.id),
                    item.title,
                    item.preview_text,
                    item.icon,
                    kind_str,
                    item.search_content,
                    item.project_id,
                    status_str,
                    draft_source_str,
                ],
            )?;
        }

        for (history_id, segments_str) in &transcript_entries {
            tx.execute(
                "INSERT INTO history_transcripts (history_id, segments) VALUES (?1, ?2)",
                rusqlite::params![history_id, segments_str],
            )?;
        }

        for (history_id, payload_str) in &summary_entries {
            tx.execute(
                "INSERT INTO history_summaries (history_id, payload) VALUES (?1, ?2)",
                rusqlite::params![history_id, payload_str],
            )?;
        }

        for (id, history_id, reason, created_at, segment_count, segments_str) in &snapshot_data {
            tx.execute(
                "INSERT INTO transcript_snapshots (id, history_id, reason, created_at, segment_count, segments)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![id, history_id, reason, created_at, segment_count, segments_str],
            )?;
        }

        Ok(())
    }).map_err(|e| e.to_string())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::database::Database;
    use crate::repositories::history::fs_utils::{
        create_tar_bz2_archive, extract_tar_bz2_archive, read_json_value, remove_path_if_exists,
        write_json_pretty_atomic,
    };
    use crate::repositories::history::sqlite_store::SqliteHistoryStore;
    use crate::repositories::history::state::PreparedBackupImportState;
    use crate::repositories::history::test_support::create_valid_backup_archive;
    use crate::repositories::history::types::TranscriptSnapshotReason;
    use crate::repositories::history::{
        ANALYTICS_DIR_NAME, ANALYTICS_USAGE_FILE_NAME, AUTOMATION_DIR_NAME,
        AUTOMATION_PROCESSED_FILE_NAME, AUTOMATION_RULES_FILE_NAME, CONFIG_DIR_NAME,
        CONFIG_FILE_NAME, HISTORY_DIR_NAME, HISTORY_VERSIONS_DIR_NAME, HistorySaveRecordingRequest,
        PROJECTS_DIR_NAME, PROJECTS_INDEX_FILE_NAME,
    };
    use serde_json::{Value, json};
    use std::fs;
    use std::sync::{Mutex, Once};
    use tempfile::tempdir;

    static BACKUP_TEST_LOCK: Mutex<()> = Mutex::new(());
    static INIT_GLOBAL_DB: Once = Once::new();

    fn init_global_db() {
        INIT_GLOBAL_DB.call_once(|| {
            let db = Database::open_in_memory().unwrap();
            Database::set_global(db).unwrap();
        });
    }

    fn clear_global_db() {
        Database::global()
            .unwrap()
            .with_transaction(|tx| {
                tx.execute("DELETE FROM history_items", [])?;
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn export_backup_archive_skips_draft_items_and_preserves_manifest() {
        let _guard = BACKUP_TEST_LOCK.lock().unwrap();
        init_global_db();
        clear_global_db();

        let root = tempdir().unwrap();
        let store = SqliteHistoryStore::new(root.path().to_path_buf());
        store.ensure_ready().unwrap();

        let keep_item = store
            .save_recording(HistorySaveRecordingRequest {
                segments: json!([{
                    "id": "seg-1",
                    "text": "keep",
                    "start": 0.0,
                    "end": 1.0,
                    "isFinal": true
                }]),
                duration: 1.0,
                project_id: None,
                audio_bytes: Some(vec![1, 2, 3]),
                native_audio_path: None,
                audio_extension: Some("wav".to_string()),
            })
            .unwrap();

        Database::global().unwrap()
            .with_transaction(|tx| {
                tx.execute(
                    "INSERT INTO history_items (id, timestamp, duration, audio_path, transcript_path, title, kind, status, draft_source)
                     VALUES ('draft', 1, 2.0, 'draft.wav', 'draft.json', 'Item draft', 'recording', 'draft', 'live_record')",
                    [],
                )?;
                tx.execute(
                    "INSERT INTO history_transcripts (history_id, segments) VALUES ('draft', '[]')",
                    [],
                )?;
                Ok(())
            })
            .unwrap();

        store
            .save_summary(&keep_item.id, json!({ "activeTemplateId": "general" }))
            .unwrap();

        let keep_snapshot = store
            .create_transcript_snapshot(
                &keep_item.id,
                TranscriptSnapshotReason::Polish,
                json!([{ "id": "seg-1", "text": "keep before" }]),
            )
            .unwrap();

        let _draft_snapshot = store
            .create_transcript_snapshot(
                "draft",
                TranscriptSnapshotReason::Translate,
                json!([{ "id": "seg-1", "text": "draft before" }]),
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
            keep_item.id
        );
        let exported_versions_dir = extract_dir
            .path()
            .join(HISTORY_DIR_NAME)
            .join(HISTORY_VERSIONS_DIR_NAME);
        let keep_versions_dir = exported_versions_dir.join(&keep_item.id);
        assert!(keep_versions_dir.join(PROJECTS_INDEX_FILE_NAME).exists());
        assert!(
            keep_versions_dir
                .join(format!("{}.json", keep_snapshot.id))
                .exists()
        );
        assert!(!exported_versions_dir.join("draft").exists());
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
    fn prepare_backup_import_rejects_missing_snapshot_record_before_mutation() {
        let archive_dir = tempdir().unwrap();
        let archive_path = archive_dir.path().join("invalid-snapshot-backup.tar.bz2");
        let staging_dir = tempdir().unwrap();
        let history_dir = staging_dir.path().join(HISTORY_DIR_NAME);
        let versions_dir = history_dir
            .join(HISTORY_VERSIONS_DIR_NAME)
            .join("history-1");
        fs::create_dir_all(&history_dir).unwrap();
        fs::create_dir_all(staging_dir.path().join(CONFIG_DIR_NAME)).unwrap();
        fs::create_dir_all(staging_dir.path().join(PROJECTS_DIR_NAME)).unwrap();
        fs::create_dir_all(staging_dir.path().join(AUTOMATION_DIR_NAME)).unwrap();
        fs::create_dir_all(staging_dir.path().join(ANALYTICS_DIR_NAME)).unwrap();
        fs::create_dir_all(&versions_dir).unwrap();
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
            &history_dir.join(PROJECTS_INDEX_FILE_NAME),
            &vec![json!({
                "id": "history-1",
                "audioPath": "history-1.webm",
                "transcriptPath": "history-1.json",
                "title": "Snapshot Broken",
                "projectId": null,
                "status": "complete"
            })],
        )
        .unwrap();
        write_json_pretty_atomic(
            &history_dir.join("history-1.json"),
            &json!([{ "id": "seg-1", "text": "hello" }]),
        )
        .unwrap();
        write_json_pretty_atomic(
            &versions_dir.join(PROJECTS_INDEX_FILE_NAME),
            &vec![json!({
                "id": "snapshot-1",
                "historyId": "history-1",
                "reason": "polish",
                "createdAt": 1,
                "segmentCount": 1
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
            err.contains("Transcript snapshot \"snapshot-1\"")
                || err.contains("snapshot-1.json")
                || err.contains("No such file")
                || err.contains("找不到指定的文件")
        );
    }

    #[test]
    fn prepare_backup_import_normalizes_sparse_projects_and_automation_payloads() {
        let archive_dir = tempdir().unwrap();
        let archive_path = archive_dir.path().join("sparse-backup.tar.bz2");
        create_valid_backup_archive(&archive_path);

        let (prepared, _snapshot) = prepare_backup_import_inner(&archive_path).unwrap();

        assert_eq!(prepared.projects.len(), 1);
        assert_eq!(prepared.projects[0]["id"], "project-1");
        assert_eq!(prepared.projects[0]["name"], "Workspace");
        assert_eq!(prepared.projects[0]["description"], "");
        assert_eq!(prepared.projects[0]["icon"], "");
        assert_eq!(
            prepared.projects[0]["defaults"]["summaryTemplateId"],
            "general"
        );
        assert_eq!(
            prepared.projects[0]["defaults"]["translationLanguage"],
            "zh"
        );
        assert_eq!(
            prepared.projects[0]["defaults"]["polishPresetId"],
            "general"
        );
        assert_eq!(
            prepared.projects[0]["defaults"]["enabledTextReplacementSetIds"],
            json!([])
        );
        assert_eq!(
            prepared.projects[0]["defaults"]["enabledHotwordSetIds"],
            json!([])
        );
        assert_eq!(
            prepared.projects[0]["defaults"]["enabledPolishKeywordSetIds"],
            json!([])
        );
        assert_eq!(
            prepared.projects[0]["defaults"]["enabledSpeakerProfileIds"],
            json!([])
        );

        assert_eq!(prepared.automation_rules.len(), 1);
        assert_eq!(prepared.automation_rules[0]["id"], "rule-1");
        assert_eq!(prepared.automation_rules[0]["name"], "");
        assert_eq!(prepared.automation_rules[0]["projectId"], "");
        assert_eq!(prepared.automation_rules[0]["presetId"], "custom");
        assert_eq!(prepared.automation_rules[0]["watchDirectory"], "");
        assert_eq!(prepared.automation_rules[0]["recursive"], false);
        assert_eq!(prepared.automation_rules[0]["enabled"], false);
        assert_eq!(
            prepared.automation_rules[0]["stageConfig"],
            json!({
                "autoPolish": false,
                "polishPresetId": "general",
                "autoTranslate": false,
                "translationLanguage": "en",
                "exportEnabled": false
            })
        );
        assert_eq!(
            prepared.automation_rules[0]["exportConfig"],
            json!({
                "directory": "",
                "format": "txt",
                "mode": "original",
                "prefix": ""
            })
        );
        assert_eq!(prepared.automation_rules[0]["createdAt"], 0);
        assert_eq!(prepared.automation_rules[0]["updatedAt"], 0);

        assert_eq!(prepared.automation_processed_entries.len(), 1);
        assert_eq!(prepared.automation_processed_entries[0]["ruleId"], "rule-1");
        assert_eq!(
            prepared.automation_processed_entries[0]["filePath"],
            "C:\\watch\\file.wav"
        );
        assert_eq!(
            prepared.automation_processed_entries[0]["sourceFingerprint"],
            ""
        );
        assert_eq!(prepared.automation_processed_entries[0]["size"], 0);
        assert_eq!(prepared.automation_processed_entries[0]["mtimeMs"], 0);
        assert_eq!(
            prepared.automation_processed_entries[0]["status"],
            "complete"
        );
        assert_eq!(prepared.automation_processed_entries[0]["processedAt"], 0);
    }

    #[test]
    fn apply_prepared_history_import_replaces_history_and_dispose_cleans_snapshot() {
        let _guard = BACKUP_TEST_LOCK.lock().unwrap();
        init_global_db();
        clear_global_db();

        let app_data_dir = tempdir().unwrap();

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

        let store = SqliteHistoryStore::new(app_data_dir.path().to_path_buf());
        let items = store.list_items().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "history-1");

        let transcript = store.load_transcript("history-1").unwrap().unwrap();
        assert_eq!(transcript.len(), 1);
        assert_eq!(transcript[0].id, "seg-1");

        let summary = store.load_summary("history-1").unwrap().unwrap();
        assert_eq!(summary["activeTemplateId"], "general");

        let removed = state.remove(&prepared.import_id).unwrap().unwrap();
        remove_path_if_exists(&removed.extraction_dir).unwrap();
        assert!(!removed.extraction_dir.exists());
    }

    #[test]
    fn export_and_import_round_trip_restores_transcript_snapshots() {
        let _guard = BACKUP_TEST_LOCK.lock().unwrap();
        init_global_db();
        clear_global_db();

        let source_dir = tempdir().unwrap();
        let source_store = SqliteHistoryStore::new(source_dir.path().to_path_buf());
        source_store.ensure_ready().unwrap();

        let item = source_store
            .save_recording(HistorySaveRecordingRequest {
                segments: json!([{
                    "id": "seg-1",
                    "text": "current",
                    "start": 0.0,
                    "end": 1.0,
                    "isFinal": true
                }]),
                duration: 1.0,
                project_id: None,
                audio_bytes: Some(vec![1, 2, 3]),
                native_audio_path: None,
                audio_extension: Some("wav".to_string()),
            })
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(1));
        let first = source_store
            .create_transcript_snapshot(
                &item.id,
                TranscriptSnapshotReason::Polish,
                json!([{ "id": "seg-1", "text": "before" }]),
            )
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1));
        let second = source_store
            .create_transcript_snapshot(
                &item.id,
                TranscriptSnapshotReason::Translate,
                json!([{ "id": "seg-1", "text": "after" }]),
            )
            .unwrap();

        let archive_dir = tempdir().unwrap();
        let archive_path = archive_dir.path().join("snapshot-round-trip.tar.bz2");
        export_backup_archive_inner(
            source_dir.path(),
            ExportBackupArchiveRequest {
                archive_path: archive_path.to_string_lossy().into_owned(),
                app_version: "0.6.4".to_string(),
                config: json!({ "theme": "auto" }),
                projects: vec![],
                automation_rules: vec![],
                automation_processed_entries: vec![],
                analytics_content: r#"{"schemaVersion":1}"#.to_string(),
            },
        )
        .unwrap();

        Database::global()
            .unwrap()
            .with_transaction(|tx| {
                tx.execute("DELETE FROM history_items", [])?;
                Ok(())
            })
            .unwrap();

        let restore_dir = tempdir().unwrap();
        let (prepared, snapshot) = prepare_backup_import_inner(&archive_path).unwrap();
        apply_prepared_history_import_inner(
            restore_dir.path(),
            &prepared.import_id,
            &snapshot.extraction_dir,
        )
        .unwrap();

        let restored_store = SqliteHistoryStore::new(restore_dir.path().to_path_buf());
        let snapshots = restored_store.list_transcript_snapshots(&item.id).unwrap();
        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].id, second.id);
        assert_eq!(snapshots[1].id, first.id);

        let restored_first = restored_store
            .load_transcript_snapshot(&item.id, &first.id)
            .unwrap()
            .unwrap();
        assert_eq!(
            restored_first.metadata.reason,
            TranscriptSnapshotReason::Polish
        );
        assert_eq!(restored_first.segments.len(), 1);
        assert_eq!(restored_first.segments[0].id, "seg-1");
        assert_eq!(restored_first.segments[0].text, "before");
    }
}
