use serde_json::Value;
use sona_runtime_fs::SystemClock;
use sona_sqlite::{Database, SqliteAppConfigAdapter};
use std::sync::Arc;
use tauri::{AppHandle, Runtime};

fn run_app_config_adapter<T>(
    db: Arc<Database>,
    operation: impl FnOnce(&SqliteAppConfigAdapter) -> Result<T, String>,
) -> Result<T, String> {
    let adapter = SqliteAppConfigAdapter::new(db, Arc::new(SystemClock));
    operation(&adapter)
}

pub fn load_config<R: Runtime>(app: &AppHandle<R>) -> Result<Option<Value>, String> {
    let db = crate::platform::database::sqlite_database(app);
    run_app_config_adapter(db, |adapter| adapter.load_config())
}

pub fn save_config<R: Runtime>(app: &AppHandle<R>, config: Value) -> Result<(), String> {
    let db = crate::platform::database::sqlite_database(app);
    run_app_config_adapter(db, |adapter| adapter.save_config(&config))
}

pub fn get_setting<R: Runtime>(app: &AppHandle<R>, key: String) -> Result<Option<Value>, String> {
    let db = crate::platform::database::sqlite_database(app);
    run_app_config_adapter(db, |adapter| adapter.get_setting(&key))
}

pub fn set_setting<R: Runtime>(
    app: &AppHandle<R>,
    key: String,
    value: Value,
) -> Result<(), String> {
    let db = crate::platform::database::sqlite_database(app);
    run_app_config_adapter(db, |adapter| adapter.set_setting(&key, &value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sona_sqlite::Database;
    use std::sync::Arc;

    fn in_memory_database() -> Arc<Database> {
        Arc::new(Database::open_in_memory().unwrap())
    }

    #[test]
    fn desktop_composition_loads_saves_gets_and_sets_config() {
        let db = in_memory_database();
        let config = serde_json::json!({"theme": "dark", "configVersion": 7});

        run_app_config_adapter(Arc::clone(&db), |adapter| adapter.save_config(&config)).unwrap();
        let loaded = run_app_config_adapter(Arc::clone(&db), |adapter| adapter.load_config())
            .unwrap()
            .unwrap();
        assert_eq!(loaded.get("theme"), Some(&serde_json::json!("dark")));
        assert_eq!(loaded.get("configVersion"), Some(&serde_json::json!(7)));

        run_app_config_adapter(Arc::clone(&db), |adapter| {
            adapter.set_setting("locale", &serde_json::json!({"language": "zh-CN"}))
        })
        .unwrap();
        let setting = run_app_config_adapter(db, |adapter| adapter.get_setting("locale")).unwrap();
        assert_eq!(setting, Some(serde_json::json!({"language": "zh-CN"})));
    }

    #[test]
    fn desktop_composition_preserves_malformed_json_serialization_error_prefix() {
        let db = in_memory_database();
        run_app_config_adapter(Arc::clone(&db), |adapter| {
            adapter.save_config(&serde_json::json!({"theme": "dark"}))
        })
        .unwrap();
        db.with_write_connection(|connection| {
            connection.execute("UPDATE app_config SET config = '{' WHERE id = 1", [])?;
            Ok(())
        })
        .unwrap();

        let error = run_app_config_adapter(db, |adapter| adapter.load_config()).unwrap_err();

        assert!(error.starts_with("Serialization error: "), "{error}");
    }

    #[test]
    fn desktop_composition_propagates_sqlite_store_errors() {
        let db = in_memory_database();
        db.with_write_connection(|connection| {
            connection.execute("DROP TABLE app_settings", [])?;
            Ok(())
        })
        .unwrap();

        let error =
            run_app_config_adapter(db, |adapter| adapter.get_setting("locale")).unwrap_err();

        assert_eq!(error, "Query error: no such table: app_settings");
    }
}
