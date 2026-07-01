use crate::core::database::Database;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct SqliteConfigStore {
    #[allow(dead_code)]
    app_local_data_dir: PathBuf,
    db: Option<Arc<Database>>,
}

impl SqliteConfigStore {
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

    pub fn load_config(&self) -> Result<Option<Value>, String> {
        self.get_db().with_connection(|conn| {
            let mut stmt = conn.prepare("SELECT config FROM app_config WHERE id = 1")?;
            let mut rows = stmt.query([])?;
            if let Some(row) = rows.next()? {
                let config_str: String = row.get(0)?;
                let config: Value = serde_json::from_str(&config_str)
                    .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
                Ok(Some(config))
            } else {
                Ok(None)
            }
        })
    }

    pub fn save_config(&self, config: &Value) -> Result<(), String> {
        let config_str = serde_json::to_string(config).map_err(|e| e.to_string())?;
        self.get_db().with_connection(|conn| {
            conn.execute(
                "INSERT INTO app_config (id, config, migrated_version) VALUES (1, ?1, 0)
                 ON CONFLICT(id) DO UPDATE SET config = excluded.config",
                rusqlite::params![config_str],
            )?;
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
    fn test_config_load_save() {
        let db = Database::open_in_memory().unwrap();
        let store = SqliteConfigStore::with_db(PathBuf::new(), db);

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
        let db = Database::open_in_memory().unwrap();
        let store = SqliteConfigStore::with_db(PathBuf::new(), db);

        store.save_config(&json!({"version": 1})).unwrap();
        store.save_config(&json!({"version": 2})).unwrap();

        let loaded = store.load_config().unwrap().unwrap();
        assert_eq!(loaded["version"], 2);
    }
}
