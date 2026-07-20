use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Transaction;
use serde_json::Value;
use sona_core::tag::normalize_tag_value_with_timestamp;
use sona_core::task_ledger::types::TASK_LEDGER_VERSION;

use crate::{Database, DatabaseError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationReport {
    pub migrated: bool,
    pub domains: LegacyMigrationDomains,
    pub history_count: usize,
    pub project_count: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LegacyMigrationDomains {
    pub history: bool,
    pub projects: bool,
    pub automation: bool,
    pub task_ledger: bool,
    pub analytics: bool,
}

impl LegacyMigrationDomains {
    fn any(self) -> bool {
        self.history || self.projects || self.automation || self.task_ledger || self.analytics
    }
}

pub fn migrate_legacy_to_sqlite(
    db: &Database,
    app_dir: &Path,
) -> Result<MigrationReport, DatabaseError> {
    let domains = detect_legacy_domains(app_dir);

    if !domains.any() {
        return Ok(MigrationReport {
            migrated: false,
            domains,
            history_count: 0,
            project_count: 0,
            errors: vec![],
        });
    }

    db.with_transaction(|tx| {
        let mut errors: Vec<String> = Vec::new();
        let mut history_count: usize = 0;
        let mut project_count: usize = 0;
        let mut automation_rule_count: usize = 0;
        let mut automation_processed_count: usize = 0;
        let mut task_ledger_count: usize = 0;

        if domains.projects {
            let _ = migrate_projects(tx, app_dir, &mut errors, &mut project_count);
        }
        if domains.history {
            let _ = migrate_history(tx, app_dir, &mut errors, &mut history_count);
        }
        if domains.automation {
            let _ = migrate_automation(
                tx,
                app_dir,
                &mut errors,
                &mut automation_rule_count,
                &mut automation_processed_count,
            );
        }
        if domains.task_ledger {
            let _ = migrate_task_ledger(tx, app_dir, &mut errors, &mut task_ledger_count);
        }
        if domains.analytics {
            let _ = migrate_llm_usage(tx, app_dir, &mut errors);
        }
        let _ = verify_counts(
            tx,
            history_count,
            project_count,
            automation_rule_count,
            automation_processed_count,
            task_ledger_count,
            &mut errors,
        );

        Ok(MigrationReport {
            migrated: true,
            domains,
            history_count,
            project_count,
            errors,
        })
    })
}

fn detect_legacy_domains(app_dir: &Path) -> LegacyMigrationDomains {
    LegacyMigrationDomains {
        history: app_dir.join("history").join("index.json").exists(),
        projects: app_dir.join("projects").join("index.json").exists(),
        automation: app_dir.join("automation").join("rules.json").exists()
            || app_dir.join("automation").join("processed.json").exists(),
        task_ledger: app_dir.join("task-ledger").join("tasks.json").exists(),
        analytics: app_dir.join("analytics").join("llm-usage.json").exists(),
    }
}

pub fn move_legacy_to_backup(app_dir: &Path) -> Result<(), DatabaseError> {
    move_legacy_domains_to_backup(
        app_dir,
        LegacyMigrationDomains {
            history: true,
            projects: true,
            automation: true,
            task_ledger: true,
            analytics: true,
        },
    )
}

pub fn move_legacy_domains_to_backup(
    app_dir: &Path,
    domains: LegacyMigrationDomains,
) -> Result<(), DatabaseError> {
    let backup_dir = app_dir.join(".legacy-backup");
    fs::create_dir_all(&backup_dir)
        .map_err(|e| DatabaseError::ConnectionError(format!("Failed to create backup dir: {e}")))?;

    for (should_move, dir_name) in [
        (domains.history, "history"),
        (domains.projects, "projects"),
        (domains.automation, "automation"),
        (domains.task_ledger, "task-ledger"),
        (domains.analytics, "analytics"),
    ] {
        if !should_move {
            continue;
        }

        let src = app_dir.join(dir_name);
        if src.exists() {
            let dst = backup_dir.join(dir_name);
            if dst.exists() {
                fs::remove_dir_all(&dst).ok();
            }
            if let Err(e) = fs::rename(&src, &dst) {
                log::warn!("[Migration] Failed to move {}: {e}", src.display());
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers: read JSON files
// ---------------------------------------------------------------------------

fn try_read_json(path: &Path) -> Result<Option<Value>, String> {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content)
            .map(Some)
            .map_err(|e| e.to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("Failed to read {}: {e}", path.display())),
    }
}

fn try_read_json_or_skip(path: &Path, label: &str, errors: &mut Vec<String>) -> Option<Value> {
    match try_read_json(path) {
        Ok(Some(val)) => Some(val),
        Ok(None) => None,
        Err(e) => {
            errors.push(format!("{label}: {e}"));
            None
        }
    }
}

fn string_field(obj: &Value, key: &str) -> Option<String> {
    obj.get(key).and_then(Value::as_str).map(|s| s.to_string())
}

fn u64_field(obj: &Value, key: &str) -> u64 {
    obj.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn i64_field(obj: &Value, key: &str) -> i64 {
    obj.get(key)
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_u64().and_then(|n| i64::try_from(n).ok()))
                .or_else(|| value.as_f64().map(|n| n.round() as i64))
        })
        .unwrap_or(0)
}

fn f64_field(obj: &Value, key: &str) -> f64 {
    obj.get(key).and_then(Value::as_f64).unwrap_or(0.0).max(0.0)
}

fn bool_field(obj: &Value, key: &str) -> bool {
    obj.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn optional_string_field(obj: &Value, key: &str) -> Option<String> {
    obj.get(key).and_then(Value::as_str).map(ToOwned::to_owned)
}

fn nested_string_field(obj: &Value, parent: &str, key: &str, default: &str) -> String {
    obj.get(parent)
        .and_then(|value| value.get(key))
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_string()
}

fn nested_bool_field(obj: &Value, parent: &str, key: &str) -> bool {
    obj.get(parent)
        .and_then(|value| value.get(key))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn nested_string_array_field(obj: &Value, parent: &str, key: &str) -> Vec<String> {
    obj.get(parent)
        .and_then(|value| value.get(key))
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn current_time_millis() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .map_err(|error| error.to_string())
}

// ---------------------------------------------------------------------------
// History migration
// ---------------------------------------------------------------------------

fn migrate_history(
    tx: &Transaction,
    app_dir: &Path,
    errors: &mut Vec<String>,
    count: &mut usize,
) -> Result<(), DatabaseError> {
    let index_path = app_dir.join("history").join("index.json");
    let raw = match try_read_json(&index_path) {
        Ok(Some(v)) => v,
        Ok(None) => return Ok(()),
        Err(e) => {
            errors.push(format!("Failed to read history index: {e}"));
            return Ok(());
        }
    };

    let items = match raw {
        Value::Array(arr) => arr,
        _ => {
            errors.push("Unexpected format in history/index.json".to_string());
            return Ok(());
        }
    };

    let mut existing_tags = std::collections::HashSet::new();
    let mut tags_stmt = tx.prepare("SELECT id FROM tags")?;
    let mut tag_rows = tags_stmt.query([])?;
    while let Some(row) = tag_rows.next()? {
        let p_id: String = row.get(0)?;
        existing_tags.insert(p_id);
    }

    for item_val in items {
        if !item_val.is_object() {
            errors.push("History item is not an object".to_string());
            continue;
        }
        let id = match string_field(&item_val, "id") {
            Some(id) if !id.is_empty() => id,
            _ => {
                errors.push("History item missing id".to_string());
                continue;
            }
        };

        if let Err(e) = tx.execute(
            "INSERT OR IGNORE INTO history_items (id, timestamp, duration, audio_path, transcript_path, title, preview_text, icon, kind, search_content, status, draft_source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            rusqlite::params![
                &id,
                u64_field(&item_val, "timestamp") as i64,
                f64_field(&item_val, "duration"),
                string_field(&item_val, "audioPath").unwrap_or_default(),
                string_field(&item_val, "transcriptPath").unwrap_or_default(),
                string_field(&item_val, "title").unwrap_or_default(),
                string_field(&item_val, "previewText").unwrap_or_default(),
                string_field(&item_val, "icon"),
                match string_field(&item_val, "type").as_deref() {
                    Some("batch") => "batch",
                    _ => "recording",
                },
                string_field(&item_val, "searchContent").unwrap_or_default(),
                match string_field(&item_val, "status").as_deref() {
                    Some("draft") => "draft",
                    _ => "complete",
                },
                match string_field(&item_val, "draftSource").as_deref() {
                    Some("live_record") => Some("live_record"),
                    _ => None,
                },
            ],
        ) {
            errors.push(format!("Failed to insert history item {id}: {e}"));
            continue;
        }

        if let Some(tag_id) =
            string_field(&item_val, "projectId").filter(|tag_id| existing_tags.contains(tag_id))
            && let Err(error) = tx.execute(
                "INSERT OR IGNORE INTO history_item_tags (history_id, tag_id) VALUES (?1, ?2)",
                rusqlite::params![&id, tag_id],
            )
        {
            errors.push(format!("Failed to assign history tag {id}: {error}"));
        }

        // Migrate transcript
        migrate_item_transcript(tx, app_dir, &id, &item_val, errors);

        // Migrate summary
        migrate_item_summary(tx, app_dir, &id, errors);

        // Migrate snapshots
        migrate_item_snapshots(tx, app_dir, &id, errors);

        *count += 1;
    }

    Ok(())
}

fn migrate_item_transcript(
    tx: &Transaction,
    app_dir: &Path,
    id: &str,
    item_val: &Value,
    errors: &mut Vec<String>,
) {
    // v0.7.4 stores transcripts as history/{transcriptPath}, normally {id}.json.
    let paths = [
        try_resolve_path(
            app_dir,
            "history",
            &string_field(item_val, "transcriptPath").unwrap_or_default(),
        ),
        Some(app_dir.join("history").join(format!("{id}.json"))),
    ];

    let segments_val = paths
        .iter()
        .flatten()
        .find_map(|p| try_read_json_or_skip(p, &format!("Transcript for {id}"), errors));

    let Some(segments_val) = segments_val else {
        return;
    };

    let segments_str = match extract_segments_string(&segments_val) {
        Ok(s) => s,
        Err(e) => {
            errors.push(format!("Failed to process transcript for {id}: {e}"));
            return;
        }
    };

    if let Err(e) = tx.execute(
        "INSERT OR IGNORE INTO history_transcripts (history_id, segments) VALUES (?1, ?2)",
        rusqlite::params![id, &segments_str],
    ) {
        errors.push(format!("Failed to insert transcript for {id}: {e}"));
    }
}

fn migrate_item_summary(tx: &Transaction, app_dir: &Path, id: &str, errors: &mut Vec<String>) {
    let summary_path = app_dir.join("history").join(format!("{id}.summary.json"));
    let summary_val = try_read_json_or_skip(&summary_path, &format!("Summary for {id}"), errors);

    let Some(summary_val) = summary_val else {
        return;
    };

    let payload_str = match serde_json::to_string(&summary_val) {
        Ok(s) => s,
        Err(e) => {
            errors.push(format!("Failed to serialize summary for {id}: {e}"));
            return;
        }
    };

    if let Err(e) = tx.execute(
        "INSERT OR IGNORE INTO history_summaries (history_id, payload) VALUES (?1, ?2)",
        rusqlite::params![id, &payload_str],
    ) {
        errors.push(format!("Failed to insert summary for {id}: {e}"));
    }
}

fn migrate_item_snapshots(tx: &Transaction, app_dir: &Path, id: &str, errors: &mut Vec<String>) {
    let snapshot_dir = app_dir.join("history").join("versions").join(id);
    let index_val = try_read_json_or_skip(
        &snapshot_dir.join("index.json"),
        &format!("Snapshot index for {id}"),
        errors,
    );

    let Some(index_val) = index_val else { return };

    let snapshots_meta = match index_val {
        Value::Array(arr) => arr,
        _ => {
            errors.push(format!("Snapshot index for {id} is not an array"));
            return;
        }
    };

    for meta in &snapshots_meta {
        let snapshot_id = match string_field(meta, "id") {
            Some(id) if !id.is_empty() => id,
            _ => continue,
        };
        let reason = string_field(meta, "reason").unwrap_or_else(|| "polish".to_string());
        let created_at = u64_field(meta, "created_at") as i64;
        let segment_count = u64_field(meta, "segment_count") as i64;

        let segments_str = try_read_json_or_skip(
            &snapshot_dir.join(format!("{snapshot_id}.json")),
            &format!("Snapshot data for {id}/{snapshot_id}"),
            errors,
        )
        .and_then(|val| {
            let record_obj = val.as_object()?;
            let segs = record_obj.get("segments")?;
            serde_json::to_string(segs).ok()
        })
        .unwrap_or_else(|| "[]".to_string());

        if let Err(e) = tx.execute(
            "INSERT OR IGNORE INTO transcript_snapshots (id, history_id, reason, created_at, segment_count, segments)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![&snapshot_id, id, &reason, created_at, segment_count, &segments_str],
        ) {
            errors.push(format!("Failed to insert snapshot {snapshot_id} for {id}: {e}"));
        }
    }
}

fn try_resolve_path(app_dir: &Path, parent: &str, file_name: &str) -> Option<std::path::PathBuf> {
    if file_name.is_empty()
        || file_name.contains("..")
        || file_name.contains('/')
        || file_name.contains('\\')
    {
        return None;
    }
    Some(app_dir.join(parent).join(file_name))
}

fn extract_segments_string(value: &Value) -> Result<String, String> {
    let Value::Array(segments) = value else {
        return Err("Transcript is not an array".to_string());
    };
    serde_json::to_string(&segments).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Projects migration
// ---------------------------------------------------------------------------

fn migrate_projects(
    tx: &Transaction,
    app_dir: &Path,
    errors: &mut Vec<String>,
    count: &mut usize,
) -> Result<(), DatabaseError> {
    let path = app_dir.join("projects").join("index.json");
    let raw = match try_read_json(&path) {
        Ok(Some(v)) => v,
        Ok(None) => return Ok(()),
        Err(e) => {
            errors.push(format!("Failed to read projects index: {e}"));
            return Ok(());
        }
    };

    let projects = match raw {
        Value::Array(arr) => arr,
        _ => {
            errors.push("Unexpected format in projects/index.json".to_string());
            return Ok(());
        }
    };

    let fallback_timestamp = current_time_millis().unwrap_or(0);

    for (i, project_val) in projects.iter().enumerate() {
        let mut project = normalize_tag_value_with_timestamp(project_val, fallback_timestamp);
        if project.id.is_empty() {
            project.id = format!("_migrated_{i}");
        }
        let defaults_obj = project_val.get("defaults").and_then(Value::as_object);
        if project.description.is_empty()
            && let Some(description) = defaults_obj
                .and_then(|d| d.get("description"))
                .and_then(Value::as_str)
        {
            project.description = description.to_string();
        }
        let id = project.id.clone();

        if let Err(e) = tx.execute(
            "INSERT OR IGNORE INTO tags (
                id, name, description, icon, color, sort_order, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                &id,
                &project.name,
                &project.description,
                &project.icon,
                string_field(project_val, "color").unwrap_or_default(),
                i as i64,
                project.created_at as i64,
                project.updated_at as i64,
            ],
        ) {
            errors.push(format!("Failed to insert project {id}: {e}"));
            continue;
        }
        if let Err(e) = insert_legacy_project_automation(tx, &id, &project.name, project_val) {
            errors.push(format!("Failed to migrate project defaults for {id}: {e}"));
            continue;
        }

        *count += 1;
    }

    Ok(())
}

fn insert_legacy_project_automation(
    tx: &Transaction,
    tag_id: &str,
    tag_name: &str,
    source: &Value,
) -> Result<(), rusqlite::Error> {
    let profile_id = format!("migrated-tag-profile:{tag_id}");
    let rule_id = format!("migrated-tag-rule:{tag_id}");
    let created_at = i64_field(source, "createdAt");
    let updated_at = i64_field(source, "updatedAt");
    tx.execute(
        "INSERT OR IGNORE INTO automation_profiles (
            id, name, translation_language, polish_preset_id, summary_template_id,
            created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            &profile_id,
            tag_name,
            nested_string_field(source, "defaults", "translationLanguage", "zh"),
            nested_string_field(source, "defaults", "polishPresetId", "general"),
            nested_string_field(source, "defaults", "summaryTemplateId", "general"),
            created_at,
            updated_at,
        ],
    )?;
    for (kind, key) in [
        ("text_replacement", "enabledTextReplacementSetIds"),
        ("hotword", "enabledHotwordSetIds"),
        ("polish_keyword", "enabledPolishKeywordSetIds"),
        ("speaker_profile", "enabledSpeakerProfileIds"),
    ] {
        insert_automation_profile_link_kind(
            tx,
            &profile_id,
            kind,
            &nested_string_array_field(source, "defaults", key),
        )?;
    }
    tx.execute(
        "INSERT OR IGNORE INTO automation_rules (
            id, name, kind, priority, profile_id, profile_source, save_history,
            preset_id, watch_directory, recursive, enabled, stage_auto_polish,
            stage_polish_preset_id, stage_auto_translate, stage_translation_language,
            stage_export_enabled, action_auto_summary, export_directory, export_format,
            export_mode, export_prefix, created_at, updated_at
         ) VALUES (?1, ?2, 'tag', 0, ?3, 'explicit', 1, 'custom', '', 0, 1, 0,
                   'general', 0, 'zh', 0, 0, '', 'txt', 'original', '', ?4, ?5)",
        rusqlite::params![&rule_id, tag_name, &profile_id, created_at, updated_at],
    )?;
    tx.execute(
        "INSERT OR IGNORE INTO automation_rule_tags (rule_id, tag_id) VALUES (?1, ?2)",
        rusqlite::params![&rule_id, tag_id],
    )?;
    Ok(())
}

fn insert_automation_profile_link_kind(
    tx: &Transaction,
    profile_id: &str,
    kind: &str,
    target_ids: &[String],
) -> Result<(), rusqlite::Error> {
    let mut stmt = tx.prepare_cached(
        "INSERT OR IGNORE INTO automation_profile_links (profile_id, kind, target_id, sort_order)
         VALUES (?1, ?2, ?3, ?4)",
    )?;
    for (sort_order, target_id) in target_ids.iter().enumerate() {
        stmt.execute(rusqlite::params![
            profile_id,
            kind,
            target_id,
            sort_order as i64
        ])?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Automation migration
// ---------------------------------------------------------------------------

fn migrate_automation(
    tx: &Transaction,
    app_dir: &Path,
    errors: &mut Vec<String>,
    rule_count: &mut usize,
    processed_count: &mut usize,
) -> Result<(), DatabaseError> {
    // Rules
    let rules_path = app_dir.join("automation").join("rules.json");
    if let Some(rules) = try_read_json_or_skip(&rules_path, "automation/rules.json", errors) {
        if let Value::Array(items) = rules {
            for rule in items {
                let id = rule
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let id = if id.is_empty() {
                    uuid::Uuid::new_v4().to_string()
                } else {
                    id
                };
                let project_id = string_field(&rule, "projectId").unwrap_or_default();
                let legacy_auto_polish = nested_bool_field(&rule, "stageConfig", "autoPolish");
                let legacy_auto_translate =
                    nested_bool_field(&rule, "stageConfig", "autoTranslate");
                let migration_notice = if legacy_auto_polish || legacy_auto_translate {
                    Some(
                        "Legacy automatic polish/translation was disabled during migration. Configure a Tag automation to enable it.",
                    )
                } else {
                    None
                };
                if let Err(e) = tx.execute(
                    "INSERT OR IGNORE INTO automation_rules (
                        id, name, kind, priority, profile_id, profile_source, save_history,
                        preset_id, watch_directory, recursive, enabled, stage_auto_polish,
                        stage_polish_preset_id, stage_auto_translate, stage_translation_language,
                        stage_export_enabled, action_auto_summary, export_directory, export_format,
                        export_mode, export_prefix, created_at, updated_at, migration_notice
                    )
                    VALUES (?1, ?2, 'file', 0, NULL, 'tag_match', ?3, ?4, ?5, ?6, ?7,
                            0, ?8, 0, ?9, ?10, 0, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
                    rusqlite::params![
                        &id,
                        string_field(&rule, "name").unwrap_or_default(),
                        (project_id != "none") as i64,
                        string_field(&rule, "presetId").unwrap_or_else(|| "custom".to_string()),
                        string_field(&rule, "watchDirectory").unwrap_or_default(),
                        bool_field(&rule, "recursive") as i64,
                        bool_field(&rule, "enabled") as i64,
                        nested_string_field(&rule, "stageConfig", "polishPresetId", "general"),
                        nested_string_field(&rule, "stageConfig", "translationLanguage", "en"),
                        nested_bool_field(&rule, "stageConfig", "exportEnabled") as i64,
                        nested_string_field(&rule, "exportConfig", "directory", ""),
                        nested_string_field(&rule, "exportConfig", "format", "txt"),
                        nested_string_field(&rule, "exportConfig", "mode", "original"),
                        nested_string_field(&rule, "exportConfig", "prefix", ""),
                        i64_field(&rule, "createdAt"),
                        i64_field(&rule, "updatedAt"),
                        migration_notice,
                    ],
                ) {
                    errors.push(format!("Failed to insert automation rule: {e}"));
                    continue;
                }
                if !matches!(project_id.as_str(), "" | "inbox" | "none")
                    && let Err(error) = tx.execute(
                        "INSERT OR IGNORE INTO automation_rule_tags (rule_id, tag_id)
                         SELECT ?1, id FROM tags WHERE id = ?2",
                        rusqlite::params![&id, &project_id],
                    )
                {
                    errors.push(format!("Failed to assign automation tag: {error}"));
                    continue;
                }
                *rule_count += 1;
            }
        } else {
            errors.push("automation/rules.json is not an array".to_string());
        }
    }

    // Processed entries
    let processed_path = app_dir.join("automation").join("processed.json");
    if let Some(processed) =
        try_read_json_or_skip(&processed_path, "automation/processed.json", errors)
    {
        if let Value::Array(items) = processed {
            for entry in items {
                let id = entry
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let id = if id.is_empty() {
                    uuid::Uuid::new_v4().to_string()
                } else {
                    id
                };
                if let Err(e) = tx.execute(
                    "INSERT OR IGNORE INTO automation_processed (
                        id, rule_id, file_path, source_fingerprint, size, mtime_ms,
                        status, processed_at, history_id, export_path, error_message
                    )
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    rusqlite::params![
                        &id,
                        string_field(&entry, "ruleId").unwrap_or_default(),
                        string_field(&entry, "filePath").unwrap_or_default(),
                        string_field(&entry, "sourceFingerprint").unwrap_or_default(),
                        i64_field(&entry, "size"),
                        i64_field(&entry, "mtimeMs"),
                        string_field(&entry, "status").unwrap_or_else(|| "complete".to_string()),
                        i64_field(&entry, "processedAt"),
                        optional_string_field(&entry, "historyId"),
                        optional_string_field(&entry, "exportPath"),
                        optional_string_field(&entry, "errorMessage"),
                    ],
                ) {
                    errors.push(format!("Failed to insert automation processed: {e}"));
                    continue;
                }
                *processed_count += 1;
            }
        } else {
            errors.push("automation/processed.json is not an array".to_string());
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Task ledger migration
// ---------------------------------------------------------------------------

fn migrate_task_ledger(
    tx: &Transaction,
    app_dir: &Path,
    errors: &mut Vec<String>,
    count: &mut usize,
) -> Result<(), DatabaseError> {
    let path = app_dir.join("task-ledger").join("tasks.json");
    let raw = match try_read_json(&path) {
        Ok(Some(v)) => v,
        Ok(None) => return Ok(()),
        Err(e) => {
            errors.push(format!("Failed to read task ledger: {e}"));
            return Ok(());
        }
    };

    let snapshot_obj = match raw.as_object() {
        Some(obj) => obj,
        None => {
            errors.push("task-ledger/tasks.json is not an object".to_string());
            return Ok(());
        }
    };

    let tasks = match snapshot_obj.get("tasks").and_then(Value::as_array) {
        Some(arr) => arr,
        None => return Ok(()),
    };

    for task in tasks {
        let id = string_field(task, "id").unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let tag_ids = task
            .get("tagIds")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| {
                optional_string_field(task, "projectId")
                    .filter(|id| !matches!(id.as_str(), "" | "inbox" | "none"))
                    .into_iter()
                    .collect()
            });
        let tag_ids_json = serde_json::to_string(&tag_ids).unwrap_or_else(|_| "[]".to_string());

        if let Err(e) = tx.execute(
            "INSERT OR IGNORE INTO task_ledger (
                id, kind, status, title, progress, created_at, updated_at,
                retryable, cancelable, recoverable, stage, history_id, tag_ids,
                file_path, automation_rule_id, source_fingerprint, error_message,
                template_id, target_language, version
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
            rusqlite::params![
                &id,
                string_field(task, "kind").unwrap_or_else(|| "llmPolish".to_string()),
                string_field(task, "status").unwrap_or_else(|| "pending".to_string()),
                string_field(task, "title").unwrap_or_else(|| "Task".to_string()),
                f64_field(task, "progress").clamp(0.0, 100.0),
                i64_field(task, "createdAt"),
                i64_field(task, "updatedAt"),
                bool_field(task, "retryable") as i64,
                bool_field(task, "cancelable") as i64,
                bool_field(task, "recoverable") as i64,
                optional_string_field(task, "stage"),
                optional_string_field(task, "historyId"),
                tag_ids_json,
                optional_string_field(task, "filePath"),
                optional_string_field(task, "automationRuleId"),
                optional_string_field(task, "sourceFingerprint"),
                optional_string_field(task, "errorMessage"),
                optional_string_field(task, "templateId"),
                optional_string_field(task, "targetLanguage"),
                TASK_LEDGER_VERSION as i64,
            ],
        ) {
            errors.push(format!("Failed to insert task {id}: {e}"));
            continue;
        }
        *count += 1;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// LLM Usage migration
// ---------------------------------------------------------------------------

fn migrate_llm_usage(
    tx: &Transaction,
    app_dir: &Path,
    errors: &mut Vec<String>,
) -> Result<(), DatabaseError> {
    let path = app_dir.join("analytics").join("llm-usage.json");
    let raw = match try_read_json(&path) {
        Ok(Some(v)) => v,
        Ok(None) => return Ok(()),
        Err(e) => {
            errors.push(format!("Failed to read LLM usage: {e}"));
            return Ok(());
        }
    };

    let obj = match raw.as_object() {
        Some(obj) => obj,
        None => {
            errors.push("analytics/llm-usage.json is not an object".to_string());
            return Ok(());
        }
    };

    let started_at = obj.get("startedAt").and_then(Value::as_str).unwrap_or("");

    let by_provider = obj
        .get("byProvider")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    for (provider, bucket) in &by_provider {
        let prompt_tokens = bucket
            .get("promptTokens")
            .and_then(Value::as_u64)
            .unwrap_or(0) as i64;
        let completion_tokens = bucket
            .get("completionTokens")
            .and_then(Value::as_u64)
            .unwrap_or(0) as i64;
        let total_tokens = bucket
            .get("totalTokens")
            .and_then(Value::as_u64)
            .unwrap_or(0) as i64;
        let call_count = bucket.get("callCount").and_then(Value::as_u64).unwrap_or(0);

        if call_count == 0 {
            continue;
        }

        if let Err(e) = tx.execute(
            "INSERT INTO analytics.llm_usage (occurred_at, provider, category, prompt_tokens, completion_tokens, total_tokens)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![started_at, provider, "migrated", prompt_tokens, completion_tokens, total_tokens],
        ) {
            errors.push(format!("Failed to insert LLM usage for {provider}: {e}"));
        }
    }

    // If no by_provider data, insert overall totals as a single record
    if by_provider.is_empty()
        && let Some(totals) = obj.get("totals").and_then(Value::as_object)
    {
        let prompt_tokens = totals
            .get("promptTokens")
            .and_then(Value::as_u64)
            .unwrap_or(0) as i64;
        let completion_tokens = totals
            .get("completionTokens")
            .and_then(Value::as_u64)
            .unwrap_or(0) as i64;
        let total_tokens = totals
            .get("totalTokens")
            .and_then(Value::as_u64)
            .unwrap_or(0) as i64;
        let call_count = totals.get("callCount").and_then(Value::as_u64).unwrap_or(0);

        if call_count > 0
                && let Err(e) = tx.execute(
                    "INSERT INTO analytics.llm_usage (occurred_at, provider, category, prompt_tokens, completion_tokens, total_tokens)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    rusqlite::params![started_at, "total", "migrated", prompt_tokens, completion_tokens, total_tokens],
                ) {
                    errors.push(format!("Failed to insert LLM usage totals: {e}"));
                }
    }

    // Migrate daily usage
    if let Some(daily) = obj.get("daily").and_then(Value::as_object) {
        for (date, bucket) in daily {
            let prompt_tokens = bucket
                .get("promptTokens")
                .and_then(Value::as_u64)
                .unwrap_or(0) as i64;
            let completion_tokens = bucket
                .get("completionTokens")
                .and_then(Value::as_u64)
                .unwrap_or(0) as i64;
            let total_tokens = bucket
                .get("totalTokens")
                .and_then(Value::as_u64)
                .unwrap_or(0) as i64;
            let call_count = bucket.get("callCount").and_then(Value::as_u64).unwrap_or(0);
            if call_count > 0
                && let Err(e) = tx.execute(
                    "INSERT INTO analytics.llm_usage (occurred_at, provider, category, prompt_tokens, completion_tokens, total_tokens)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    rusqlite::params![date, "daily", "migrated", prompt_tokens, completion_tokens, total_tokens],
                ) {
                    errors.push(format!("Failed to insert daily LLM usage for {date}: {e}"));
                }
        }
    }

    // Migrate byCategory usage
    if let Some(by_category) = obj.get("byCategory").and_then(Value::as_object) {
        for (category, bucket) in by_category {
            let prompt_tokens = bucket
                .get("promptTokens")
                .and_then(Value::as_u64)
                .unwrap_or(0) as i64;
            let completion_tokens = bucket
                .get("completionTokens")
                .and_then(Value::as_u64)
                .unwrap_or(0) as i64;
            let total_tokens = bucket
                .get("totalTokens")
                .and_then(Value::as_u64)
                .unwrap_or(0) as i64;
            let call_count = bucket.get("callCount").and_then(Value::as_u64).unwrap_or(0);
            if call_count > 0
                && let Err(e) = tx.execute(
                    "INSERT INTO analytics.llm_usage (occurred_at, provider, category, prompt_tokens, completion_tokens, total_tokens)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    rusqlite::params![started_at, "byCategory", category, prompt_tokens, completion_tokens, total_tokens],
                ) {
                    errors.push(format!("Failed to insert byCategory LLM usage for {category}: {e}"));
                }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Verification
// ---------------------------------------------------------------------------

fn verify_counts(
    tx: &Transaction,
    history_count: usize,
    project_count: usize,
    automation_rule_count: usize,
    automation_processed_count: usize,
    task_ledger_count: usize,
    errors: &mut Vec<String>,
) -> Result<(), DatabaseError> {
    let mut verify = |table: &str, expected: usize, label: &str| {
        if expected == 0 {
            return;
        }
        let count: i64 = tx
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap_or(0);
        if count as usize != expected {
            errors.push(format!(
                "Row count mismatch in {label}: expected {expected}, got {count}"
            ));
        }
    };

    verify("history_items", history_count, "history_items");
    verify("tags", project_count, "tags");
    verify(
        "automation_rules",
        automation_rule_count + project_count,
        "automation_rules",
    );
    verify(
        "automation_processed",
        automation_processed_count,
        "automation_processed",
    );
    verify("task_ledger", task_ledger_count, "task_ledger");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    fn with_open_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    fn write_json(path: &Path, value: &Value) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, serde_json::to_string_pretty(value).unwrap()).unwrap();
    }

    // ---- test_migrate_empty ----

    #[test]
    fn test_migrate_empty() {
        let dir = tempfile::tempdir().unwrap();
        let db = with_open_db();
        let report = migrate_legacy_to_sqlite(&db, dir.path()).unwrap();
        assert!(!report.migrated);
        assert_eq!(report.history_count, 0);
        assert_eq!(report.project_count, 0);
        assert!(report.errors.is_empty());
    }

    // ---- test_migrate_history ----

    #[test]
    fn test_migrate_history() {
        let dir = tempfile::tempdir().unwrap();
        let db = with_open_db();

        let items = json!([
            {
                "id": "hist-1",
                "timestamp": 1000,
                "duration": 5.0,
                "audioPath": "hist-1.wav",
                "transcriptPath": "hist-1.json",
                "title": "Test Item",
                "previewText": "Hello...",
                "icon": "mic",
                "type": "recording",
                "searchContent": "Hello world",
                "projectId": null,
                "status": "complete",
                "draftSource": null
            },
            {
                "id": "hist-2",
                "timestamp": 2000,
                "duration": 3.0,
                "audioPath": "hist-2.wav",
                "transcriptPath": "hist-2.json",
                "title": "Batch Item",
                "previewText": "Batch...",
                "icon": null,
                "type": "batch",
                "searchContent": "batch import",
                "projectId": "proj-1",
                "status": "complete",
                "draftSource": null
            }
        ]);

        write_json(&dir.path().join("history").join("index.json"), &items);

        std::fs::create_dir_all(dir.path().join("projects")).unwrap();
        write_json(
            &dir.path().join("projects").join("index.json"),
            &json!([
                {
                    "id": "proj-1",
                    "name": "Project 1",
                    "icon": "folder",
                    "color": "",
                    "sortOrder": 0,
                    "createdAt": 1000,
                    "updatedAt": 1000
                }
            ]),
        );
        // Transcripts
        write_json(
            &dir.path().join("history").join("hist-1.json"),
            &json!([{"id": "seg-1", "text": "Hello", "start": 0.0, "end": 1.0, "isFinal": true}]),
        );
        write_json(
            &dir.path().join("history").join("hist-2.json"),
            &json!([{"id": "seg-2", "text": "Batch", "start": 0.0, "end": 2.0, "isFinal": true}]),
        );
        // Summary for hist-1
        write_json(
            &dir.path().join("history").join("hist-1.summary.json"),
            &json!({"activeTemplateId": "general"}),
        );
        // Snapshot for hist-1
        write_json(
            &dir.path()
                .join("history")
                .join("versions")
                .join("hist-1")
                .join("index.json"),
            &json!([{"id": "snap-1", "reason": "polish", "created_at": 3000, "segment_count": 1}]),
        );
        write_json(
            &dir.path()
                .join("history")
                .join("versions")
                .join("hist-1")
                .join("snap-1.json"),
            &json!({"segments": [{"id": "ss-seg-1", "text": "Snapshot text", "start": 0.0, "end": 1.0, "isFinal": true}]}),
        );

        let report = migrate_legacy_to_sqlite(&db, dir.path()).unwrap();
        assert!(report.migrated);
        assert_eq!(report.history_count, 2);
        assert!(report.errors.is_empty(), "Errors: {:?}", report.errors);

        // Verify DB rows
        db.with_connection(|conn| {
            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM history_items", [], |r| r.get(0))
                .unwrap();
            assert_eq!(count, 2);

            let t_count: i64 = conn
                .query_row("SELECT COUNT(*) FROM history_transcripts", [], |r| r.get(0))
                .unwrap();
            assert_eq!(t_count, 2);

            let s_count: i64 = conn
                .query_row("SELECT COUNT(*) FROM history_summaries", [], |r| r.get(0))
                .unwrap();
            assert_eq!(s_count, 1);

            let snap_count: i64 = conn
                .query_row("SELECT COUNT(*) FROM transcript_snapshots", [], |r| {
                    r.get(0)
                })
                .unwrap();
            assert_eq!(snap_count, 1);

            let tag_id: Option<String> = conn
                .query_row(
                    "SELECT tag_id FROM history_item_tags WHERE history_id = 'hist-2'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(tag_id, Some("proj-1".to_string()));

            Ok(())
        })
        .unwrap();
    }

    // ---- test_migrate_projects ----

    #[test]
    fn test_migrate_projects() {
        let dir = tempfile::tempdir().unwrap();
        let db = with_open_db();

        let projects = json!([
            {
                "id": "proj-1",
                "name": "Work",
                "icon": "folder",
                "createdAt": 1000,
                "updatedAt": 2000,
                "defaults": {
                    "summaryTemplateId": "detailed",
                    "translationLanguage": "en",
                    "polishPresetId": "formal",
                    "polishScenario": "scenario1",
                    "polishContext": "context1",
                    "exportFileNamePrefix": "work-",
                    "description": "A work project",
                    "enabledTextReplacementSetIds": ["tr-1", "tr-2"],
                    "enabledHotwordSetIds": ["hw-1"],
                    "enabledPolishKeywordSetIds": ["kw-1"],
                    "enabledSpeakerProfileIds": ["sp-1"]
                }
            },
            {
                "id": "proj-2",
                "name": "Personal",
                "icon": "user",
                "createdAt": 3000,
                "updatedAt": 4000,
                "defaults": {}
            }
        ]);

        write_json(&dir.path().join("projects").join("index.json"), &projects);

        // Need history/index.json as sentinel
        write_json(&dir.path().join("history").join("index.json"), &json!([]));

        let report = migrate_legacy_to_sqlite(&db, dir.path()).unwrap();
        assert!(
            report.migrated,
            "migrated should be true, errors: {:?}",
            report.errors
        );
        assert_eq!(report.project_count, 2);
        assert!(report.errors.is_empty(), "Errors: {:?}", report.errors);

        db.with_connection(|conn| {
            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM tags", [], |r| r.get(0))
                .unwrap();
            assert_eq!(count, 2);

            let project: String = conn
                .query_row("SELECT description FROM tags WHERE id = 'proj-1'", [], |r| {
                    r.get(0)
                })
                .unwrap();
            assert_eq!(project, "A work project");

            let links: Vec<(String, String)> = conn
                .prepare(
                    "SELECT kind, target_id FROM automation_profile_links
                     WHERE profile_id = 'migrated-tag-profile:proj-1'
                     ORDER BY kind, sort_order",
                )
                .unwrap()
                .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            assert_eq!(
                links,
                vec![
                    ("hotword".to_string(), "hw-1".to_string()),
                    ("polish_keyword".to_string(), "kw-1".to_string()),
                    ("speaker_profile".to_string(), "sp-1".to_string()),
                    ("text_replacement".to_string(), "tr-1".to_string()),
                    ("text_replacement".to_string(), "tr-2".to_string()),
                ]
            );

            // Check proj-2 defaults
            let tmpl: String = conn
                .query_row(
                    "SELECT summary_template_id FROM automation_profiles WHERE id = 'migrated-tag-profile:proj-2'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(tmpl, "general");

            Ok(())
        })
        .unwrap();
    }

    // ---- test_migrate_automation ----

    #[test]
    fn test_migrate_automation() {
        let dir = tempfile::tempdir().unwrap();
        let db = with_open_db();

        write_json(
            &dir.path().join("automation").join("rules.json"),
            &json!([
                {
                    "id": "rule-1",
                    "name": "Watch Docs",
                    "watchDirectory": "/docs",
                    "projectId": "proj-1",
                    "presetId": "preset-1",
                    "recursive": true,
                    "enabled": true,
                    "stageConfig": {
                        "autoPolish": true,
                        "polishPresetId": "formal",
                        "autoTranslate": true,
                        "translationLanguage": "ja",
                        "exportEnabled": true
                    },
                    "exportConfig": {
                        "directory": "/exports",
                        "format": "md",
                        "mode": "translated",
                        "prefix": "auto-"
                    },
                    "createdAt": 1000,
                    "updatedAt": 2000
                },
                {"name": "No ID Rule", "watchDirectory": "/tmp", "projectId": "proj-2"}
            ]),
        );
        write_json(
            &dir.path().join("automation").join("processed.json"),
            &json!([
                {
                    "id": "proc-1",
                    "ruleId": "rule-1",
                    "filePath": "/docs/file.txt",
                    "sourceFingerprint": "fp-1",
                    "size": 123,
                    "mtimeMs": 456,
                    "status": "failed",
                    "processedAt": 789,
                    "historyId": "hist-1",
                    "exportPath": "/exports/file.md",
                    "errorMessage": "boom"
                }
            ]),
        );
        write_json(&dir.path().join("history").join("index.json"), &json!([]));

        let report = migrate_legacy_to_sqlite(&db, dir.path()).unwrap();
        assert!(report.migrated);
        assert!(report.errors.is_empty(), "Errors: {:?}", report.errors);

        db.with_connection(|conn| {
            let r_count: i64 = conn
                .query_row("SELECT COUNT(*) FROM automation_rules", [], |r| r.get(0))
                .unwrap();
            assert_eq!(r_count, 2);

            let p_count: i64 = conn
                .query_row("SELECT COUNT(*) FROM automation_processed", [], |r| {
                    r.get(0)
                })
                .unwrap();
            assert_eq!(p_count, 1);

            let rule = conn
                .query_row(
                    "SELECT name, save_history, preset_id, watch_directory, recursive, enabled,
                            stage_auto_polish, stage_polish_preset_id, stage_auto_translate,
                            stage_translation_language, stage_export_enabled,
                            export_directory, export_format, export_mode, export_prefix,
                            created_at, updated_at
                     FROM automation_rules WHERE id = 'rule-1'",
                    [],
                    |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, i64>(1)?,
                            r.get::<_, String>(2)?,
                            r.get::<_, String>(3)?,
                            r.get::<_, i64>(4)?,
                            r.get::<_, i64>(5)?,
                            r.get::<_, i64>(6)?,
                            r.get::<_, String>(7)?,
                            r.get::<_, i64>(8)?,
                            r.get::<_, String>(9)?,
                            r.get::<_, i64>(10)?,
                            r.get::<_, String>(11)?,
                            r.get::<_, String>(12)?,
                            r.get::<_, String>(13)?,
                            r.get::<_, String>(14)?,
                            r.get::<_, i64>(15)?,
                            r.get::<_, i64>(16)?,
                        ))
                    },
                )
                .unwrap();
            assert_eq!(rule.0, "Watch Docs");
            assert_eq!(rule.1, 1);
            assert_eq!(rule.2, "preset-1");
            assert_eq!(rule.3, "/docs");
            assert_eq!(rule.4, 1);
            assert_eq!(rule.5, 1);
            assert_eq!(rule.6, 0);
            assert_eq!(rule.7, "formal");
            assert_eq!(rule.8, 0);
            assert_eq!(rule.9, "ja");
            assert_eq!(rule.10, 1);
            assert_eq!(rule.11, "/exports");
            assert_eq!(rule.12, "md");
            assert_eq!(rule.13, "translated");
            assert_eq!(rule.14, "auto-");
            assert_eq!(rule.15, 1000);
            assert_eq!(rule.16, 2000);

            let processed = conn
                .query_row(
                    "SELECT rule_id, file_path, source_fingerprint, size, mtime_ms, status,
                            processed_at, history_id, export_path, error_message
                     FROM automation_processed WHERE id = 'proc-1'",
                    [],
                    |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, String>(2)?,
                            r.get::<_, i64>(3)?,
                            r.get::<_, i64>(4)?,
                            r.get::<_, String>(5)?,
                            r.get::<_, i64>(6)?,
                            r.get::<_, Option<String>>(7)?,
                            r.get::<_, Option<String>>(8)?,
                            r.get::<_, Option<String>>(9)?,
                        ))
                    },
                )
                .unwrap();
            assert_eq!(processed.0, "rule-1");
            assert_eq!(processed.1, "/docs/file.txt");
            assert_eq!(processed.2, "fp-1");
            assert_eq!(processed.3, 123);
            assert_eq!(processed.4, 456);
            assert_eq!(processed.5, "failed");
            assert_eq!(processed.6, 789);
            assert_eq!(processed.7.as_deref(), Some("hist-1"));
            assert_eq!(processed.8.as_deref(), Some("/exports/file.md"));
            assert_eq!(processed.9.as_deref(), Some("boom"));

            Ok(())
        })
        .unwrap();
    }

    // ---- test_migrate_task_ledger ----

    #[test]
    fn test_migrate_task_ledger() {
        let dir = tempfile::tempdir().unwrap();
        let db = with_open_db();

        write_json(
            &dir.path().join("task-ledger").join("tasks.json"),
            &json!({
                "version": 1,
                "updatedAt": 5000,
                "tasks": [
                    {"id": "task-1", "kind": "llmPolish", "status": "pending", "title": "Polish task", "progress": 25.0, "createdAt": 1000, "updatedAt": 1000, "retryable": true, "cancelable": true, "recoverable": false, "stage": "polish", "historyId": "hist-1", "projectId": "proj-1", "filePath": "/docs/file.txt", "automationRuleId": "rule-1", "sourceFingerprint": "fp-1", "errorMessage": "retryable", "templateId": "tmpl-1", "targetLanguage": "ja"},
                    {"id": "task-2", "kind": "llmTranslate", "status": "succeeded", "title": "Translate task", "progress": 100.0, "createdAt": 2000, "updatedAt": 3000, "retryable": false, "cancelable": false, "recoverable": false}
                ]
            }),
        );
        write_json(&dir.path().join("history").join("index.json"), &json!([]));

        let report = migrate_legacy_to_sqlite(&db, dir.path()).unwrap();
        assert!(report.migrated);
        assert!(report.errors.is_empty(), "Errors: {:?}", report.errors);

        db.with_connection(|conn| {
            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM task_ledger", [], |r| r.get(0))
                .unwrap();
            assert_eq!(count, 2);

            let task = conn
                .query_row(
                    "SELECT kind, status, title, progress, created_at, updated_at,
                            retryable, cancelable, recoverable, stage, history_id, tag_ids,
                            file_path, automation_rule_id, source_fingerprint, error_message,
                            template_id, target_language, version
                     FROM task_ledger WHERE id = 'task-1'",
                    [],
                    |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, String>(2)?,
                            r.get::<_, f64>(3)?,
                            r.get::<_, i64>(4)?,
                            r.get::<_, i64>(5)?,
                            r.get::<_, i64>(6)?,
                            r.get::<_, i64>(7)?,
                            r.get::<_, i64>(8)?,
                            r.get::<_, Option<String>>(9)?,
                            r.get::<_, Option<String>>(10)?,
                            r.get::<_, String>(11)?,
                            r.get::<_, Option<String>>(12)?,
                            r.get::<_, Option<String>>(13)?,
                            r.get::<_, Option<String>>(14)?,
                            r.get::<_, Option<String>>(15)?,
                            r.get::<_, Option<String>>(16)?,
                            r.get::<_, Option<String>>(17)?,
                            r.get::<_, i64>(18)?,
                        ))
                    },
                )
                .unwrap();
            assert_eq!(task.0, "llmPolish");
            assert_eq!(task.1, "pending");
            assert_eq!(task.2, "Polish task");
            assert_eq!(task.3, 25.0);
            assert_eq!(task.4, 1000);
            assert_eq!(task.5, 1000);
            assert_eq!(task.6, 1);
            assert_eq!(task.7, 1);
            assert_eq!(task.8, 0);
            assert_eq!(task.9.as_deref(), Some("polish"));
            assert_eq!(task.10.as_deref(), Some("hist-1"));
            assert_eq!(task.11, r#"["proj-1"]"#);
            assert_eq!(task.12.as_deref(), Some("/docs/file.txt"));
            assert_eq!(task.13.as_deref(), Some("rule-1"));
            assert_eq!(task.14.as_deref(), Some("fp-1"));
            assert_eq!(task.15.as_deref(), Some("retryable"));
            assert_eq!(task.16.as_deref(), Some("tmpl-1"));
            assert_eq!(task.17.as_deref(), Some("ja"));
            assert_eq!(task.18, TASK_LEDGER_VERSION as i64);

            Ok(())
        })
        .unwrap();
    }

    // ---- test_migrate_llm_usage ----

    #[test]
    fn test_migrate_llm_usage() {
        let dir = tempfile::tempdir().unwrap();
        let db = with_open_db();

        write_json(
            &dir.path().join("analytics").join("llm-usage.json"),
            &json!({
                "schemaVersion": 1,
                "startedAt": "2026-01-01T00:00:00Z",
                "lastUpdatedAt": "2026-06-01T00:00:00Z",
                "totals": {
                    "callCount": 10,
                    "callsWithUsage": 8,
                    "callsWithoutUsage": 2,
                    "promptTokens": 1000,
                    "completionTokens": 500,
                    "totalTokens": 1500
                },
                "byProvider": {
                    "open_ai": {
                        "callCount": 6,
                        "callsWithUsage": 5,
                        "promptTokens": 800,
                        "completionTokens": 400,
                        "totalTokens": 1200
                    },
                    "google_translate_free": {
                        "callCount": 4,
                        "callsWithUsage": 3,
                        "promptTokens": 200,
                        "completionTokens": 100,
                        "totalTokens": 300
                    }
                },
                "byCategory": {},
                "daily": {}
            }),
        );
        write_json(&dir.path().join("history").join("index.json"), &json!([]));

        let report = migrate_legacy_to_sqlite(&db, dir.path()).unwrap();
        assert!(report.migrated);
        assert!(report.errors.is_empty(), "Errors: {:?}", report.errors);

        db.with_connection(|conn| {
            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM analytics.llm_usage", [], |r| r.get(0))
                .unwrap();
            assert_eq!(count, 2);

            Ok(())
        })
        .unwrap();
    }

    // ---- test_migrate_non_history_domains_without_history_index ----

    #[test]
    fn test_migrate_non_history_domains_without_history_index() {
        let dir = tempfile::tempdir().unwrap();
        let db = with_open_db();

        write_json(
            &dir.path().join("projects").join("index.json"),
            &json!([{"id": "proj-1", "name": "Work", "createdAt": 1000, "updatedAt": 2000, "defaults": {}}]),
        );
        write_json(
            &dir.path().join("automation").join("rules.json"),
            &json!([{"id": "rule-1", "name": "Watch", "watchDirectory": "/watch", "projectId": "proj-1"}]),
        );
        write_json(
            &dir.path().join("automation").join("processed.json"),
            &json!([{"id": "proc-1", "filePath": "/f.txt"}]),
        );
        write_json(
            &dir.path().join("task-ledger").join("tasks.json"),
            &json!({"version": 1, "updatedAt": 1000, "tasks": [{"id": "task-1", "kind": "llmPolish", "status": "pending", "title": "Task", "progress": 0.0, "createdAt": 1000, "updatedAt": 1000, "retryable": false, "cancelable": true, "recoverable": false}]}),
        );
        write_json(
            &dir.path().join("analytics").join("llm-usage.json"),
            &json!({"schemaVersion": 1, "startedAt": "2026-01-01T00:00:00Z", "totals": {"callCount": 5, "promptTokens": 100, "completionTokens": 50, "totalTokens": 150}, "byProvider": {"test_provider": {"callCount": 5, "promptTokens": 100, "completionTokens": 50, "totalTokens": 150}}, "byCategory": {}, "daily": {}}),
        );

        let report = migrate_legacy_to_sqlite(&db, dir.path()).unwrap();
        assert!(report.migrated);
        assert_eq!(
            report.domains,
            LegacyMigrationDomains {
                history: false,
                projects: true,
                automation: true,
                task_ledger: true,
                analytics: true,
            }
        );
        assert_eq!(report.history_count, 0);
        assert_eq!(report.project_count, 1);
        assert!(report.errors.is_empty(), "Errors: {:?}", report.errors);

        db.with_connection(|conn| {
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM history_items", [], |r| r
                    .get::<_, i64>(0))
                    .unwrap(),
                0
            );
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM tags", [], |r| r.get::<_, i64>(0))
                    .unwrap(),
                1
            );
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM automation_rules", [], |r| r
                    .get::<_, i64>(0))
                    .unwrap(),
                2
            );
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM automation_processed", [], |r| r
                    .get::<_, i64>(0))
                    .unwrap(),
                1
            );
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM task_ledger", [], |r| r
                    .get::<_, i64>(0))
                    .unwrap(),
                1
            );
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM analytics.llm_usage", [], |r| r
                    .get::<_, i64>(0))
                    .unwrap(),
                1
            );
            Ok(())
        })
        .unwrap();
    }

    // ---- test_migrate_all ----

    #[test]
    fn test_migrate_all() {
        let dir = tempfile::tempdir().unwrap();
        let db = with_open_db();

        // History
        write_json(
            &dir.path().join("history").join("index.json"),
            &json!([{"id": "hist-1", "timestamp": 1000, "duration": 5.0, "audioPath": "hist-1.wav", "transcriptPath": "hist-1.json", "title": "Test", "previewText": "", "type": "recording", "searchContent": "", "status": "complete"}]),
        );
        write_json(
            &dir.path().join("history").join("hist-1.json"),
            &json!([{"id": "seg-1", "text": "Hello", "start": 0.0, "end": 1.0, "isFinal": true}]),
        );

        // Projects
        write_json(
            &dir.path().join("projects").join("index.json"),
            &json!([{"id": "proj-1", "name": "Work", "icon": "folder", "createdAt": 1000, "updatedAt": 2000, "defaults": {}}]),
        );

        // Automation
        write_json(
            &dir.path().join("automation").join("rules.json"),
            &json!([{"id": "rule-1", "name": "Watch", "watchDirectory": "/watch", "projectId": "proj-1"}]),
        );
        write_json(
            &dir.path().join("automation").join("processed.json"),
            &json!([{"id": "proc-1", "filePath": "/f.txt"}]),
        );

        // Task ledger
        write_json(
            &dir.path().join("task-ledger").join("tasks.json"),
            &json!({"version": 1, "updatedAt": 1000, "tasks": [{"id": "task-1", "kind": "llmPolish", "status": "pending", "title": "Task", "progress": 0.0, "createdAt": 1000, "updatedAt": 1000, "retryable": false, "cancelable": true, "recoverable": false}]}),
        );

        // LLM Usage
        write_json(
            &dir.path().join("analytics").join("llm-usage.json"),
            &json!({"schemaVersion": 1, "startedAt": "2026-01-01T00:00:00Z", "totals": {"callCount": 5, "promptTokens": 100, "completionTokens": 50, "totalTokens": 150}, "byProvider": {"test_provider": {"callCount": 5, "promptTokens": 100, "completionTokens": 50, "totalTokens": 150}}, "byCategory": {}, "daily": {}}),
        );

        let report = migrate_legacy_to_sqlite(&db, dir.path()).unwrap();
        assert!(report.migrated);
        assert_eq!(report.history_count, 1);
        assert_eq!(report.project_count, 1);
        assert!(report.errors.is_empty(), "Errors: {:?}", report.errors);

        // Verify all tables have data
        db.with_connection(|conn| {
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM history_items", [], |r| r
                    .get::<_, i64>(0))
                    .unwrap(),
                1
            );
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM tags", [], |r| r.get::<_, i64>(0))
                    .unwrap(),
                1
            );
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM automation_rules", [], |r| r
                    .get::<_, i64>(0))
                    .unwrap(),
                2
            );
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM automation_processed", [], |r| r
                    .get::<_, i64>(0))
                    .unwrap(),
                1
            );
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM task_ledger", [], |r| r
                    .get::<_, i64>(0))
                    .unwrap(),
                1
            );
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM analytics.llm_usage", [], |r| r
                    .get::<_, i64>(0))
                    .unwrap(),
                1
            );
            Ok(())
        })
        .unwrap();
    }

    // ---- test_move_to_backup ----

    #[test]
    fn test_move_to_backup() {
        let dir = tempfile::tempdir().unwrap();
        let db = with_open_db();

        // Create legacy files
        write_json(
            &dir.path().join("history").join("index.json"),
            &json!([{"id": "hist-1", "timestamp": 1000, "duration": 1.0, "audioPath": "", "transcriptPath": "", "title": "", "previewText": "", "type": "recording", "searchContent": "", "status": "complete"}]),
        );
        write_json(
            &dir.path().join("projects").join("index.json"),
            &json!([{"id": "proj-1", "name": "P", "createdAt": 1000, "updatedAt": 2000, "defaults": {}}]),
        );
        fs::create_dir_all(dir.path().join("automation")).unwrap();
        fs::create_dir_all(dir.path().join("task-ledger")).unwrap();
        fs::create_dir_all(dir.path().join("analytics")).unwrap();

        // Migrate first
        let report = migrate_legacy_to_sqlite(&db, dir.path()).unwrap();
        assert!(report.migrated);
        assert!(report.errors.is_empty(), "Errors: {:?}", report.errors);

        // Now move to backup
        move_legacy_to_backup(dir.path()).unwrap();

        // Verify originals are gone
        assert!(!dir.path().join("history").exists());
        assert!(!dir.path().join("projects").exists());
        assert!(!dir.path().join("automation").exists());
        assert!(!dir.path().join("task-ledger").exists());
        assert!(!dir.path().join("analytics").exists());

        // Verify backup exists
        let backup_dir = dir.path().join(".legacy-backup");
        assert!(backup_dir.join("history").exists());
        assert!(backup_dir.join("projects").exists());
        assert!(backup_dir.join("automation").exists());
        assert!(backup_dir.join("task-ledger").exists());
        assert!(backup_dir.join("analytics").exists());

        // Verify data still in DB
        db.with_connection(|conn| {
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM history_items", [], |r| r
                    .get::<_, i64>(0))
                    .unwrap(),
                1
            );
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM tags", [], |r| r.get::<_, i64>(0))
                    .unwrap(),
                1
            );
            Ok(())
        })
        .unwrap();
    }

    // ---- test_migrate_history_rejects_items_wrapped_in_object ----

    #[test]
    fn test_migrate_history_rejects_items_wrapped_in_object() {
        let dir = tempfile::tempdir().unwrap();
        let db = with_open_db();

        write_json(
            &dir.path().join("history").join("index.json"),
            &json!({
                "items": [
                    {"id": "hist-1", "timestamp": 1000, "duration": 1.0, "audioPath": "", "transcriptPath": "", "title": "Wrapped", "previewText": "", "type": "recording", "searchContent": "", "status": "complete"}
                ]
            }),
        );

        let report = migrate_legacy_to_sqlite(&db, dir.path()).unwrap();
        assert!(report.migrated);
        assert_eq!(report.history_count, 0);
        assert_eq!(
            report.errors,
            vec!["Unexpected format in history/index.json".to_string()]
        );
    }

    #[test]
    fn test_migrate_history_ignores_pre_v0_7_4_nested_transcript_path() {
        let dir = tempfile::tempdir().unwrap();
        let db = with_open_db();

        write_json(
            &dir.path().join("history").join("index.json"),
            &json!([
                {"id": "hist-1", "timestamp": 1000, "duration": 1.0, "audioPath": "hist-1.wav", "transcriptPath": "", "title": "Nested", "previewText": "", "type": "recording", "searchContent": "", "status": "complete"}
            ]),
        );
        write_json(
            &dir.path()
                .join("history")
                .join("hist-1")
                .join("transcript.json"),
            &json!([{"id": "seg-1", "text": "Nested", "start": 0.0, "end": 1.0, "isFinal": true}]),
        );

        let report = migrate_legacy_to_sqlite(&db, dir.path()).unwrap();
        assert!(report.migrated);
        assert_eq!(report.history_count, 1);
        assert!(report.errors.is_empty(), "Errors: {:?}", report.errors);

        db.with_connection(|conn| {
            let transcript_count: i64 =
                conn.query_row("SELECT COUNT(*) FROM history_transcripts", [], |r| r.get(0))?;
            assert_eq!(transcript_count, 0);
            Ok(())
        })
        .unwrap();
    }

    // ---- test_migrate_errors_do_not_abort ----

    #[test]
    fn test_migrate_partial_errors() {
        let dir = tempfile::tempdir().unwrap();
        let db = with_open_db();

        // Valid item plus an item that references a missing transcript - should not abort
        write_json(
            &dir.path().join("history").join("index.json"),
            &json!([
                {"id": "good-1", "timestamp": 1000, "duration": 1.0, "audioPath": "good-1.wav", "transcriptPath": "good-1.json", "title": "Good", "previewText": "", "type": "recording", "searchContent": "", "status": "complete"},
                {"id": "bad-1", "timestamp": 2000, "duration": 2.0, "audioPath": "bad-1.wav", "transcriptPath": "nonexistent.json", "title": "Bad", "previewText": "", "type": "recording", "searchContent": "", "status": "complete"}
            ]),
        );
        write_json(
            &dir.path().join("history").join("good-1.json"),
            &json!([{"id": "seg-1", "text": "Hello", "start": 0.0, "end": 1.0, "isFinal": true}]),
        );

        let report = migrate_legacy_to_sqlite(&db, dir.path()).unwrap();
        assert!(report.migrated);
        assert_eq!(report.history_count, 2);

        // Both items should be in DB, but only good-1 has a transcript
        db.with_connection(|conn| {
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM history_items", [], |r| r
                    .get::<_, i64>(0))
                    .unwrap(),
                2
            );
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM history_transcripts", [], |r| r
                    .get::<_, i64>(0))
                    .unwrap(),
                1
            );
            Ok(())
        })
        .unwrap();
    }

    // ---- test_move_to_backup_when_no_legacy_files ----

    #[test]
    fn test_move_to_backup_no_files() {
        let dir = tempfile::tempdir().unwrap();
        // No legacy files exist
        move_legacy_to_backup(dir.path()).unwrap();
        assert!(!dir.path().join(".legacy-backup").join("history").exists());
    }

    #[test]
    fn test_move_to_backup_only_detected_domains() {
        let dir = tempfile::tempdir().unwrap();

        write_json(
            &dir.path().join("projects").join("index.json"),
            &json!([{"id": "proj-1", "name": "P", "createdAt": 1000, "updatedAt": 2000, "defaults": {}}]),
        );
        write_json(
            &dir.path().join("history").join("orphan.json"),
            &json!({"not": "migrated without an index"}),
        );

        move_legacy_domains_to_backup(
            dir.path(),
            LegacyMigrationDomains {
                projects: true,
                ..Default::default()
            },
        )
        .unwrap();

        assert!(dir.path().join("history").exists());
        assert!(!dir.path().join("projects").exists());

        let backup_dir = dir.path().join(".legacy-backup");
        assert!(!backup_dir.join("history").exists());
        assert!(backup_dir.join("projects").exists());
    }

    // ---- test_idempotent ----

    #[test]
    fn test_idempotent_no_history_index() {
        let dir = tempfile::tempdir().unwrap();
        let db = with_open_db();
        // No history/index.json at all
        let report = migrate_legacy_to_sqlite(&db, dir.path()).unwrap();
        assert!(!report.migrated);
    }
}
