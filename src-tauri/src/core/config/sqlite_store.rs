use crate::core::database::DatabaseError;
use serde_json::Value;

#[derive(Clone)]
pub struct SqliteConfigStore {
    db: crate::core::database::DbProvider,
}

crate::impl_db_repository!(SqliteConfigStore);

impl SqliteConfigStore {
    pub fn load_config(&self) -> Result<Option<Value>, DatabaseError> {
        self.get_db()?.with_connection(|conn| {
            let mut stmt = conn.prepare_cached("SELECT config FROM app_config WHERE id = 1")?;
            let mut rows = stmt.query([])?;
            if let Some(row) = rows.next()? {
                let config_str: String = row.get(0)?;
                let config: Value = serde_json::from_str(&config_str)?;
                Ok(Some(config))
            } else {
                Ok(None)
            }
        })
    }

    pub fn save_config(&self, config: &Value) -> Result<(), DatabaseError> {
        let config_str = serde_json::to_string(config)?;
        self.get_db()?.with_write_connection(|conn| {
            conn.execute(
                "INSERT INTO app_config (id, config, migrated_version) VALUES (1, ?1, 0)
                 ON CONFLICT(id) DO UPDATE SET config = excluded.config",
                rusqlite::params![config_str],
            )?;
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

    #[test]
    fn test_app_settings_round_trip_json_values() {
        let db = Database::open_in_memory().unwrap();
        let store = SqliteConfigStore::with_db(PathBuf::new(), db);

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
}
