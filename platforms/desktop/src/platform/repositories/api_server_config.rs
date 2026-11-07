use crate::platform::paths::{PathKind, PathPort};
use sona_core::config::ConfigError;
use sona_core::runtime::serve::{
    ServeStartupSettings, online_asr_config_from_app_config, serve_startup_settings_from_app_config,
};
use sona_runtime_fs::SystemClock;
use sona_sqlite::{Database, DatabaseError, SqliteAppConfigAdapter};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

fn database_for_app_local_data_dir_or_open(
    app_local_data_dir: &Path,
    global_database: Option<Arc<Database>>,
    open_database: impl FnOnce(&Path) -> Result<Database, DatabaseError>,
) -> Result<Arc<Database>, DatabaseError> {
    if let Some(database) = global_database
        && database.is_for_app_local_data_dir(app_local_data_dir)
    {
        return Ok(database);
    }
    open_database(app_local_data_dir).map(Arc::new)
}

fn with_config_adapter<T>(
    app_local_data_dir: &Path,
    database: Option<Arc<Database>>,
    load: impl FnOnce(&SqliteAppConfigAdapter) -> Result<T, ConfigError>,
) -> Result<T, String> {
    let database =
        database_for_app_local_data_dir_or_open(app_local_data_dir, database, Database::open)
            .map_err(|error| error.to_string())?;
    let adapter = SqliteAppConfigAdapter::new(database, Arc::new(SystemClock));
    load(&adapter).map_err(|error| error.to_string())
}

fn load_sqlite_app_config_payload(
    provider: &dyn PathPort,
    database: Option<Arc<Database>>,
) -> Option<serde_json::Value> {
    let app_local_data_dir = provider.resolve_path(PathKind::AppLocalData).ok()?;
    with_config_adapter(&app_local_data_dir, database, |adapter| {
        adapter.load_app_config_payload()
    })
    .map_err(|error| {
        log::warn!("[API Server] Failed to load SQLite app config: {error}");
        error
    })
    .ok()
    .flatten()
}

fn load_sqlite_serve_startup_settings(
    provider: &dyn PathPort,
    database: Option<Arc<Database>>,
) -> Option<ServeStartupSettings> {
    let app_local_data_dir = provider.resolve_path(PathKind::AppLocalData).ok()?;
    with_config_adapter(&app_local_data_dir, database, |adapter| {
        adapter.load_serve_startup_settings()
    })
    .map_err(|error| {
        log::warn!("[API Server] Failed to load SQLite startup settings: {error}");
        error
    })
    .ok()
    .flatten()
}

fn load_app_config_for_server(provider: &dyn PathPort) -> Option<serde_json::Value> {
    load_app_config_for_server_with_database(provider, None)
}

fn load_app_config_for_server_with_database(
    provider: &dyn PathPort,
    database: Option<Arc<Database>>,
) -> Option<serde_json::Value> {
    load_sqlite_app_config_payload(provider, database)
}

pub fn load_online_asr_config(provider: &dyn PathPort) -> HashMap<String, serde_json::Value> {
    load_app_config_for_server(provider)
        .map(|config| online_asr_config_from_app_config(&config))
        .unwrap_or_default()
}

pub fn load_api_server_startup_settings(provider: &dyn PathPort) -> ServeStartupSettings {
    if let Some(settings) = load_sqlite_serve_startup_settings(provider, None) {
        return settings;
    }
    load_app_config_for_server(provider)
        .map(|config| serve_startup_settings_from_app_config(&config))
        .unwrap_or_default()
}

pub fn load_ffmpeg_path(provider: &dyn PathPort) -> Option<String> {
    load_app_config_for_server(provider).and_then(|config| {
        let custom_path = config
            .get("ffmpegPath")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from);
        let enabled = config
            .get("ffmpegEnabled")
            .and_then(|v| v.as_bool())
            .unwrap_or_else(|| custom_path.is_some());
        if !enabled {
            return None;
        }
        if custom_path.is_some() {
            return custom_path;
        }
        sona_core::ports::asr::find_available_ffmpeg(None).map(|p| p.to_string_lossy().into_owned())
    })
}

pub fn load_feature_llm_config(
    provider: &dyn PathPort,
    feature: &str,
) -> Option<sona_core::llm::requests::LlmConfig> {
    load_app_config_for_server(provider)
        .and_then(|config| resolve_feature_llm_config_from_app_config(&config, feature))
}

pub fn resolve_feature_llm_config_from_app_config(
    config: &serde_json::Value,
    feature: &str,
) -> Option<sona_core::llm::requests::LlmConfig> {
    // 1. Try modern llmSettings
    if let Some(llm_settings) = config.get("llmSettings") {
        let feature_key = match feature {
            "polish" => "polishModelId",
            "translation" | "translate" => "translationModelId",
            "summary" => "summaryModelId",
            _ => "polishModelId",
        };
        let model_id = llm_settings
            .get("selections")
            .and_then(|s| s.get(feature_key))
            .and_then(|v| v.as_str())
            .or_else(|| {
                llm_settings
                    .get("modelOrder")
                    .and_then(|arr| arr.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|v| v.as_str())
            });

        if let Some(m_id) = model_id
            && let Some(model_entry) = llm_settings.get("models").and_then(|m| m.get(m_id))
            && let Some(provider_str) = model_entry.get("provider").and_then(|p| p.as_str())
            && let Some(model_str) = model_entry.get("model").and_then(|m| m.as_str())
        {
            let provider_settings = llm_settings
                .get("providers")
                .and_then(|p| p.get(provider_str));
            let provider_definition =
                sona_core::llm::providers::find_llm_provider_by_id_or_alias(provider_str);
            let default_host = match provider_str {
                "google_translate_free" => "https://translate.googleapis.com/translate_a/single",
                "google_translate" => "https://translation.googleapis.com/language/translate/v2",
                _ => provider_definition
                    .map(|provider| provider.defaults.api_host.as_str())
                    .unwrap_or_default(),
            };
            let api_host = provider_settings
                .and_then(|s| s.get("apiHost"))
                .and_then(|v| v.as_str())
                .filter(|value| !value.is_empty())
                .unwrap_or(default_host);
            let api_key = provider_settings
                .and_then(|s| s.get("apiKey"))
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let api_path = provider_settings
                .and_then(|s| s.get("apiPath"))
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .or_else(|| {
                    provider_definition.and_then(|provider| provider.defaults.api_path.clone())
                })
                .or_else(|| match provider_str {
                    "google_translate_free" | "google_translate" => None,
                    _ if provider_str.starts_with("custom-") => {
                        let custom_strategy = llm_settings
                            .pointer(&format!("/customProviders/{provider_str}/strategy"))
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("openai_compatible");
                        match custom_strategy {
                            "openai_responses" => Some("/v1/responses".to_string()),
                            "openai_compatible" => Some("/v1/chat/completions".to_string()),
                            _ => None,
                        }
                    }
                    _ => None,
                });
            let api_version = provider_settings
                .and_then(|s| s.get("apiVersion"))
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .or_else(|| {
                    provider_definition.and_then(|provider| provider.defaults.api_version.clone())
                });

            let provider: Result<sona_core::domain::LlmProvider, _> =
                serde_json::from_value(serde_json::Value::String(provider_str.to_string()));
            if let Ok(prov) = provider {
                let default_strategy =
                    sona_core::llm::tasks::LlmProviderStrategy::from_provider(&prov);
                let strategy = llm_settings
                    .pointer(&format!("/customProviders/{provider_str}/strategy"))
                    .and_then(serde_json::Value::as_str)
                    .and_then(|strategy| {
                        serde_json::from_value(serde_json::Value::String(strategy.to_string())).ok()
                    })
                    .unwrap_or(default_strategy);
                let selections = llm_settings.get("selections");
                let feature_prefix = match feature {
                    "translation" | "translate" => "translation",
                    "summary" => "summary",
                    _ => "polish",
                };
                let temperature = selections
                    .and_then(|settings| settings.get(format!("{feature_prefix}Temperature")))
                    .and_then(serde_json::Value::as_f64)
                    .filter(|temperature| {
                        temperature.is_finite() && (0.0..=2.0).contains(temperature)
                    })
                    .map(|temperature| temperature as f32)
                    .or(Some(0.7));
                let reasoning_enabled = selections
                    .and_then(|settings| settings.get(format!("{feature_prefix}ReasoningEnabled")))
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                let reasoning_mode = model_entry
                    .pointer("/metadata/reasoningMode")
                    .and_then(|mode| mode.get("type"))
                    .and_then(serde_json::Value::as_str);
                let reasoning_level = if reasoning_mode == Some("budget") {
                    selections
                        .and_then(|settings| {
                            settings.get(format!("{feature_prefix}ReasoningBudget"))
                        })
                        .and_then(serde_json::Value::as_u64)
                        .or_else(|| {
                            model_entry
                                .pointer("/metadata/reasoningMode/defaultBudget")
                                .and_then(serde_json::Value::as_u64)
                        })
                        .map(|budget| budget.to_string())
                } else {
                    selections
                        .and_then(|settings| {
                            settings.get(format!("{feature_prefix}ReasoningLevel"))
                        })
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                };
                return Some(sona_core::llm::requests::LlmConfig {
                    strategy,
                    provider: prov,
                    base_url: api_host.to_string(),
                    api_key: api_key.to_string(),
                    model: model_str.to_string(),
                    api_path,
                    api_version,
                    temperature,
                    reasoning_enabled: Some(reasoning_enabled),
                    reasoning_level,
                    timeout_seconds: config
                        .get("llmRequestTimeoutSeconds")
                        .and_then(|v| v.as_u64()),
                });
            }
        }
    }

    // 2. Try legacy llm config
    if let Some(llm) = config.get("llm")
        && let Ok(llm_config) =
            serde_json::from_value::<sona_core::llm::requests::LlmConfig>(llm.clone())
    {
        return Some(llm_config);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::paths::{MockPathProvider, PathKind};
    use sona_sqlite::{Database, SqliteAppConfigAdapter};
    use std::cell::Cell;
    use std::collections::HashMap as StdHashMap;
    use std::path::Path as StdPath;
    use std::sync::Arc;

    fn save_config(db: Arc<Database>, config: &serde_json::Value) {
        SqliteAppConfigAdapter::new(db, Arc::new(SystemClock))
            .save_config(config)
            .unwrap();
    }

    #[test]
    fn feature_llm_config_preserves_custom_provider_and_feature_options() {
        let config = serde_json::json!({
            "llmRequestTimeoutSeconds": 240,
            "llmSettings": {
                "selections": {
                    "polishModelId": "custom-gateway-gpt-model",
                    "polishTemperature": 0.35,
                    "polishReasoningEnabled": true,
                    "polishReasoningLevel": "high"
                },
                "models": {
                    "custom-gateway-gpt-model": {
                        "provider": "custom-gateway",
                        "model": "gpt-model",
                        "metadata": {
                            "reasoningMode": { "type": "effort" }
                        }
                    }
                },
                "customProviders": {
                    "custom-gateway": { "strategy": "openai_responses" }
                },
                "providers": {
                    "custom-gateway": {
                        "apiHost": "https://llm.example.test",
                        "apiKey": "secret",
                        "apiVersion": "2025-01-01"
                    }
                }
            }
        });

        let resolved = resolve_feature_llm_config_from_app_config(&config, "polish")
            .expect("selected feature model should resolve");

        assert_eq!(resolved.provider.as_str(), "custom-gateway");
        assert_eq!(
            resolved.strategy,
            sona_core::llm::tasks::LlmProviderStrategy::OpenAiResponses
        );
        assert_eq!(resolved.base_url, "https://llm.example.test");
        assert_eq!(resolved.api_path.as_deref(), Some("/v1/responses"));
        assert_eq!(resolved.api_version.as_deref(), Some("2025-01-01"));
        assert_eq!(resolved.temperature, Some(0.35));
        assert_eq!(resolved.reasoning_enabled, Some(true));
        assert_eq!(resolved.reasoning_level.as_deref(), Some("high"));
        assert_eq!(resolved.timeout_seconds, Some(240));
    }

    #[test]
    fn feature_llm_config_maps_budget_reasoning_to_runtime_budget_level() {
        let config = serde_json::json!({
            "llmSettings": {
                "selections": {
                    "summaryModelId": "anthropic-claude",
                    "summaryReasoningEnabled": true,
                    "summaryReasoningBudget": 4096
                },
                "models": {
                    "anthropic-claude": {
                        "provider": "anthropic",
                        "model": "claude-sonnet-4",
                        "metadata": {
                            "reasoningMode": {
                                "type": "budget",
                                "defaultBudget": 2048
                            }
                        }
                    }
                },
                "providers": {
                    "anthropic": { "apiHost": "https://api.anthropic.com" }
                }
            }
        });

        let resolved = resolve_feature_llm_config_from_app_config(&config, "summary")
            .expect("selected feature model should resolve");

        assert_eq!(resolved.reasoning_enabled, Some(true));
        assert_eq!(resolved.reasoning_level.as_deref(), Some("4096"));
    }

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
    fn app_local_data_database_selector_reuses_matching_global_database() {
        let app_local_data = tempfile::tempdir().unwrap();
        let global = Arc::new(Database::open(app_local_data.path()).unwrap());
        let opened = Cell::new(false);

        let selected = database_for_app_local_data_dir_or_open(
            app_local_data.path(),
            Some(Arc::clone(&global)),
            |path| {
                opened.set(true);
                Database::open(path)
            },
        )
        .unwrap();

        assert!(Arc::ptr_eq(&selected, &global));
        assert!(!opened.get());
    }

    #[test]
    fn app_local_data_database_selector_opens_requested_path_when_global_mismatches() {
        let global_dir = tempfile::tempdir().unwrap();
        let requested_dir = tempfile::tempdir().unwrap();
        let global = Arc::new(Database::open(global_dir.path()).unwrap());
        let opened = Cell::new(false);

        let selected =
            database_for_app_local_data_dir_or_open(requested_dir.path(), Some(global), |path| {
                assert_eq!(path, requested_dir.path());
                opened.set(true);
                Database::open(path)
            })
            .unwrap();

        assert!(opened.get());
        assert!(selected.is_for_app_local_data_dir(requested_dir.path()));
    }

    #[test]
    fn load_online_asr_config_reads_sqlite_app_config() {
        let app_data = tempfile::tempdir().unwrap();
        let app_local_data = tempfile::tempdir().unwrap();
        let provider = provider_for_config_test(app_data.path(), app_local_data.path());
        let db = Database::open(app_local_data.path()).unwrap();
        save_config(
            Arc::new(db),
            &serde_json::json!({
                "asr": {
                    "providers": {
                        "online": {
                            "volcengine": {
                                "apiKey": "sqlite-key"
                            }
                        }
                    }
                }
            }),
        );
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
    fn load_online_asr_config_unwraps_sqlite_object_wrappers() {
        let app_data = tempfile::tempdir().unwrap();
        let app_local_data = tempfile::tempdir().unwrap();
        let provider = provider_for_config_test(app_data.path(), app_local_data.path());
        let db = Database::open(app_local_data.path()).unwrap();
        save_config(
            Arc::new(db),
            &serde_json::json!({
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
            }),
        );

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
        save_config(
            Arc::new(db),
            &serde_json::json!({
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
            }),
        );

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
        save_config(
            Arc::new(db),
            &serde_json::json!({
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
            }),
        );

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

    #[test]
    fn load_ffmpeg_path_falls_back_to_custom_path_when_ffmpeg_enabled_omitted() {
        let app_data = tempfile::tempdir().unwrap();
        let app_local_data = tempfile::tempdir().unwrap();
        let provider = provider_for_config_test(app_data.path(), app_local_data.path());
        let db = Database::open(app_local_data.path()).unwrap();
        save_config(
            Arc::new(db),
            &serde_json::json!({
                "ffmpegPath": "C:/custom/ffmpeg.exe"
            }),
        );

        let ffmpeg_path = load_ffmpeg_path(&provider);
        assert_eq!(ffmpeg_path.as_deref(), Some("C:/custom/ffmpeg.exe"));
    }

    #[test]
    fn load_ffmpeg_path_returns_none_when_explicitly_disabled() {
        let app_data = tempfile::tempdir().unwrap();
        let app_local_data = tempfile::tempdir().unwrap();
        let provider = provider_for_config_test(app_data.path(), app_local_data.path());
        let db = Database::open(app_local_data.path()).unwrap();
        save_config(
            Arc::new(db),
            &serde_json::json!({
                "ffmpegEnabled": false,
                "ffmpegPath": "C:/custom/ffmpeg.exe"
            }),
        );

        let ffmpeg_path = load_ffmpeg_path(&provider);
        assert_eq!(ffmpeg_path, None);
    }
}
