use crate::DatabaseError;
use crate::ports::Database as DatabasePort;
use rusqlite::{Connection, Transaction};
use serde_json::{Map, Value, json};
use sona_core::config::defaults::CURRENT_CONFIG_VERSION;
use std::collections::HashSet;
use std::sync::Arc;

#[derive(Clone)]
pub struct SqliteConfigStore<D = crate::Database>
where
    D: DatabasePort,
{
    db: Arc<D>,
}

crate::impl_db_repository!(SqliteConfigStore);

impl<D> SqliteConfigStore<D>
where
    D: DatabasePort,
{
    pub fn load_config(&self) -> Result<Option<Value>, DatabaseError> {
        self.get_db()?.with_connection(|conn| {
            let mut stmt = conn.prepare_cached("SELECT config FROM app_config WHERE id = 1")?;
            let mut rows = stmt.query([])?;
            if let Some(row) = rows.next()? {
                let config_str: String = row.get(0)?;
                let config: Value = serde_json::from_str(&config_str)?;
                let library_config = load_library_config(conn)?;
                Ok(Some(inject_library_config(config, library_config)))
            } else {
                Ok(None)
            }
        })
    }

    pub fn save_config(&self, config: &Value) -> Result<(), DatabaseError> {
        let projection = AppConfigStartupProjection::from_config(config);
        let config_version = config_version_from_config(config);
        let updated_at = now_ms();
        let (base_config, library_config) = extract_library_config(config);
        let config_str = serde_json::to_string(&base_config)?;

        self.get_db()?.with_rw_transaction(|tx| {
            tx.execute(
                "INSERT INTO app_config (
                    id, config, config_version, updated_at, http_server_enabled,
                    http_server_host, http_server_port, http_server_api_key,
                    http_server_max_concurrent, http_server_max_queue_size,
                    http_server_max_upload_size_mb, http_server_job_ttl_minutes,
                    http_server_max_streaming, http_server_ip_whitelist,
                    gpu_acceleration
                )
                VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
                ON CONFLICT(id) DO UPDATE SET
                    config = excluded.config,
                    config_version = excluded.config_version,
                    updated_at = excluded.updated_at,
                    http_server_enabled = excluded.http_server_enabled,
                    http_server_host = excluded.http_server_host,
                    http_server_port = excluded.http_server_port,
                    http_server_api_key = excluded.http_server_api_key,
                    http_server_max_concurrent = excluded.http_server_max_concurrent,
                    http_server_max_queue_size = excluded.http_server_max_queue_size,
                    http_server_max_upload_size_mb = excluded.http_server_max_upload_size_mb,
                    http_server_job_ttl_minutes = excluded.http_server_job_ttl_minutes,
                    http_server_max_streaming = excluded.http_server_max_streaming,
                    http_server_ip_whitelist = excluded.http_server_ip_whitelist,
                    gpu_acceleration = excluded.gpu_acceleration",
                rusqlite::params![
                    config_str,
                    config_version,
                    updated_at,
                    projection.http_server_enabled as i64,
                    projection.host,
                    projection.port,
                    projection.api_key,
                    projection.max_concurrent,
                    projection.max_queue_size,
                    projection.max_upload_size_mb,
                    projection.job_ttl_minutes,
                    projection.max_streaming,
                    projection.ip_whitelist,
                    projection.gpu_acceleration,
                ],
            )?;
            save_library_config(tx, &library_config, updated_at)?;
            Ok(())
        })
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<Value>, DatabaseError> {
        self.get_db()?.with_connection(|conn| {
            let mut stmt = conn.prepare_cached("SELECT value FROM app_settings WHERE key = ?1")?;
            let mut rows = stmt.query([key])?;
            if let Some(row) = rows.next()? {
                let value_str: String = row.get(0)?;
                let value: Value = serde_json::from_str(&value_str)?;
                Ok(Some(value))
            } else {
                Ok(None)
            }
        })
    }

    pub fn set_setting(&self, key: &str, value: &Value) -> Result<(), DatabaseError> {
        let value_str = serde_json::to_string(value)?;
        self.get_db()?.with_write_connection(|conn| {
            conn.execute(
                "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                rusqlite::params![key, value_str],
            )?;
            Ok(())
        })
    }
}

const SUMMARY_TEMPLATES_KEY: &str = "summaryCustomTemplates";
const POLISH_PRESETS_KEY: &str = "polishCustomPresets";
const TEXT_REPLACEMENT_SETS_KEY: &str = "textReplacementSets";
const HOTWORD_SETS_KEY: &str = "hotwordSets";
const POLISH_KEYWORD_SETS_KEY: &str = "polishKeywordSets";
const SPEAKER_PROFILES_KEY: &str = "speakerProfiles";

const VOCABULARY_KIND_TEXT_REPLACEMENT: &str = "text_replacement";
const VOCABULARY_KIND_HOTWORD: &str = "hotword";
const VOCABULARY_KIND_POLISH_KEYWORD: &str = "polish_keyword";

#[derive(Default)]
struct LibraryConfig {
    summary_templates: Vec<Value>,
    polish_presets: Vec<Value>,
    text_replacement_sets: Vec<Value>,
    hotword_sets: Vec<Value>,
    polish_keyword_sets: Vec<Value>,
    speaker_profiles: Vec<Value>,
}

struct VocabularySetRow {
    id: String,
    name: String,
    enabled: bool,
    ignore_case: bool,
    keywords: String,
}

fn extract_library_config(config: &Value) -> (Value, LibraryConfig) {
    let mut base_config = config.clone();
    let payload = app_config_payload_mut(&mut base_config);
    let mut library_config = LibraryConfig {
        summary_templates: take_array_field(payload, SUMMARY_TEMPLATES_KEY),
        polish_presets: take_array_field(payload, POLISH_PRESETS_KEY),
        text_replacement_sets: take_array_field(payload, TEXT_REPLACEMENT_SETS_KEY),
        hotword_sets: take_array_field(payload, HOTWORD_SETS_KEY),
        polish_keyword_sets: take_array_field(payload, POLISH_KEYWORD_SETS_KEY),
        speaker_profiles: take_array_field(payload, SPEAKER_PROFILES_KEY),
    };
    repair_library_config_ids(&mut library_config);
    (base_config, library_config)
}

fn inject_library_config(mut config: Value, library_config: LibraryConfig) -> Value {
    let payload = app_config_payload_mut(&mut config);
    let payload = ensure_object(payload);
    payload.insert(
        SUMMARY_TEMPLATES_KEY.to_string(),
        Value::Array(library_config.summary_templates),
    );
    payload.insert(
        POLISH_PRESETS_KEY.to_string(),
        Value::Array(library_config.polish_presets),
    );
    payload.insert(
        TEXT_REPLACEMENT_SETS_KEY.to_string(),
        Value::Array(library_config.text_replacement_sets),
    );
    payload.insert(
        HOTWORD_SETS_KEY.to_string(),
        Value::Array(library_config.hotword_sets),
    );
    payload.insert(
        POLISH_KEYWORD_SETS_KEY.to_string(),
        Value::Array(library_config.polish_keyword_sets),
    );
    payload.insert(
        SPEAKER_PROFILES_KEY.to_string(),
        Value::Array(library_config.speaker_profiles),
    );
    config
}

fn load_library_config(conn: &Connection) -> Result<LibraryConfig, DatabaseError> {
    Ok(LibraryConfig {
        summary_templates: load_summary_templates(conn)?,
        polish_presets: load_polish_presets(conn)?,
        text_replacement_sets: load_vocabulary_sets(conn, VOCABULARY_KIND_TEXT_REPLACEMENT)?,
        hotword_sets: load_vocabulary_sets(conn, VOCABULARY_KIND_HOTWORD)?,
        polish_keyword_sets: load_vocabulary_sets(conn, VOCABULARY_KIND_POLISH_KEYWORD)?,
        speaker_profiles: load_speaker_profiles(conn)?,
    })
}

fn save_library_config(
    tx: &Transaction,
    library_config: &LibraryConfig,
    updated_at: i64,
) -> Result<(), DatabaseError> {
    tx.execute("DELETE FROM vocabulary_rules", [])?;
    tx.execute("DELETE FROM vocabulary_sets", [])?;
    tx.execute("DELETE FROM speaker_profile_samples", [])?;
    tx.execute("DELETE FROM speaker_profiles", [])?;
    tx.execute("DELETE FROM summary_templates", [])?;
    tx.execute("DELETE FROM polish_presets", [])?;

    save_summary_templates(tx, &library_config.summary_templates, updated_at)?;
    save_polish_presets(tx, &library_config.polish_presets, updated_at)?;
    save_vocabulary_sets(
        tx,
        VOCABULARY_KIND_TEXT_REPLACEMENT,
        &library_config.text_replacement_sets,
        updated_at,
    )?;
    save_vocabulary_sets(
        tx,
        VOCABULARY_KIND_HOTWORD,
        &library_config.hotword_sets,
        updated_at,
    )?;
    save_vocabulary_sets(
        tx,
        VOCABULARY_KIND_POLISH_KEYWORD,
        &library_config.polish_keyword_sets,
        updated_at,
    )?;
    save_speaker_profiles(tx, &library_config.speaker_profiles, updated_at)?;
    Ok(())
}

fn load_summary_templates(conn: &Connection) -> Result<Vec<Value>, DatabaseError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, name, instructions
         FROM summary_templates
         ORDER BY sort_order, id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(json!({
            "id": row.get::<_, String>("id")?,
            "name": row.get::<_, String>("name")?,
            "instructions": row.get::<_, String>("instructions")?,
        }))
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(DatabaseError::QueryError)
}

fn save_summary_templates(
    tx: &Transaction,
    templates: &[Value],
    updated_at: i64,
) -> Result<(), DatabaseError> {
    let mut stmt = tx.prepare_cached(
        "INSERT INTO summary_templates (
            id, name, instructions, sort_order, created_at, updated_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    for (sort_order, template) in templates.iter().enumerate() {
        stmt.execute(rusqlite::params![
            string_field(template, "id", ""),
            string_field(template, "name", ""),
            string_field(template, "instructions", ""),
            sort_order as i64,
            updated_at,
            updated_at,
        ])?;
    }
    Ok(())
}

fn load_polish_presets(conn: &Connection) -> Result<Vec<Value>, DatabaseError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, name, context
         FROM polish_presets
         ORDER BY sort_order, id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(json!({
            "id": row.get::<_, String>("id")?,
            "name": row.get::<_, String>("name")?,
            "context": row.get::<_, String>("context")?,
        }))
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(DatabaseError::QueryError)
}

fn save_polish_presets(
    tx: &Transaction,
    presets: &[Value],
    updated_at: i64,
) -> Result<(), DatabaseError> {
    let mut stmt = tx.prepare_cached(
        "INSERT INTO polish_presets (
            id, name, context, sort_order, created_at, updated_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    for (sort_order, preset) in presets.iter().enumerate() {
        stmt.execute(rusqlite::params![
            string_field(preset, "id", ""),
            string_field(preset, "name", ""),
            string_field(preset, "context", ""),
            sort_order as i64,
            updated_at,
            updated_at,
        ])?;
    }
    Ok(())
}

fn load_vocabulary_sets(conn: &Connection, kind: &str) -> Result<Vec<Value>, DatabaseError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, name, enabled, ignore_case, keywords
         FROM vocabulary_sets
         WHERE kind = ?1
         ORDER BY sort_order, id",
    )?;
    let rows = stmt.query_map([kind], |row| {
        Ok(VocabularySetRow {
            id: row.get("id")?,
            name: row.get("name")?,
            enabled: row.get::<_, i64>("enabled")? != 0,
            ignore_case: row.get::<_, i64>("ignore_case")? != 0,
            keywords: row.get("keywords")?,
        })
    })?;
    let rows = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(DatabaseError::QueryError)?;

    rows.into_iter()
        .map(|row| match kind {
            VOCABULARY_KIND_TEXT_REPLACEMENT => Ok(json!({
                "id": row.id,
                "name": row.name,
                "enabled": row.enabled,
                "ignoreCase": row.ignore_case,
                "rules": load_text_replacement_rules(conn, kind, &row.id)?,
            })),
            VOCABULARY_KIND_HOTWORD => Ok(json!({
                "id": row.id,
                "name": row.name,
                "enabled": row.enabled,
                "rules": load_hotword_rules(conn, kind, &row.id)?,
            })),
            VOCABULARY_KIND_POLISH_KEYWORD => Ok(json!({
                "id": row.id,
                "name": row.name,
                "enabled": row.enabled,
                "keywords": row.keywords,
            })),
            _ => Ok(json!({})),
        })
        .collect()
}

fn save_vocabulary_sets(
    tx: &Transaction,
    kind: &str,
    sets: &[Value],
    updated_at: i64,
) -> Result<(), DatabaseError> {
    let mut set_stmt = tx.prepare_cached(
        "INSERT INTO vocabulary_sets (
            id, kind, name, enabled, ignore_case, keywords, sort_order, created_at, updated_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?;
    for (sort_order, set) in sets.iter().enumerate() {
        let id = string_field(set, "id", "");
        set_stmt.execute(rusqlite::params![
            id,
            kind,
            string_field(set, "name", ""),
            bool_field(set, "enabled", true) as i64,
            bool_field(set, "ignoreCase", false) as i64,
            string_field(set, "keywords", ""),
            sort_order as i64,
            updated_at,
            updated_at,
        ])?;

        match kind {
            VOCABULARY_KIND_TEXT_REPLACEMENT => {
                save_text_replacement_rules(tx, kind, &id, array_field(set, "rules"))?;
            }
            VOCABULARY_KIND_HOTWORD => {
                save_hotword_rules(tx, kind, &id, array_field(set, "rules"))?;
            }
            VOCABULARY_KIND_POLISH_KEYWORD => {}
            _ => {}
        }
    }
    Ok(())
}

fn load_text_replacement_rules(
    conn: &Connection,
    set_kind: &str,
    set_id: &str,
) -> Result<Vec<Value>, DatabaseError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, from_text, to_text
         FROM vocabulary_rules
         WHERE set_kind = ?1 AND set_id = ?2
         ORDER BY sort_order, id",
    )?;
    let rows = stmt.query_map(rusqlite::params![set_kind, set_id], |row| {
        Ok(json!({
            "id": row.get::<_, String>("id")?,
            "from": row.get::<_, String>("from_text")?,
            "to": row.get::<_, String>("to_text")?,
        }))
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(DatabaseError::QueryError)
}

fn save_text_replacement_rules(
    tx: &Transaction,
    set_kind: &str,
    set_id: &str,
    rules: Vec<Value>,
) -> Result<(), DatabaseError> {
    let mut stmt = tx.prepare_cached(
        "INSERT INTO vocabulary_rules (
            id, set_kind, set_id, from_text, to_text, text, sort_order
         )
         VALUES (?1, ?2, ?3, ?4, ?5, '', ?6)",
    )?;
    for (sort_order, rule) in rules.iter().enumerate() {
        stmt.execute(rusqlite::params![
            string_field(rule, "id", ""),
            set_kind,
            set_id,
            string_field(rule, "from", ""),
            string_field(rule, "to", ""),
            sort_order as i64,
        ])?;
    }
    Ok(())
}

fn load_hotword_rules(
    conn: &Connection,
    set_kind: &str,
    set_id: &str,
) -> Result<Vec<Value>, DatabaseError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, text
         FROM vocabulary_rules
         WHERE set_kind = ?1 AND set_id = ?2
         ORDER BY sort_order, id",
    )?;
    let rows = stmt.query_map(rusqlite::params![set_kind, set_id], |row| {
        Ok(json!({
            "id": row.get::<_, String>("id")?,
            "text": row.get::<_, String>("text")?,
        }))
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(DatabaseError::QueryError)
}

fn save_hotword_rules(
    tx: &Transaction,
    set_kind: &str,
    set_id: &str,
    rules: Vec<Value>,
) -> Result<(), DatabaseError> {
    let mut stmt = tx.prepare_cached(
        "INSERT INTO vocabulary_rules (
            id, set_kind, set_id, from_text, to_text, text, sort_order
         )
         VALUES (?1, ?2, ?3, '', '', ?4, ?5)",
    )?;
    for (sort_order, rule) in rules.iter().enumerate() {
        stmt.execute(rusqlite::params![
            string_field(rule, "id", ""),
            set_kind,
            set_id,
            string_field(rule, "text", ""),
            sort_order as i64,
        ])?;
    }
    Ok(())
}

fn load_speaker_profiles(conn: &Connection) -> Result<Vec<Value>, DatabaseError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, name, enabled
         FROM speaker_profiles
         ORDER BY sort_order, id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>("id")?,
            row.get::<_, String>("name")?,
            row.get::<_, i64>("enabled")? != 0,
        ))
    })?;
    let rows = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(DatabaseError::QueryError)?;

    rows.into_iter()
        .map(|(id, name, enabled)| {
            Ok(json!({
                "id": id,
                "name": name,
                "enabled": enabled,
                "samples": load_speaker_profile_samples(conn, &id)?,
            }))
        })
        .collect()
}

fn save_speaker_profiles(
    tx: &Transaction,
    profiles: &[Value],
    updated_at: i64,
) -> Result<(), DatabaseError> {
    let mut profile_stmt = tx.prepare_cached(
        "INSERT INTO speaker_profiles (
            id, name, enabled, sort_order, created_at, updated_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    for (sort_order, profile) in profiles.iter().enumerate() {
        let id = string_field(profile, "id", "");
        profile_stmt.execute(rusqlite::params![
            id,
            string_field(profile, "name", ""),
            bool_field(profile, "enabled", true) as i64,
            sort_order as i64,
            updated_at,
            updated_at,
        ])?;
        save_speaker_profile_samples(tx, &id, array_field(profile, "samples"))?;
    }
    Ok(())
}

fn load_speaker_profile_samples(
    conn: &Connection,
    profile_id: &str,
) -> Result<Vec<Value>, DatabaseError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, file_path, source_name, duration_seconds
         FROM speaker_profile_samples
         WHERE profile_id = ?1
         ORDER BY sort_order, id",
    )?;
    let rows = stmt.query_map([profile_id], |row| {
        Ok(json!({
            "id": row.get::<_, String>("id")?,
            "filePath": row.get::<_, String>("file_path")?,
            "sourceName": row.get::<_, String>("source_name")?,
            "durationSeconds": row.get::<_, f64>("duration_seconds")?,
        }))
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(DatabaseError::QueryError)
}

fn save_speaker_profile_samples(
    tx: &Transaction,
    profile_id: &str,
    samples: Vec<Value>,
) -> Result<(), DatabaseError> {
    let mut stmt = tx.prepare_cached(
        "INSERT INTO speaker_profile_samples (
            id, profile_id, file_path, source_name, duration_seconds, sort_order
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    for (sort_order, sample) in samples.iter().enumerate() {
        stmt.execute(rusqlite::params![
            string_field(sample, "id", ""),
            profile_id,
            string_field(sample, "filePath", ""),
            string_field(sample, "sourceName", ""),
            float_field(sample, "durationSeconds", 0.0),
            sort_order as i64,
        ])?;
    }
    Ok(())
}

struct AppConfigStartupProjection {
    http_server_enabled: bool,
    host: String,
    port: i64,
    api_key: String,
    max_concurrent: i64,
    max_queue_size: i64,
    max_upload_size_mb: i64,
    job_ttl_minutes: i64,
    max_streaming: i64,
    ip_whitelist: String,
    gpu_acceleration: String,
}

impl AppConfigStartupProjection {
    fn from_config(value: &Value) -> Self {
        let config = app_config_payload(value);
        Self {
            http_server_enabled: bool_field(config, "httpServerEnabled", false),
            host: string_field(config, "httpServerHost", "127.0.0.1"),
            port: integer_field(config, "httpServerPort", 14200),
            api_key: string_field(config, "httpServerApiKey", ""),
            max_concurrent: integer_field(config, "httpServerMaxConcurrent", 2),
            max_queue_size: integer_field(config, "httpServerMaxQueueSize", 100),
            max_upload_size_mb: integer_field(config, "httpServerMaxUploadSizeMB", 50),
            job_ttl_minutes: integer_field(config, "httpServerJobTtlMinutes", 60),
            max_streaming: integer_field(config, "httpServerMaxStreaming", 2),
            ip_whitelist: string_field(config, "httpServerIpWhitelist", "localhost"),
            gpu_acceleration: string_field(config, "gpuAcceleration", "auto"),
        }
    }
}

fn app_config_payload(value: &Value) -> &Value {
    value
        .get("sona-config")
        .filter(|value| value.is_object())
        .or_else(|| value.get("sona_config"))
        .filter(|value| value.is_object())
        .or_else(|| value.get("config"))
        .filter(|value| value.is_object())
        .unwrap_or(value)
}

fn app_config_payload_mut(value: &mut Value) -> &mut Value {
    let nested_key = if value.get("sona-config").is_some_and(Value::is_object) {
        Some("sona-config")
    } else if value.get("sona_config").is_some_and(Value::is_object) {
        Some("sona_config")
    } else if value.get("config").is_some_and(Value::is_object) {
        Some("config")
    } else {
        None
    };

    if let Some(key) = nested_key {
        value.get_mut(key).unwrap()
    } else {
        value
    }
}

fn ensure_object(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = Value::Object(Map::new());
    }
    value.as_object_mut().unwrap()
}

fn take_array_field(value: &mut Value, key: &str) -> Vec<Value> {
    value
        .as_object_mut()
        .and_then(|object| object.remove(key))
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default()
}

fn array_field(value: &Value, key: &str) -> Vec<Value> {
    value
        .get(key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn config_version_from_config(value: &Value) -> i64 {
    integer_field(
        app_config_payload(value),
        "configVersion",
        CURRENT_CONFIG_VERSION,
    )
}

fn repair_library_config_ids(library_config: &mut LibraryConfig) {
    ensure_unique_ids(&mut library_config.summary_templates, "summary-template");
    ensure_unique_ids(&mut library_config.polish_presets, "polish-preset");
    ensure_unique_ids(
        &mut library_config.text_replacement_sets,
        "text-replacement-set",
    );
    ensure_nested_unique_ids(
        &mut library_config.text_replacement_sets,
        "rules",
        "text-replacement-rule",
    );
    ensure_unique_ids(&mut library_config.hotword_sets, "hotword-set");
    ensure_nested_unique_ids(&mut library_config.hotword_sets, "rules", "hotword-rule");
    ensure_unique_ids(
        &mut library_config.polish_keyword_sets,
        "polish-keyword-set",
    );
    ensure_unique_ids(&mut library_config.speaker_profiles, "speaker-profile");
    ensure_nested_unique_ids(
        &mut library_config.speaker_profiles,
        "samples",
        "speaker-sample",
    );
}

fn ensure_unique_ids(values: &mut Vec<Value>, id_prefix: &str) {
    values.retain(Value::is_object);
    let mut seen = HashSet::new();
    for (index, value) in values.iter_mut().enumerate() {
        let current_id = value.get("id").and_then(non_empty_str).map(str::to_string);
        let next_id = if let Some(id) = current_id {
            if seen.insert(id.clone()) {
                id
            } else {
                unique_generated_id(id_prefix, value, index, &mut seen)
            }
        } else {
            unique_generated_id(id_prefix, value, index, &mut seen)
        };

        if value.get("id").and_then(Value::as_str) != Some(next_id.as_str())
            && let Some(object) = value.as_object_mut()
        {
            object.insert("id".to_string(), Value::String(next_id));
        }
    }
}

fn ensure_nested_unique_ids(values: &mut [Value], field: &str, id_prefix: &str) {
    for value in values {
        let Some(object) = value.as_object_mut() else {
            continue;
        };
        let mut nested = object
            .remove(field)
            .and_then(|value| value.as_array().cloned())
            .unwrap_or_default();
        ensure_unique_ids(&mut nested, id_prefix);
        object.insert(field.to_string(), Value::Array(nested));
    }
}

fn unique_generated_id(
    id_prefix: &str,
    value: &Value,
    index: usize,
    seen: &mut HashSet<String>,
) -> String {
    let serialized = serde_json::to_string(value).unwrap_or_default();
    let base = format!(
        "{id_prefix}-{}",
        hash_string(&format!("{serialized}-{index}"))
    );
    let mut candidate = base.clone();
    let mut suffix = 2;
    while !seen.insert(candidate.clone()) {
        candidate = format!("{base}-{suffix}");
        suffix += 1;
    }
    candidate
}

fn non_empty_str(value: &Value) -> Option<&str> {
    value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn hash_string(value: &str) -> String {
    let mut hash: u32 = 5381;
    for unit in value.encode_utf16() {
        hash = hash.wrapping_shl(5).wrapping_add(hash) ^ u32::from(unit);
    }
    format!("{hash:08x}")
}

fn bool_field(value: &Value, key: &str, default: bool) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(default)
}

fn string_field(value: &Value, key: &str, default: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_string()
}

fn integer_field(value: &Value, key: &str, default: i64) -> i64 {
    value
        .get(key)
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_u64().and_then(|n| i64::try_from(n).ok()))
                .or_else(|| value.as_f64().map(|n| n.round() as i64))
        })
        .unwrap_or(default)
}

fn float_field(value: &Value, key: &str, default: f64) -> f64 {
    value.get(key).and_then(Value::as_f64).unwrap_or(default)
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Database;
    use serde_json::json;
    use std::sync::Arc;

    const LIBRARY_KEYS: &[&str] = &[
        "summaryCustomTemplates",
        "polishCustomPresets",
        "textReplacementSets",
        "hotwordSets",
        "polishKeywordSets",
        "speakerProfiles",
    ];

    #[test]
    fn test_config_load_save() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let store = SqliteConfigStore::new(Arc::clone(&db));

        // Initially empty
        let config = store.load_config().unwrap();
        assert!(config.is_none());

        // Save
        let test_config = json!({"theme": "dark", "language": "en"});
        store.save_config(&test_config).unwrap();

        // Load
        let loaded = store.load_config().unwrap().unwrap();
        assert_eq!(loaded["theme"], "dark");
        assert_eq!(loaded["language"], "en");
    }

    #[test]
    fn test_config_overwrite() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let store = SqliteConfigStore::new(Arc::clone(&db));

        store.save_config(&json!({"version": 1})).unwrap();
        store.save_config(&json!({"version": 2})).unwrap();

        let loaded = store.load_config().unwrap().unwrap();
        assert_eq!(loaded["version"], 2);
    }

    #[test]
    fn test_save_config_projects_startup_columns() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let store = SqliteConfigStore::new(Arc::clone(&db));

        store
            .save_config(&json!({
                "httpServerEnabled": true,
                "httpServerHost": "0.0.0.0",
                "httpServerPort": 15555,
                "httpServerApiKey": "secret",
                "httpServerMaxConcurrent": 4,
                "httpServerMaxQueueSize": 32,
                "httpServerMaxUploadSizeMB": 128,
                "httpServerJobTtlMinutes": 15,
                "httpServerMaxStreaming": 6,
                "httpServerIpWhitelist": "127.0.0.1/32",
                "gpuAcceleration": "cpu",
                "asr": {
                    "providers": {
                        "online": {
                            "volcengine": {
                                "apiKey": "kept-in-json"
                            }
                        }
                    }
                }
            }))
            .unwrap();

        db.with_connection(|conn| {
            let projected = conn.query_row(
                "SELECT http_server_enabled, http_server_host, http_server_port,
                            http_server_api_key, http_server_max_concurrent,
                            http_server_max_queue_size, http_server_max_upload_size_mb,
                            http_server_job_ttl_minutes, http_server_max_streaming,
                            http_server_ip_whitelist, gpu_acceleration
                     FROM app_config WHERE id = 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, i64>(7)?,
                        row.get::<_, i64>(8)?,
                        row.get::<_, String>(9)?,
                        row.get::<_, String>(10)?,
                    ))
                },
            )?;

            assert_eq!(projected.0, 1);
            assert_eq!(projected.1, "0.0.0.0");
            assert_eq!(projected.2, 15555);
            assert_eq!(projected.3, "secret");
            assert_eq!(projected.4, 4);
            assert_eq!(projected.5, 32);
            assert_eq!(projected.6, 128);
            assert_eq!(projected.7, 15);
            assert_eq!(projected.8, 6);
            assert_eq!(projected.9, "127.0.0.1/32");
            assert_eq!(projected.10, "cpu");
            Ok(())
        })
        .unwrap();

        let loaded = store.load_config().unwrap().unwrap();
        assert_eq!(
            loaded["asr"]["providers"]["online"]["volcengine"]["apiKey"],
            "kept-in-json"
        );
    }

    #[test]
    fn test_save_config_splits_and_rehydrates_library_config() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let store = SqliteConfigStore::new(Arc::clone(&db));
        let original = json!({
            "configVersion": 7,
            "theme": "dark",
            "llmSettings": {
                "openai": {
                    "apiKey": "kept-in-json"
                }
            },
            "asr": {
                "providers": {
                    "online": {
                        "volcengine": {
                            "apiKey": "also-kept-in-json"
                        }
                    }
                }
            },
            "summaryCustomTemplates": [
                {
                    "id": "summary-a",
                    "name": "Summary A",
                    "instructions": "Summarize A"
                },
                {
                    "id": "summary-b",
                    "name": "Summary B",
                    "instructions": "Summarize B"
                }
            ],
            "polishCustomPresets": [
                {
                    "id": "polish-a",
                    "name": "Polish A",
                    "context": "Use a concise tone"
                }
            ],
            "textReplacementSets": [
                {
                    "id": "tr-a",
                    "name": "Replacement A",
                    "enabled": true,
                    "ignoreCase": true,
                    "rules": [
                        {
                            "id": "tr-rule-a",
                            "from": "foo",
                            "to": "bar"
                        },
                        {
                            "id": "tr-rule-b",
                            "from": "baz",
                            "to": "qux"
                        }
                    ]
                }
            ],
            "hotwordSets": [
                {
                    "id": "hotword-a",
                    "name": "Hotword A",
                    "enabled": false,
                    "rules": [
                        {
                            "id": "hotword-rule-a",
                            "text": "Sona"
                        }
                    ]
                }
            ],
            "polishKeywordSets": [
                {
                    "id": "keyword-a",
                    "name": "Keyword A",
                    "enabled": true,
                    "keywords": "clear, concise"
                }
            ],
            "speakerProfiles": [
                {
                    "id": "speaker-a",
                    "name": "Alice",
                    "enabled": true,
                    "samples": [
                        {
                            "id": "sample-a",
                            "filePath": "profiles/alice.wav",
                            "sourceName": "Alice sample",
                            "durationSeconds": 12.5
                        }
                    ]
                }
            ]
        });

        store.save_config(&original).unwrap();

        let raw = raw_app_config_json(&db);
        for key in LIBRARY_KEYS {
            assert!(
                raw.get(key).is_none(),
                "{key} should be stored outside app_config.config"
            );
        }
        assert_eq!(raw["llmSettings"]["openai"]["apiKey"], "kept-in-json");
        assert_eq!(
            raw["asr"]["providers"]["online"]["volcengine"]["apiKey"],
            "also-kept-in-json"
        );

        db.with_connection(|conn| {
            assert_eq!(count_rows(conn, "summary_templates")?, 2);
            assert_eq!(count_rows(conn, "polish_presets")?, 1);
            assert_eq!(count_rows(conn, "vocabulary_sets")?, 3);
            assert_eq!(count_rows(conn, "vocabulary_rules")?, 3);
            assert_eq!(count_rows(conn, "speaker_profiles")?, 1);
            assert_eq!(count_rows(conn, "speaker_profile_samples")?, 1);

            let version: i64 = conn.query_row(
                "SELECT config_version FROM app_config WHERE id = 1",
                [],
                |row| row.get(0),
            )?;
            assert_eq!(version, 7);

            let ordered_summary_ids = ordered_ids(conn, "summary_templates")?;
            assert_eq!(ordered_summary_ids, vec!["summary-a", "summary-b"]);
            Ok(())
        })
        .unwrap();

        let loaded = store.load_config().unwrap().unwrap();
        assert_eq!(loaded, original);
    }

    #[test]
    fn test_save_config_replaces_library_tables_on_overwrite() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let store = SqliteConfigStore::new(Arc::clone(&db));

        store
            .save_config(&json!({
                "summaryCustomTemplates": [
                    {"id": "summary-old", "name": "Old", "instructions": "old"}
                ],
                "polishCustomPresets": [
                    {"id": "polish-old", "name": "Old", "context": "old"}
                ],
                "textReplacementSets": [
                    {
                        "id": "tr-old",
                        "name": "Old",
                        "enabled": true,
                        "ignoreCase": false,
                        "rules": [{"id": "tr-rule-old", "from": "old", "to": "new"}]
                    }
                ],
                "hotwordSets": [
                    {
                        "id": "hotword-old",
                        "name": "Old",
                        "enabled": true,
                        "rules": [{"id": "hotword-rule-old", "text": "old"}]
                    }
                ],
                "polishKeywordSets": [
                    {"id": "keyword-old", "name": "Old", "enabled": true, "keywords": "old"}
                ],
                "speakerProfiles": [
                    {
                        "id": "speaker-old",
                        "name": "Old",
                        "enabled": true,
                        "samples": [
                            {
                                "id": "sample-old",
                                "filePath": "old.wav",
                                "sourceName": "Old",
                                "durationSeconds": 1.0
                            }
                        ]
                    }
                ]
            }))
            .unwrap();

        store
            .save_config(&json!({
                "theme": "light",
                "summaryCustomTemplates": [],
                "polishCustomPresets": [],
                "textReplacementSets": [],
                "hotwordSets": [],
                "polishKeywordSets": [],
                "speakerProfiles": []
            }))
            .unwrap();

        let loaded = store.load_config().unwrap().unwrap();
        assert_eq!(loaded["theme"], "light");
        for key in LIBRARY_KEYS {
            assert_eq!(
                loaded[key],
                json!([]),
                "{key} should be empty after overwrite"
            );
        }

        db.with_connection(|conn| {
            for table in [
                "summary_templates",
                "polish_presets",
                "vocabulary_sets",
                "vocabulary_rules",
                "speaker_profiles",
                "speaker_profile_samples",
            ] {
                assert_eq!(count_rows(conn, table)?, 0, "{table} should be empty");
            }
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_save_config_repairs_missing_and_duplicate_library_ids() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let store = SqliteConfigStore::new(Arc::clone(&db));

        store
            .save_config(&json!({
                "summaryCustomTemplates": [
                    {"name": "No ID", "instructions": "Keep me"},
                    {"id": "duplicate", "name": "First duplicate", "instructions": "First"},
                    {"id": "duplicate", "name": "Second duplicate", "instructions": "Second"}
                ],
                "polishCustomPresets": [
                    {"name": "Preset", "context": "Context"},
                    {"id": "duplicate", "name": "Preset A", "context": "A"},
                    {"id": "duplicate", "name": "Preset B", "context": "B"}
                ],
                "textReplacementSets": [
                    {
                        "name": "Replacement",
                        "enabled": true,
                        "ignoreCase": false,
                        "rules": [
                            {"from": "a", "to": "b"},
                            {"id": "same-rule", "from": "c", "to": "d"},
                            {"id": "same-rule", "from": "e", "to": "f"}
                        ]
                    }
                ],
                "hotwordSets": [
                    {
                        "id": "hotword-set",
                        "name": "Hotwords",
                        "enabled": true,
                        "rules": [
                            {"text": "alpha"},
                            {"id": "same-rule", "text": "beta"},
                            {"id": "same-rule", "text": "gamma"}
                        ]
                    }
                ],
                "polishKeywordSets": [
                    {"name": "Keywords", "enabled": true, "keywords": "one"},
                    {"id": "duplicate", "name": "Keywords A", "enabled": true, "keywords": "two"},
                    {"id": "duplicate", "name": "Keywords B", "enabled": true, "keywords": "three"}
                ],
                "speakerProfiles": [
                    {
                        "name": "Speaker",
                        "enabled": true,
                        "samples": [
                            {"filePath": "a.wav", "sourceName": "A", "durationSeconds": 1.0},
                            {"id": "same-sample", "filePath": "b.wav", "sourceName": "B", "durationSeconds": 2.0},
                            {"id": "same-sample", "filePath": "c.wav", "sourceName": "C", "durationSeconds": 3.0}
                        ]
                    }
                ]
            }))
            .unwrap();

        let loaded = store.load_config().unwrap().unwrap();
        assert_unique_ids(loaded["summaryCustomTemplates"].as_array().unwrap());
        assert_unique_ids(loaded["polishCustomPresets"].as_array().unwrap());
        assert_unique_ids(loaded["textReplacementSets"].as_array().unwrap());
        assert_unique_ids(loaded["hotwordSets"].as_array().unwrap());
        assert_unique_ids(loaded["polishKeywordSets"].as_array().unwrap());
        assert_unique_ids(loaded["speakerProfiles"].as_array().unwrap());
        assert_unique_ids(
            loaded["textReplacementSets"][0]["rules"]
                .as_array()
                .unwrap(),
        );
        assert_unique_ids(loaded["hotwordSets"][0]["rules"].as_array().unwrap());
        assert_unique_ids(loaded["speakerProfiles"][0]["samples"].as_array().unwrap());
    }

    #[test]
    fn test_wrapper_payload_detection_only_uses_object_values() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let store = SqliteConfigStore::new(Arc::clone(&db));

        let wrapped = json!({
            "sona-config": {
                "theme": "wrapped",
                "summaryCustomTemplates": [
                    {"id": "wrapped-summary", "name": "Wrapped", "instructions": "Wrapped"}
                ]
            }
        });
        store.save_config(&wrapped).unwrap();
        let loaded_wrapped = store.load_config().unwrap().unwrap();
        assert_eq!(loaded_wrapped["sona-config"]["theme"], "wrapped");
        assert_eq!(
            loaded_wrapped["sona-config"]["summaryCustomTemplates"][0]["id"],
            "wrapped-summary"
        );
        assert!(loaded_wrapped.get("summaryCustomTemplates").is_none());

        let malformed_wrapper = json!({
            "config": null,
            "theme": "top-level",
            "summaryCustomTemplates": [
                {"id": "top-level-summary", "name": "Top Level", "instructions": "Top Level"}
            ]
        });
        store.save_config(&malformed_wrapper).unwrap();
        let loaded = store.load_config().unwrap().unwrap();
        assert_eq!(loaded["config"], Value::Null);
        assert_eq!(loaded["theme"], "top-level");
        assert_eq!(
            loaded["summaryCustomTemplates"][0]["id"],
            "top-level-summary"
        );
    }

    #[test]
    fn test_app_settings_round_trip_json_values() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let store = SqliteConfigStore::new(Arc::clone(&db));

        assert!(store.get_setting("sona-onboarding").unwrap().is_none());

        store
            .set_setting(
                "sona-onboarding",
                &json!({"version": 1, "status": "completed"}),
            )
            .unwrap();
        store
            .set_setting("sona-active-project-id", &json!("project-1"))
            .unwrap();

        assert_eq!(
            store.get_setting("sona-onboarding").unwrap(),
            Some(json!({"version": 1, "status": "completed"}))
        );
        assert_eq!(
            store.get_setting("sona-active-project-id").unwrap(),
            Some(json!("project-1"))
        );
    }

    fn raw_app_config_json(db: &Arc<Database>) -> Value {
        db.with_connection(|conn| {
            let raw: String =
                conn.query_row("SELECT config FROM app_config WHERE id = 1", [], |row| {
                    row.get(0)
                })?;
            serde_json::from_str(&raw).map_err(DatabaseError::SerializationError)
        })
        .unwrap()
    }

    fn count_rows(conn: &rusqlite::Connection, table: &str) -> Result<i64, DatabaseError> {
        conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .map_err(DatabaseError::QueryError)
    }

    fn ordered_ids(conn: &rusqlite::Connection, table: &str) -> Result<Vec<String>, DatabaseError> {
        let mut stmt = conn.prepare(&format!("SELECT id FROM {table} ORDER BY sort_order"))?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(DatabaseError::QueryError)
    }

    fn assert_unique_ids(values: &[Value]) {
        let mut seen = std::collections::HashSet::new();
        for value in values {
            let id = value.get("id").and_then(Value::as_str).unwrap_or_default();
            assert!(!id.is_empty());
            assert!(seen.insert(id.to_string()), "duplicate id: {id}");
        }
    }
}
