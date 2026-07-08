use async_trait::async_trait;
use axum::{Router, routing::get};
use sona_api_server::{
    ApiServerPlatform, ApiServerRuntimeConfig, ApiServerTranscriptionDefaults,
    ONLINE_ASR_BATCH_UNAVAILABLE, OnlineBatchRequest, parse_ip_whitelist, run_server,
};
use sona_core::gpu::{DEFAULT_GPU_ACCELERATION, resolve_gpu_acceleration};
use sona_sqlite::{Database, DatabaseError};
use std::any::Any;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex as AsyncMutex;

use crate::platform::paths::{PathKind, PathProvider, TauriPathProvider};

pub const DESKTOP_ONLINE_ASR_BATCH_UNAVAILABLE: &str = "Online ASR batch is unavailable in the desktop app because no online ASR configuration is loaded. Start the API Server from the desktop app to use configured Online ASR providers.";

pub struct ApiServerController {
    pub shutdown_sender: Arc<AsyncMutex<Option<tokio::sync::oneshot::Sender<()>>>>,
    pub online_asr_config: Arc<tokio::sync::RwLock<HashMap<String, serde_json::Value>>>,
}

impl Default for ApiServerController {
    fn default() -> Self {
        Self {
            shutdown_sender: Arc::new(AsyncMutex::new(None)),
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
    ) -> Result<Vec<sona_core::transcript::TranscriptSegment>, String> {
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct ApiServerStartupSettings {
    http_server_enabled: bool,
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
}

impl Default for ApiServerStartupSettings {
    fn default() -> Self {
        Self {
            http_server_enabled: false,
            host: "127.0.0.1".to_string(),
            port: 14200,
            api_key: String::new(),
            max_concurrent: 2,
            max_queue_size: 100,
            max_upload_size_mb: 50,
            job_ttl_minutes: 60,
            max_streaming: 2,
            ip_whitelist: "localhost".to_string(),
            gpu_acceleration: DEFAULT_GPU_ACCELERATION.to_string(),
        }
    }
}

fn extract_app_config_value(value: serde_json::Value) -> serde_json::Value {
    value
        .get("sona-config")
        .filter(|value| value.is_object())
        .or_else(|| value.get("sona_config"))
        .filter(|value| value.is_object())
        .or_else(|| value.get("config"))
        .filter(|value| value.is_object())
        .cloned()
        .unwrap_or(value)
}

fn read_sqlite_app_config(db: &Database) -> Result<Option<serde_json::Value>, DatabaseError> {
    db.with_connection(|conn| {
        let mut stmt = conn.prepare_cached("SELECT config FROM app_config WHERE id = 1")?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let config_str: String = row.get(0)?;
            let config: serde_json::Value = serde_json::from_str(&config_str)?;
            Ok(Some(extract_app_config_value(config)))
        } else {
            Ok(None)
        }
    })
}

fn read_sqlite_api_server_startup_settings(
    db: &Database,
) -> Result<Option<ApiServerStartupSettings>, DatabaseError> {
    db.with_connection(|conn| {
        let mut stmt = conn.prepare_cached(
            "SELECT http_server_enabled, http_server_host, http_server_port,
                    http_server_api_key, http_server_max_concurrent,
                    http_server_max_queue_size, http_server_max_upload_size_mb,
                    http_server_job_ttl_minutes, http_server_max_streaming,
                    http_server_ip_whitelist, gpu_acceleration
             FROM app_config WHERE id = 1",
        )?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let defaults = ApiServerStartupSettings::default();
            Ok(Some(ApiServerStartupSettings {
                http_server_enabled: row.get::<_, i64>(0)? != 0,
                host: row.get(1)?,
                port: u16::try_from(row.get::<_, i64>(2)?).unwrap_or(defaults.port),
                api_key: row.get(3)?,
                max_concurrent: usize::try_from(row.get::<_, i64>(4)?)
                    .unwrap_or(defaults.max_concurrent),
                max_queue_size: usize::try_from(row.get::<_, i64>(5)?)
                    .unwrap_or(defaults.max_queue_size),
                max_upload_size_mb: usize::try_from(row.get::<_, i64>(6)?)
                    .unwrap_or(defaults.max_upload_size_mb),
                job_ttl_minutes: u64::try_from(row.get::<_, i64>(7)?)
                    .unwrap_or(defaults.job_ttl_minutes),
                max_streaming: usize::try_from(row.get::<_, i64>(8)?)
                    .unwrap_or(defaults.max_streaming),
                ip_whitelist: row.get(9)?,
                gpu_acceleration: row.get(10)?,
            }))
        } else {
            Ok(None)
        }
    })
}

fn load_sqlite_app_config(provider: &dyn PathProvider) -> Option<serde_json::Value> {
    let app_local_data_dir = provider.resolve_path(PathKind::AppLocalData).ok()?;
    if let Ok(db) = Database::global()
        && db.is_for_app_local_data_dir(&app_local_data_dir)
    {
        return read_sqlite_app_config(db)
            .map_err(|error| {
                log::warn!("[API Server] Failed to load SQLite app config: {error}");
                error
            })
            .ok()
            .flatten();
    }

    let db = match Database::open(&app_local_data_dir) {
        Ok(db) => db,
        Err(error) => {
            log::warn!("[API Server] Failed to open SQLite config store: {error}");
            return None;
        }
    };

    read_sqlite_app_config(&db)
        .map_err(|error| {
            log::warn!("[API Server] Failed to load SQLite app config: {error}");
            error
        })
        .ok()
        .flatten()
}

fn load_sqlite_api_server_startup_settings(
    provider: &dyn PathProvider,
) -> Option<ApiServerStartupSettings> {
    let app_local_data_dir = provider.resolve_path(PathKind::AppLocalData).ok()?;
    if let Ok(db) = Database::global()
        && db.is_for_app_local_data_dir(&app_local_data_dir)
    {
        return read_sqlite_api_server_startup_settings(db)
            .map_err(|error| {
                log::warn!("[API Server] Failed to load SQLite startup settings: {error}");
                error
            })
            .ok()
            .flatten();
    }

    let db = match Database::open(&app_local_data_dir) {
        Ok(db) => db,
        Err(error) => {
            log::warn!("[API Server] Failed to open SQLite config store: {error}");
            return None;
        }
    };

    read_sqlite_api_server_startup_settings(&db)
        .map_err(|error| {
            log::warn!("[API Server] Failed to load SQLite startup settings: {error}");
            error
        })
        .ok()
        .flatten()
}

fn load_legacy_settings_config(provider: &dyn PathProvider) -> Option<serde_json::Value> {
    let app_data_dir = provider.resolve_path(PathKind::AppData).ok()?;
    let settings_path = app_data_dir.join("settings.json");
    if !settings_path.exists() {
        return None;
    }
    let contents = std::fs::read_to_string(&settings_path).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&contents)
        .map_err(|error| {
            log::warn!(
                "[API Server] Failed to parse legacy settings {}: {error}",
                settings_path.display()
            );
            error
        })
        .ok()?;
    Some(extract_app_config_value(parsed))
}

fn load_app_config_for_server(provider: &dyn PathProvider) -> Option<serde_json::Value> {
    load_sqlite_app_config(provider).or_else(|| load_legacy_settings_config(provider))
}

pub fn load_online_asr_config(provider: &dyn PathProvider) -> HashMap<String, serde_json::Value> {
    load_app_config_for_server(provider)
        .and_then(|config| {
            config
                .get("asr")
                .and_then(|v| v.get("providers"))
                .and_then(|v| v.get("online"))
                .and_then(|v| v.as_object())
                .cloned()
        })
        .unwrap_or_default()
        .into_iter()
        .collect()
}

pub async fn refresh_online_asr_config(
    controller: &ApiServerController,
    provider: &dyn PathProvider,
) {
    let config_map = load_online_asr_config(provider);
    *controller.online_asr_config.write().await = config_map;
}

fn load_api_server_startup_settings(provider: &dyn PathProvider) -> ApiServerStartupSettings {
    if let Some(settings) = load_sqlite_api_server_startup_settings(provider) {
        return settings;
    }
    load_app_config_for_server(provider)
        .map(|config| startup_settings_from_config(&config))
        .unwrap_or_default()
}

fn startup_settings_from_config(config: &serde_json::Value) -> ApiServerStartupSettings {
    let mut settings = ApiServerStartupSettings::default();

    if let Some(enabled) = config.get("httpServerEnabled").and_then(|v| v.as_bool()) {
        settings.http_server_enabled = enabled;
    }
    if let Some(host) = config.get("httpServerHost").and_then(|v| v.as_str()) {
        settings.host = host.to_string();
    }
    if let Some(port) = config.get("httpServerPort").and_then(|v| v.as_u64())
        && let Ok(port) = u16::try_from(port)
    {
        settings.port = port;
    }
    if let Some(api_key) = config.get("httpServerApiKey").and_then(|v| v.as_str()) {
        settings.api_key = api_key.to_string();
    }
    if let Some(max_concurrent) = config
        .get("httpServerMaxConcurrent")
        .and_then(|v| v.as_u64())
        && let Ok(max_concurrent) = usize::try_from(max_concurrent)
    {
        settings.max_concurrent = max_concurrent;
    }
    if let Some(max_queue_size) = config
        .get("httpServerMaxQueueSize")
        .and_then(|v| v.as_u64())
        && let Ok(max_queue_size) = usize::try_from(max_queue_size)
    {
        settings.max_queue_size = max_queue_size;
    }
    if let Some(max_upload_size) = config
        .get("httpServerMaxUploadSizeMB")
        .and_then(|v| v.as_u64())
        && let Ok(max_upload_size) = usize::try_from(max_upload_size)
    {
        settings.max_upload_size_mb = max_upload_size;
    }
    if let Some(job_ttl) = config
        .get("httpServerJobTtlMinutes")
        .and_then(|v| v.as_u64())
    {
        settings.job_ttl_minutes = job_ttl;
    }
    if let Some(max_streaming) = config
        .get("httpServerMaxStreaming")
        .and_then(|v| v.as_u64())
        && let Ok(max_streaming) = usize::try_from(max_streaming)
    {
        settings.max_streaming = max_streaming;
    }
    if let Some(ip_whitelist) = config.get("httpServerIpWhitelist").and_then(|v| v.as_str()) {
        settings.ip_whitelist = ip_whitelist.to_string();
    }
    if let Some(gpu_acceleration) = config.get("gpuAcceleration").and_then(|v| v.as_str()) {
        settings.gpu_acceleration = gpu_acceleration.to_string();
    }

    settings
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
    let parsed_whitelist = parse_ip_whitelist(&ip_whitelist)?;
    let normalized_whitelist = parsed_whitelist
        .iter()
        .map(|net| net.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let parsed_arc = Arc::new(parsed_whitelist);
    let transcription_defaults = ApiServerTranscriptionDefaults {
        gpu_acceleration: resolve_gpu_acceleration(Some(gpu_acceleration))
            .map_err(|e| e.to_string())?,
        ..Default::default()
    };

    let mut sender_lock = controller.shutdown_sender.lock().await;
    if let Some(sender) = sender_lock.take() {
        let _ = sender.send(());
    }

    let (tx, rx) = tokio::sync::oneshot::channel();
    *sender_lock = Some(tx);
    drop(sender_lock);

    let (bind_tx, bind_rx) = tokio::sync::oneshot::channel();

    let path_provider = TauriPathProvider::from_app(&app);
    let app_local_data_dir = path_provider.resolve_path(PathKind::AppLocalData)?;
    let temp_dir = app_local_data_dir.join("api_temp");
    let models_dir = app_local_data_dir.join("models");

    refresh_online_asr_config(&controller, &path_provider).await;
    let online_asr_config = controller.online_asr_config.clone();
    let platform = Arc::new(TauriApiServerPlatform::from_app(Some(app.clone())));

    tauri::async_runtime::spawn(async move {
        if let Err(e) = run_server(ApiServerRuntimeConfig {
            host,
            port,
            api_key,
            temp_dir,
            models_dir,
            max_concurrent,
            max_queue_size,
            max_upload_size_mb,
            job_ttl_minutes,
            max_streaming,
            ip_whitelist: parsed_arc,
            online_asr_config,
            transcription_defaults,
            platform,
            streaming_router: Some(streaming_router()),
            shutdown_rx: rx,
            bind_tx: Some(bind_tx),
        })
        .await
        {
            log::error!("HTTP API Server failed: {}", e);
        }
    });

    match bind_rx.await {
        Ok(Ok(())) => Ok(normalized_whitelist),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("API Server failed to start: task terminated prematurely".to_string()),
    }
}

pub async fn stop_api_server(
    controller: tauri::State<'_, ApiServerController>,
) -> Result<(), String> {
    let mut sender_lock = controller.shutdown_sender.lock().await;
    if let Some(sender) = sender_lock.take() {
        let _ = sender.send(());
        log::info!("Sent shutdown signal to API server.");
    }
    Ok(())
}

pub fn start_from_app_handle(app_handle: &tauri::AppHandle) {
    let app_handle = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        let path_provider = TauriPathProvider::from_app(&app_handle);
        let settings = load_api_server_startup_settings(&path_provider);

        if settings.http_server_enabled {
            let app_local_data_dir = match path_provider.resolve_path(PathKind::AppLocalData) {
                Ok(dir) => dir,
                Err(e) => {
                    log::error!("Failed to get app_local_data_dir: {}", e);
                    return;
                }
            };
            let temp_dir = app_local_data_dir.join("api_temp");
            let models_dir = app_local_data_dir.join("models");

            let parsed_whitelist = match parse_ip_whitelist(&settings.ip_whitelist) {
                Ok(nets) => nets,
                Err(e) => {
                    log::error!(
                        "HTTP API Server failed to start due to invalid IP whitelist: {}",
                        e
                    );
                    return;
                }
            };
            let transcription_defaults =
                match resolve_gpu_acceleration(Some(settings.gpu_acceleration)) {
                    Ok(gpu_acceleration) => ApiServerTranscriptionDefaults {
                        gpu_acceleration,
                        ..Default::default()
                    },
                    Err(e) => {
                        log::error!(
                            "HTTP API Server failed to start due to invalid GPU acceleration: {}",
                            e
                        );
                        return;
                    }
                };

            let (tx, rx) = tokio::sync::oneshot::channel();
            let controller = app_handle.state::<ApiServerController>();
            *controller.shutdown_sender.lock().await = Some(tx);

            refresh_online_asr_config(&controller, &path_provider).await;
            let online_asr_config = controller.online_asr_config.clone();
            let platform = Arc::new(TauriApiServerPlatform::from_app(Some(app_handle.clone())));

            if let Err(e) = run_server(ApiServerRuntimeConfig {
                host: settings.host,
                port: settings.port,
                api_key: settings.api_key,
                temp_dir,
                models_dir,
                max_concurrent: settings.max_concurrent,
                max_queue_size: settings.max_queue_size,
                max_upload_size_mb: settings.max_upload_size_mb,
                job_ttl_minutes: settings.job_ttl_minutes,
                max_streaming: settings.max_streaming,
                ip_whitelist: Arc::new(parsed_whitelist),
                online_asr_config,
                transcription_defaults,
                platform,
                streaming_router: Some(streaming_router()),
                shutdown_rx: rx,
                bind_tx: None,
            })
            .await
            {
                log::error!("HTTP API Server failed: {}", e);
            }
        }
    });
}

fn streaming_router() -> Router<sona_api_server::ServerState> {
    Router::new().route(
        "/v1/streaming",
        get(crate::integrations::streaming::handle_streaming),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::paths::{MockPathProvider, PathKind};
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

    #[tokio::test]
    async fn refresh_online_asr_config_updates_controller_before_server_start() {
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
        let controller = ApiServerController::default();

        refresh_online_asr_config(&controller, &provider).await;

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

        assert!(settings.http_server_enabled);
        assert_eq!(settings.host, "0.0.0.0");
        assert_eq!(settings.port, 15555);
        assert_eq!(settings.api_key, "sqlite-secret");
        assert_eq!(settings.max_concurrent, 3);
        assert_eq!(settings.max_queue_size, 12);
        assert_eq!(settings.max_upload_size_mb, 99);
        assert_eq!(settings.job_ttl_minutes, 7);
        assert_eq!(settings.max_streaming, 4);
        assert_eq!(settings.ip_whitelist, "127.0.0.1/32");
        assert_eq!(settings.gpu_acceleration, "cpu");
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

        assert!(settings.http_server_enabled);
        assert_eq!(settings.host, "0.0.0.0");
        assert_eq!(settings.port, 16666);
        assert_eq!(settings.api_key, "column-secret");
        assert_eq!(settings.max_concurrent, 5);
        assert_eq!(settings.max_queue_size, 44);
        assert_eq!(settings.max_upload_size_mb, 256);
        assert_eq!(settings.job_ttl_minutes, 9);
        assert_eq!(settings.max_streaming, 7);
        assert_eq!(settings.ip_whitelist, "10.0.0.0/8");
        assert_eq!(settings.gpu_acceleration, "cuda");
    }
}
