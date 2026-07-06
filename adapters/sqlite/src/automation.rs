use crate::DatabaseError;
use crate::ports::Database as DatabasePort;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRepositoryState {
    pub rules: Vec<Value>,
    pub processed_entries: Vec<Value>,
}

#[derive(Clone)]
pub struct SqliteAutomationRepository<D = crate::Database>
where
    D: DatabasePort,
{
    db: Arc<D>,
}

crate::impl_db_repository!(SqliteAutomationRepository);

impl<D> SqliteAutomationRepository<D>
where
    D: DatabasePort,
{
    pub fn load_state(&self) -> Result<AutomationRepositoryState, DatabaseError> {
        self.get_db()?.with_transaction(|tx| {
            let rules = Self::load_rules(tx)?;
            let processed_entries = Self::load_processed_entries(tx)?;

            Ok(AutomationRepositoryState {
                rules,
                processed_entries,
            })
        })
    }

    fn load_rules(conn: &rusqlite::Connection) -> Result<Vec<Value>, DatabaseError> {
        let mut stmt = conn.prepare_cached(
            "SELECT id, name, project_id, preset_id, watch_directory, recursive, enabled,
                    stage_auto_polish, stage_polish_preset_id, stage_auto_translate,
                    stage_translation_language, stage_export_enabled,
                    export_directory, export_format, export_mode, export_prefix,
                    created_at, updated_at
             FROM automation_rules
             ORDER BY id",
        )?;
        let rows = stmt.query_map([], map_row_to_rule_value)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(DatabaseError::QueryError)
    }

    fn load_processed_entries(conn: &rusqlite::Connection) -> Result<Vec<Value>, DatabaseError> {
        let mut stmt = conn.prepare_cached(
            "SELECT id, rule_id, file_path, source_fingerprint, size, mtime_ms, status,
                    processed_at, history_id, export_path, error_message
             FROM automation_processed
             ORDER BY id",
        )?;
        let rows = stmt.query_map([], map_row_to_processed_value)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(DatabaseError::QueryError)
    }

    pub fn persist_rules(&self, rules: Vec<Value>) -> Result<(), DatabaseError> {
        self.get_db()?.with_transaction(|tx| {
            tx.execute("DELETE FROM automation_rules", [])?;
            persist_rule_values(tx, rules)?;
            Ok(())
        })
    }

    pub fn persist_processed_entries(&self, entries: Vec<Value>) -> Result<(), DatabaseError> {
        self.get_db()?.with_transaction(|tx| {
            tx.execute("DELETE FROM automation_processed", [])?;
            persist_processed_values(tx, entries)?;
            Ok(())
        })
    }

    pub fn persist_state(
        &self,
        rules: Vec<Value>,
        entries: Vec<Value>,
    ) -> Result<(), DatabaseError> {
        self.get_db()?.with_transaction(|tx| {
            tx.execute("DELETE FROM automation_rules", [])?;
            persist_rule_values(tx, rules)?;
            tx.execute("DELETE FROM automation_processed", [])?;
            persist_processed_values(tx, entries)?;
            Ok(())
        })
    }
}

fn ensure_id(data: &mut Value) -> String {
    if let Some(id) = data.get("id").and_then(Value::as_str) {
        id.to_string()
    } else {
        let id = uuid::Uuid::new_v4().to_string();
        data.as_object_mut()
            .map(|obj| obj.insert("id".to_string(), Value::String(id.clone())));
        id
    }
}

fn persist_rule_values(tx: &rusqlite::Transaction, rules: Vec<Value>) -> Result<(), DatabaseError> {
    let mut stmt = tx.prepare_cached(
        "INSERT INTO automation_rules (
            id, name, project_id, preset_id, watch_directory, recursive, enabled,
            stage_auto_polish, stage_polish_preset_id, stage_auto_translate,
            stage_translation_language, stage_export_enabled,
            export_directory, export_format, export_mode, export_prefix,
            created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
    )?;
    for mut rule in rules {
        let id = ensure_id(&mut rule);
        let stage = rule.get("stageConfig");
        let export = rule.get("exportConfig");
        stmt.execute(rusqlite::params![
            id,
            string_field(&rule, "name", ""),
            string_field(&rule, "projectId", ""),
            string_field(&rule, "presetId", "custom"),
            string_field(&rule, "watchDirectory", ""),
            bool_field(&rule, "recursive") as i64,
            bool_field(&rule, "enabled") as i64,
            nested_bool_field(stage, "autoPolish") as i64,
            nested_string_field(stage, "polishPresetId", "general"),
            nested_bool_field(stage, "autoTranslate") as i64,
            nested_string_field(stage, "translationLanguage", "en"),
            nested_bool_field(stage, "exportEnabled") as i64,
            nested_string_field(export, "directory", ""),
            nested_string_field(export, "format", "txt"),
            nested_string_field(export, "mode", "original"),
            nested_string_field(export, "prefix", ""),
            integer_field(&rule, "createdAt"),
            integer_field(&rule, "updatedAt"),
        ])?;
    }
    Ok(())
}

fn persist_processed_values(
    tx: &rusqlite::Transaction,
    entries: Vec<Value>,
) -> Result<(), DatabaseError> {
    let mut stmt = tx.prepare_cached(
        "INSERT INTO automation_processed (
            id, rule_id, file_path, source_fingerprint, size, mtime_ms, status,
            processed_at, history_id, export_path, error_message
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
    )?;
    for mut entry in entries {
        let id = ensure_id(&mut entry);
        stmt.execute(rusqlite::params![
            id,
            string_field(&entry, "ruleId", ""),
            string_field(&entry, "filePath", ""),
            string_field(&entry, "sourceFingerprint", ""),
            integer_field(&entry, "size"),
            integer_field(&entry, "mtimeMs"),
            string_field(&entry, "status", "complete"),
            integer_field(&entry, "processedAt"),
            optional_string_field(&entry, "historyId"),
            optional_string_field(&entry, "exportPath"),
            optional_string_field(&entry, "errorMessage"),
        ])?;
    }
    Ok(())
}

fn map_row_to_rule_value(row: &rusqlite::Row) -> rusqlite::Result<Value> {
    Ok(json!({
        "id": row.get::<_, String>("id")?,
        "name": row.get::<_, String>("name")?,
        "projectId": row.get::<_, String>("project_id")?,
        "presetId": row.get::<_, String>("preset_id")?,
        "watchDirectory": row.get::<_, String>("watch_directory")?,
        "recursive": row.get::<_, i64>("recursive")? != 0,
        "enabled": row.get::<_, i64>("enabled")? != 0,
        "stageConfig": {
            "autoPolish": row.get::<_, i64>("stage_auto_polish")? != 0,
            "polishPresetId": row.get::<_, String>("stage_polish_preset_id")?,
            "autoTranslate": row.get::<_, i64>("stage_auto_translate")? != 0,
            "translationLanguage": row.get::<_, String>("stage_translation_language")?,
            "exportEnabled": row.get::<_, i64>("stage_export_enabled")? != 0,
        },
        "exportConfig": {
            "directory": row.get::<_, String>("export_directory")?,
            "format": row.get::<_, String>("export_format")?,
            "mode": row.get::<_, String>("export_mode")?,
            "prefix": row.get::<_, String>("export_prefix")?,
        },
        "createdAt": row.get::<_, i64>("created_at")?,
        "updatedAt": row.get::<_, i64>("updated_at")?,
    }))
}

fn map_row_to_processed_value(row: &rusqlite::Row) -> rusqlite::Result<Value> {
    let mut entry = Map::new();
    entry.insert("id".to_string(), json!(row.get::<_, String>("id")?));
    entry.insert(
        "ruleId".to_string(),
        json!(row.get::<_, String>("rule_id")?),
    );
    entry.insert(
        "filePath".to_string(),
        json!(row.get::<_, String>("file_path")?),
    );
    entry.insert(
        "sourceFingerprint".to_string(),
        json!(row.get::<_, String>("source_fingerprint")?),
    );
    entry.insert("size".to_string(), json!(row.get::<_, i64>("size")?));
    entry.insert("mtimeMs".to_string(), json!(row.get::<_, i64>("mtime_ms")?));
    entry.insert("status".to_string(), json!(row.get::<_, String>("status")?));
    entry.insert(
        "processedAt".to_string(),
        json!(row.get::<_, i64>("processed_at")?),
    );
    insert_optional_output(&mut entry, "historyId", row.get("history_id")?);
    insert_optional_output(&mut entry, "exportPath", row.get("export_path")?);
    insert_optional_output(&mut entry, "errorMessage", row.get("error_message")?);
    Ok(Value::Object(entry))
}

fn insert_optional_output(map: &mut Map<String, Value>, key: &str, value: Option<String>) {
    if let Some(value) = value {
        map.insert(key.to_string(), Value::String(value));
    }
}

fn string_field(value: &Value, key: &str, default: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_string()
}

fn optional_string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn bool_field(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn integer_field(value: &Value, key: &str) -> i64 {
    value
        .get(key)
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_u64().map(|n| n as i64))
                .or_else(|| value.as_f64().map(|n| n.round() as i64))
        })
        .unwrap_or(0)
}

fn nested_string_field(value: Option<&Value>, key: &str, default: &str) -> String {
    value
        .and_then(|value| value.get(key))
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_string()
}

fn nested_bool_field(value: Option<&Value>, key: &str) -> bool {
    value
        .and_then(|value| value.get(key))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Database;
    use serde_json::json;
    use std::path::PathBuf;
    use std::sync::Arc;

    #[test]
    fn test_automation_persist_and_load() {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let repo = SqliteAutomationRepository::new(Arc::clone(&db));

        let rules = vec![
            json!({"name": "Rule 1", "watchDirectory": "/watch", "projectId": "proj-1"}),
            json!({"name": "Rule 2", "watchDirectory": "/watch2", "projectId": "proj-2"}),
        ];
        let entries = vec![json!({"filePath": "/path/to/file.mp3", "processedAt": "2026-01-01"})];

        repo.persist_state(rules.clone(), entries.clone()).unwrap();

        let state = repo.load_state().unwrap();
        assert_eq!(state.rules.len(), 2);
        assert_eq!(state.processed_entries.len(), 1);
        let names: Vec<&str> = state
            .rules
            .iter()
            .map(|r| r["name"].as_str().unwrap_or(""))
            .collect();
        assert!(names.contains(&"Rule 1"));
        assert!(names.contains(&"Rule 2"));
        assert!(state.rules[0].get("id").and_then(Value::as_str).is_some());
    }

    #[test]
    fn test_automation_persist_rules_replace() {
        let db = Database::open_in_memory().unwrap();
        let repo = SqliteAutomationRepository::with_db(PathBuf::new(), db);

        let rules = vec![json!({"name": "Old Rule", "projectId": "p1"})];
        repo.persist_rules(rules).unwrap();

        let rules2 = vec![json!({"name": "New Rule", "projectId": "p2"})];
        repo.persist_rules(rules2).unwrap();

        let state = repo.load_state().unwrap();
        assert_eq!(state.rules.len(), 1);
        assert_eq!(state.rules[0]["name"], "New Rule");
    }

    #[test]
    fn load_state_returns_rules_and_processed_entries_together() {
        let db = Database::open_in_memory().unwrap();
        let repo = SqliteAutomationRepository::with_db(PathBuf::new(), db);

        repo.persist_state(
            vec![json!({"id": "rule-1", "name": "Rule", "projectId": "project-1"})],
            vec![json!({"id": "entry-1", "filePath": "C:\\audio.wav"})],
        )
        .unwrap();

        let state = repo.load_state().unwrap();

        assert_eq!(
            state.rules,
            vec![json!({
                "id": "rule-1",
                "name": "Rule",
                "projectId": "project-1",
                "presetId": "custom",
                "watchDirectory": "",
                "recursive": false,
                "enabled": false,
                "stageConfig": {
                    "autoPolish": false,
                    "polishPresetId": "general",
                    "autoTranslate": false,
                    "translationLanguage": "en",
                    "exportEnabled": false
                },
                "exportConfig": {
                    "directory": "",
                    "format": "txt",
                    "mode": "original",
                    "prefix": ""
                },
                "createdAt": 0,
                "updatedAt": 0
            })]
        );
        assert_eq!(
            state.processed_entries,
            vec![json!({
                "id": "entry-1",
                "ruleId": "",
                "filePath": "C:\\audio.wav",
                "sourceFingerprint": "",
                "size": 0,
                "mtimeMs": 0,
                "status": "complete",
                "processedAt": 0
            })]
        );
    }

    #[test]
    fn test_automation_empty_state() {
        let db = Database::open_in_memory().unwrap();
        let repo = SqliteAutomationRepository::with_db(PathBuf::new(), db);

        let state = repo.load_state().unwrap();
        assert!(state.rules.is_empty());
        assert!(state.processed_entries.is_empty());
    }
}
