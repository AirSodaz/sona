use crate::core::database::DatabaseError;
use crate::core::database::ports::Database as DatabasePort;
use serde_json::Value;
use std::sync::Arc;

#[derive(Clone)]
pub struct SqliteConfigStore<D = crate::core::database::Database>
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
                Ok(Some(config))
            } else {
                Ok(None)
            }
        })
    }

    pub fn save_config(&self, config: &Value) -> Result<(), DatabaseError> {
        let config_str = serde_json::to_string(config)?;
        let projection = AppConfigStartupProjection::from_config(config);
        self.get_db()?.with_write_connection(|conn| {
            conn.execute(
                "INSERT INTO app_config (
                    id, config, migrated_version, http_server_enabled, http_server_host,
                    http_server_port, http_server_api_key, http_server_max_concurrent,
                    http_server_max_queue_size, http_server_max_upload_size_mb,
                    http_server_job_ttl_minutes, http_server_max_streaming,
                    http_server_ip_whitelist, gpu_acceleration
                )
                VALUES (1, ?1, 0, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                ON CONFLICT(id) DO UPDATE SET
                    config = excluded.config,
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
        .or_else(|| value.get("sona_config"))
        .or_else(|| value.get("config"))
        .unwrap_or(value)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::database::Database;
    use serde_json::json;
    use std::sync::Arc;

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
}
