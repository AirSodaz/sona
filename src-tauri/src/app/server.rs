use async_trait::async_trait;
use sona_api_server::{
    ApiServerPlatform, ApiServerServiceParts, ONLINE_ASR_BATCH_UNAVAILABLE, OnlineBatchRequest,
    RunningApiServer, build_streaming_router, start_api_server_runtime,
};
use sona_core::runtime::serve::{ServeRuntimeArgs, resolve_serve_runtime_options};
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex as AsyncMutex;

pub const DESKTOP_ONLINE_ASR_BATCH_UNAVAILABLE: &str = "Online ASR batch is unavailable in the desktop app because no online ASR configuration is loaded. Start the API Server from the desktop app to use configured Online ASR providers.";

pub struct ApiServerController {
    pub running_server: Arc<AsyncMutex<Option<RunningApiServer>>>,
    pub online_asr_config: Arc<tokio::sync::RwLock<HashMap<String, serde_json::Value>>>,
}

impl Default for ApiServerController {
    fn default() -> Self {
        Self {
            running_server: Arc::new(AsyncMutex::new(None)),
            online_asr_config: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }
}

#[derive(Clone)]
pub struct TauriStreamingContext {
    pub app: Option<tauri::AppHandle>,
    pub recognizer_pool: crate::integrations::asr::RecognizerPool,
}

#[derive(Clone)]
struct TauriApiServerPlatform {
    streaming_context: Arc<TauriStreamingContext>,
}

impl TauriApiServerPlatform {
    fn from_app(app: Option<tauri::AppHandle>) -> Self {
        let recognizer_pool = app
            .as_ref()
            .map(|app| {
                app.state::<crate::integrations::asr::AsrState>()
                    .recognizer_pool
                    .clone()
            })
            .unwrap_or_else(crate::integrations::asr::RecognizerPool::new);
        Self {
            streaming_context: Arc::new(TauriStreamingContext {
                app,
                recognizer_pool,
            }),
        }
    }
}

#[async_trait]
impl ApiServerPlatform for TauriApiServerPlatform {
    async fn transcribe_online_batch(
        &self,
        request: OnlineBatchRequest,
    ) -> Result<Vec<sona_core::transcription::transcript::TranscriptSegment>, String> {
        let Some(app_handle) = self.streaming_context.app.as_ref() else {
            return Err(ONLINE_ASR_BATCH_UNAVAILABLE.to_string());
        };
        if request.config.is_null()
            || request
                .config
                .get("apiKey")
                .and_then(serde_json::Value::as_str)
                .is_none_or(str::is_empty)
        {
            return Err(DESKTOP_ONLINE_ASR_BATCH_UNAVAILABLE.to_string());
        }

        let inner_app_clone = app_handle.clone();
        let sherpa_state = app_handle.state::<crate::integrations::asr::AsrState>();
        let asr_request = sona_api_server::online_batch_request_to_core_request(&request);
        crate::commands::asr::process_batch_file(
            inner_app_clone,
            sherpa_state,
            request.file_path.to_string_lossy().to_string(),
            None,
            None,
            asr_request,
            None,
        )
        .await
        .map_err(|error| error.to_string())
    }

    async fn build_info_response(
        &self,
        models_dir: &std::path::Path,
        online_asr_config: &HashMap<String, serde_json::Value>,
    ) -> Result<sona_api_server::InfoResponse, String> {
        sona_api_server::default_info_response(models_dir, online_asr_config).await
    }

    fn streaming_context(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        Some(self.streaming_context.clone())
    }
}

pub async fn refresh_online_asr_config(
    controller: &ApiServerController,
    config_map: HashMap<String, serde_json::Value>,
) {
    *controller.online_asr_config.write().await = config_map;
}

#[allow(clippy::too_many_arguments)]
pub async fn start_api_server(
    app: tauri::AppHandle,
    controller: tauri::State<'_, ApiServerController>,
    host: String,
    port: u16,
    api_key: String,
    max_concurrent: usize,
    max_queue_size: usize,
    max_upload_size_mb: usize,
    job_ttl_minutes: u64,
    max_streaming: usize,
    ip_whitelist: String,
    gpu_acceleration: String,
) -> Result<String, String> {
    let runtime_dirs =
        crate::platform::api_server_runtime::resolve_api_server_runtime_dirs_for_app(&app)?;
    let temp_dir = runtime_dirs.temp_dir;
    let models_dir = runtime_dirs.models_dir;

    let online_asr_config = controller.online_asr_config.clone();
    let platform = Arc::new(TauriApiServerPlatform::from_app(Some(app.clone())));
    let resolved = resolve_serve_runtime_options(
        ServeRuntimeArgs {
            host: Some(host),
            port: Some(port),
            api_key: Some(api_key),
            default_models_dir: Some(models_dir),
            ip_whitelist: Some(ip_whitelist),
            max_streaming: Some(max_streaming),
            max_concurrent: Some(max_concurrent),
            max_queue_size: Some(max_queue_size),
            max_upload_size_mb: Some(max_upload_size_mb),
            job_ttl_minutes: Some(job_ttl_minutes),
            gpu_acceleration: Some(gpu_acceleration),
            ..Default::default()
        },
        None,
    )?;

    let previous_server = {
        let mut server_lock = controller.running_server.lock().await;
        server_lock.take()
    };
    if let Some(server) = previous_server {
        if let Err(error) = server.stop().await {
            log::warn!("Previous HTTP API Server stopped with error: {}", error);
        }
    }

    refresh_online_asr_config(
        &controller,
        crate::platform::api_server_config::load_online_asr_config_for_app(&app),
    )
    .await;

    let running_server = start_api_server_runtime(ApiServerServiceParts {
        resolved,
        temp_dir,
        online_asr_config,
        platform,
        streaming_router: Some(build_streaming_router(
            crate::integrations::streaming::handle_streaming,
        )),
    })
    .await
    .map_err(|error| error.to_string())?;
    let normalized_whitelist = running_server.normalized_ip_whitelist.clone();
    *controller.running_server.lock().await = Some(running_server);

    Ok(normalized_whitelist)
}

pub async fn stop_api_server(
    controller: tauri::State<'_, ApiServerController>,
) -> Result<(), String> {
    let running_server = {
        let mut server_lock = controller.running_server.lock().await;
        server_lock.take()
    };
    if let Some(server) = running_server {
        server.stop().await?;
        log::info!("Sent shutdown signal to API server.");
    }
    Ok(())
}

pub fn start_from_app_handle(app_handle: &tauri::AppHandle) {
    let app_handle = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        let settings = crate::platform::api_server_config::load_api_server_startup_settings_for_app(
            &app_handle,
        );

        if settings.enabled {
            let runtime_dirs =
                match crate::platform::api_server_runtime::resolve_api_server_runtime_dirs_for_app(
                    &app_handle,
                ) {
                    Ok(dirs) => dirs,
                    Err(e) => {
                        log::error!("Failed to get app_local_data_dir: {}", e);
                        return;
                    }
                };
            let temp_dir = runtime_dirs.temp_dir;
            let models_dir = runtime_dirs.models_dir;
            let resolved = match resolve_serve_runtime_options(
                ServeRuntimeArgs {
                    default_models_dir: Some(models_dir),
                    ..Default::default()
                },
                Some(settings.config),
            ) {
                Ok(resolved) => resolved,
                Err(e) => {
                    log::error!("HTTP API Server failed to resolve startup settings: {}", e);
                    return;
                }
            };

            let controller = app_handle.state::<ApiServerController>();
            let online_asr_config = controller.online_asr_config.clone();
            let platform = Arc::new(TauriApiServerPlatform::from_app(Some(app_handle.clone())));
            refresh_online_asr_config(
                &controller,
                crate::platform::api_server_config::load_online_asr_config_for_app(&app_handle),
            )
            .await;

            let running_server = match start_api_server_runtime(ApiServerServiceParts {
                resolved,
                temp_dir,
                online_asr_config,
                platform,
                streaming_router: Some(build_streaming_router(
                    crate::integrations::streaming::handle_streaming,
                )),
            })
            .await
            {
                Ok(running_server) => running_server,
                Err(e) => {
                    log::error!("HTTP API Server failed: {}", e);
                    return;
                }
            };

            *controller.running_server.lock().await = Some(running_server);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::api_server_config::{
        load_api_server_startup_settings, load_online_asr_config,
    };
    use crate::platform::paths::{MockPathProvider, PathKind};
    use sona_sqlite::Database;
    use sona_sqlite::config_store::SqliteConfigStore;
    use std::collections::HashMap as StdHashMap;
    use std::path::Path as StdPath;

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

    #[tokio::test]
    async fn refresh_online_asr_config_updates_controller_before_server_start() {
        let controller = ApiServerController::default();
        let mut config_map = StdHashMap::new();
        config_map.insert(
            "volcengine".to_string(),
            serde_json::json!({
                "apiKey": "sqlite-key"
            }),
        );

        refresh_online_asr_config(&controller, config_map).await;

        let config = controller.online_asr_config.read().await;
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
