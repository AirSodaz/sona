use crate::platform::paths::{PathKind, PathProvider, TauriPathProvider};
use sona_core::runtime::serve::{
    ServeStartupSettings, online_asr_config_from_app_config, serve_startup_settings_from_app_config,
};
use sona_sqlite::config_store::{
    load_app_config_payload_from_app_local_data_dir,
    load_serve_startup_settings_from_app_local_data_dir,
};
use std::collections::HashMap;

fn load_sqlite_app_config_payload(provider: &dyn PathProvider) -> Option<serde_json::Value> {
    let app_local_data_dir = provider.resolve_path(PathKind::AppLocalData).ok()?;
    load_app_config_payload_from_app_local_data_dir(&app_local_data_dir)
        .map_err(|error| {
            log::warn!("[API Server] Failed to load SQLite app config: {error}");
            error
        })
        .ok()
        .flatten()
}

fn load_sqlite_serve_startup_settings(provider: &dyn PathProvider) -> Option<ServeStartupSettings> {
    let app_local_data_dir = provider.resolve_path(PathKind::AppLocalData).ok()?;
    load_serve_startup_settings_from_app_local_data_dir(&app_local_data_dir)
        .map_err(|error| {
            log::warn!("[API Server] Failed to load SQLite startup settings: {error}");
            error
        })
        .ok()
        .flatten()
}

fn load_legacy_settings_config(provider: &dyn PathProvider) -> Option<serde_json::Value> {
    let app_data_dir = provider.resolve_path(PathKind::AppData).ok()?;
    sona_runtime_fs::load_legacy_settings_app_config(&app_data_dir)
        .map_err(|error| {
            log::warn!("[API Server] Failed to load legacy settings: {error}");
            error
        })
        .ok()
        .flatten()
}

fn load_app_config_for_server(provider: &dyn PathProvider) -> Option<serde_json::Value> {
    load_sqlite_app_config_payload(provider).or_else(|| load_legacy_settings_config(provider))
}

pub fn load_online_asr_config(provider: &dyn PathProvider) -> HashMap<String, serde_json::Value> {
    load_app_config_for_server(provider)
        .map(|config| online_asr_config_from_app_config(&config))
        .unwrap_or_default()
}

pub fn load_online_asr_config_for_app<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> HashMap<String, serde_json::Value> {
    let provider = TauriPathProvider::from_app(app);
    load_online_asr_config(&provider)
}

pub fn load_api_server_startup_settings(provider: &dyn PathProvider) -> ServeStartupSettings {
    if let Some(settings) = load_sqlite_serve_startup_settings(provider) {
        return settings;
    }
    load_app_config_for_server(provider)
        .map(|config| serve_startup_settings_from_app_config(&config))
        .unwrap_or_default()
}

pub fn load_api_server_startup_settings_for_app<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> ServeStartupSettings {
    let provider = TauriPathProvider::from_app(app);
    load_api_server_startup_settings(&provider)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::paths::{MockPathProvider, PathKind};
    use sona_sqlite::Database;
    use sona_sqlite::config_store::SqliteConfigStore;
    use std::collections::HashMap as StdHashMap;
    use std::path::Path as StdPath;
    use std::sync::Arc;

    fn provider_for_config_test(
        app_data_dir: &StdPath,
        app_local_data_dir: &StdPath,
    ) -> MockPathProvider {
        let mut entries = StdHashMap::new();
        entries.insert(PathKind::AppData, Ok(app_data_dir.to_path_buf()));
        entries.insert(PathKind::AppLocalData, Ok(app_local_data_dir.to_path_buf()));
        MockPathProvider::from_map(entries)
    }

    #[test]
    fn load_online_asr_config_reads_sqlite_app_config_before_legacy_settings_file() {
        let app_data = tempfile::tempdir().unwrap();
        let app_local_data = tempfile::tempdir().unwrap();
        let provider = provider_for_config_test(app_data.path(), app_local_data.path());
        let db = Database::open(app_local_data.path()).unwrap();
        let store = SqliteConfigStore::new(Arc::new(db));
        store
            .save_config(&serde_json::json!({
                "asr": {
                    "providers": {
                        "online": {
                            "volcengine": {
                                "apiKey": "sqlite-key"
                            }
                        }
                    }
                }
            }))
            .unwrap();
        std::fs::write(
            app_data.path().join("settings.json"),
            r#"{"sona-config":{"asr":{"providers":{"online":{"volcengine":{"apiKey":"legacy-key"}}}}}}"#,
        )
        .unwrap();

        let config = load_online_asr_config(&provider);

        assert_eq!(
            config
                .get("volcengine")
                .and_then(|value| value.get("apiKey"))
                .and_then(serde_json::Value::as_str),
            Some("sqlite-key")
        );
    }

    #[test]
    fn load_online_asr_config_falls_back_to_legacy_when_sqlite_open_fails() {
        let app_data = tempfile::tempdir().unwrap();
        let app_local_data_parent = tempfile::tempdir().unwrap();
        let app_local_data_file = app_local_data_parent.path().join("app-local-data-file");
        std::fs::write(&app_local_data_file, "not a directory").unwrap();
        let provider = provider_for_config_test(app_data.path(), &app_local_data_file);
        std::fs::write(
            app_data.path().join("settings.json"),
            r#"{"sona-config":{"asr":{"providers":{"online":{"volcengine":{"apiKey":"legacy-key"}}}}}}"#,
        )
        .unwrap();

        let config = load_online_asr_config(&provider);

        assert_eq!(
            config
                .get("volcengine")
                .and_then(|value| value.get("apiKey"))
                .and_then(serde_json::Value::as_str),
            Some("legacy-key")
        );
    }

    #[test]
    fn load_online_asr_config_unwraps_sqlite_object_wrappers() {
        let app_data = tempfile::tempdir().unwrap();
        let app_local_data = tempfile::tempdir().unwrap();
        let provider = provider_for_config_test(app_data.path(), app_local_data.path());
        let db = Database::open(app_local_data.path()).unwrap();
        let store = SqliteConfigStore::new(Arc::new(db));
        store
            .save_config(&serde_json::json!({
                "sona-config": {
                    "asr": {
                        "providers": {
                            "online": {
                                "volcengine": {
                                    "apiKey": "wrapped-sqlite-key"
                                }
                            }
                        }
                    }
                }
            }))
            .unwrap();

        let config = load_online_asr_config(&provider);

        assert_eq!(
            config
                .get("volcengine")
                .and_then(|value| value.get("apiKey"))
                .and_then(serde_json::Value::as_str),
            Some("wrapped-sqlite-key")
        );
    }

    #[test]
    fn load_online_asr_config_ignores_non_object_wrappers() {
        let app_data = tempfile::tempdir().unwrap();
        let app_local_data = tempfile::tempdir().unwrap();
        let provider = provider_for_config_test(app_data.path(), app_local_data.path());
        let db = Database::open(app_local_data.path()).unwrap();
        let store = SqliteConfigStore::new(Arc::new(db));
        store
            .save_config(&serde_json::json!({
                "config": null,
                "asr": {
                    "providers": {
                        "online": {
                            "volcengine": {
                                "apiKey": "top-level-key"
                            }
                        }
                    }
                }
            }))
            .unwrap();

        let config = load_online_asr_config(&provider);

        assert_eq!(
            config
                .get("volcengine")
                .and_then(|value| value.get("apiKey"))
                .and_then(serde_json::Value::as_str),
            Some("top-level-key")
        );
    }

    #[test]
    fn load_api_server_startup_settings_reads_sqlite_app_config() {
        let app_data = tempfile::tempdir().unwrap();
        let app_local_data = tempfile::tempdir().unwrap();
        let provider = provider_for_config_test(app_data.path(), app_local_data.path());
        let db = Database::open(app_local_data.path()).unwrap();
        let store = SqliteConfigStore::new(Arc::new(db));
        store
            .save_config(&serde_json::json!({
                "httpServerEnabled": true,
                "httpServerHost": "0.0.0.0",
                "httpServerPort": 15555,
                "httpServerApiKey": "sqlite-secret",
                "httpServerMaxConcurrent": 3,
                "httpServerMaxQueueSize": 12,
                "httpServerMaxUploadSizeMB": 99,
                "httpServerJobTtlMinutes": 7,
                "httpServerMaxStreaming": 4,
                "httpServerIpWhitelist": "127.0.0.1/32",
                "gpuAcceleration": "cpu"
            }))
            .unwrap();

        let settings = load_api_server_startup_settings(&provider);

        assert!(settings.enabled);
        assert_eq!(settings.config.host.as_deref(), Some("0.0.0.0"));
        assert_eq!(settings.config.port, Some(15555));
        assert_eq!(settings.config.api_key.as_deref(), Some("sqlite-secret"));
        assert_eq!(settings.config.max_concurrent, Some(3));
        assert_eq!(settings.config.max_queue_size, Some(12));
        assert_eq!(settings.config.max_upload_size_mb, Some(99));
        assert_eq!(settings.config.job_ttl_minutes, Some(7));
        assert_eq!(settings.config.max_streaming, Some(4));
        assert_eq!(
            settings.config.ip_whitelist.as_deref(),
            Some("127.0.0.1/32")
        );
        assert_eq!(settings.config.gpu_acceleration.as_deref(), Some("cpu"));
    }

    #[test]
    fn load_api_server_startup_settings_reads_sqlite_projection_columns() {
        let app_data = tempfile::tempdir().unwrap();
        let app_local_data = tempfile::tempdir().unwrap();
        let provider = provider_for_config_test(app_data.path(), app_local_data.path());
        let db = Database::open(app_local_data.path()).unwrap();
        db.with_write_connection(|conn| {
            conn.execute(
                "INSERT INTO app_config (
                    id, config, config_version, updated_at, http_server_enabled, http_server_host,
                    http_server_port, http_server_api_key, http_server_max_concurrent,
                    http_server_max_queue_size, http_server_max_upload_size_mb,
                    http_server_job_ttl_minutes, http_server_max_streaming,
                    http_server_ip_whitelist, gpu_acceleration
                )
                VALUES (1, '{}', 7, 0, 1, '0.0.0.0', 16666, 'column-secret', 5, 44, 256, 9, 7, '10.0.0.0/8', 'cuda')",
                [],
            )?;
            Ok(())
        })
        .unwrap();

        let settings = load_api_server_startup_settings(&provider);

        assert!(settings.enabled);
        assert_eq!(settings.config.host.as_deref(), Some("0.0.0.0"));
        assert_eq!(settings.config.port, Some(16666));
        assert_eq!(settings.config.api_key.as_deref(), Some("column-secret"));
        assert_eq!(settings.config.max_concurrent, Some(5));
        assert_eq!(settings.config.max_queue_size, Some(44));
        assert_eq!(settings.config.max_upload_size_mb, Some(256));
        assert_eq!(settings.config.job_ttl_minutes, Some(9));
        assert_eq!(settings.config.max_streaming, Some(7));
        assert_eq!(settings.config.ip_whitelist.as_deref(), Some("10.0.0.0/8"));
        assert_eq!(settings.config.gpu_acceleration.as_deref(), Some("cuda"));
    }
}
