use crate::core::database::Database;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;

use super::types::AutomationRepositoryState;

#[derive(Clone)]
pub struct SqliteAutomationRepository {
    #[allow(dead_code)]
    app_local_data_dir: PathBuf,
    db: Option<Arc<Database>>,
}

impl SqliteAutomationRepository {
    pub fn new(app_local_data_dir: PathBuf) -> Self {
        Self {
            app_local_data_dir,
            db: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_db(app_local_data_dir: PathBuf, db: Database) -> Self {
        Self {
            app_local_data_dir,
            db: Some(Arc::new(db)),
        }
    }

    fn get_db(&self) -> &Database {
        if let Some(ref db) = self.db {
            db
        } else {
            Database::global()
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

    pub fn load_state(&self) -> Result<AutomationRepositoryState, String> {
        let rules = self.get_db().with_connection(|conn| {
            let mut stmt = conn.prepare("SELECT data FROM automation_rules ORDER BY id")?;
            let rows = stmt.query_map([], |row| {
                let data_str: String = row.get(0)?;
                let val: Value = serde_json::from_str(&data_str)
                    .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
                Ok(val)
            })?;
            let mut items = Vec::new();
            for row in rows {
                items.push(row?);
            }
            Ok(items)
        })?;

        let processed_entries = self.get_db().with_connection(|conn| {
            let mut stmt = conn.prepare("SELECT data FROM automation_processed ORDER BY id")?;
            let rows = stmt.query_map([], |row| {
                let data_str: String = row.get(0)?;
                let val: Value = serde_json::from_str(&data_str)
                    .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
                Ok(val)
            })?;
            let mut items = Vec::new();
            for row in rows {
                items.push(row?);
            }
            Ok(items)
        })?;

        Ok(AutomationRepositoryState {
            rules,
            processed_entries,
        })
    }

    pub fn persist_rules(&self, rules: Vec<Value>) -> Result<(), String> {
        self.get_db().with_transaction(|tx| {
            tx.execute("DELETE FROM automation_rules", [])?;
            let mut stmt = tx.prepare("INSERT INTO automation_rules (id, data) VALUES (?1, ?2)")?;
            for mut rule in rules {
                let id = Self::ensure_id(&mut rule);
                let data_str = serde_json::to_string(&rule)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                stmt.execute(rusqlite::params![id, data_str])?;
            }
            Ok(())
        })
    }

    pub fn persist_processed_entries(&self, entries: Vec<Value>) -> Result<(), String> {
        self.get_db().with_transaction(|tx| {
            tx.execute("DELETE FROM automation_processed", [])?;
            let mut stmt =
                tx.prepare("INSERT INTO automation_processed (id, data) VALUES (?1, ?2)")?;
            for mut entry in entries {
                let id = Self::ensure_id(&mut entry);
                let data_str = serde_json::to_string(&entry)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                stmt.execute(rusqlite::params![id, data_str])?;
            }
            Ok(())
        })
    }

    pub fn persist_state(&self, rules: Vec<Value>, entries: Vec<Value>) -> Result<(), String> {
        self.get_db().with_transaction(|tx| {
            tx.execute("DELETE FROM automation_rules", [])?;
            {
                let mut stmt =
                    tx.prepare("INSERT INTO automation_rules (id, data) VALUES (?1, ?2)")?;
                for mut rule in rules {
                    let id = Self::ensure_id(&mut rule);
                    let data_str = serde_json::to_string(&rule)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                    stmt.execute(rusqlite::params![id, data_str])?;
                }
            }
            tx.execute("DELETE FROM automation_processed", [])?;
            {
                let mut stmt =
                    tx.prepare("INSERT INTO automation_processed (id, data) VALUES (?1, ?2)")?;
                for mut entry in entries {
                    let id = Self::ensure_id(&mut entry);
                    let data_str = serde_json::to_string(&entry)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                    stmt.execute(rusqlite::params![id, data_str])?;
                }
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::database::Database;
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn test_automation_persist_and_load() {
        let db = Database::open_in_memory().unwrap();
        let repo = SqliteAutomationRepository::with_db(PathBuf::new(), db);

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
    fn test_automation_empty_state() {
        let db = Database::open_in_memory().unwrap();
        let repo = SqliteAutomationRepository::with_db(PathBuf::new(), db);

        let state = repo.load_state().unwrap();
        assert!(state.rules.is_empty());
        assert!(state.processed_entries.is_empty());
    }
}
