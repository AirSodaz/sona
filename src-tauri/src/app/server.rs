use axum::{
    Json, Router,
    extract::{ConnectInfo, Multipart, Path, Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
    routing::{get, post},
};
use futures_util::stream::StreamExt;
use hmac::{Hmac, KeyInit, Mac};
use ipnet::IpNet;
use sha2::Sha256;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path as StdPath, PathBuf};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::{RwLock, mpsc};
use tower_http::{
    cors::{Any, CorsLayer},
    validate_request::ValidateRequestHeaderLayer,
};
type HmacSha256 = Hmac<Sha256>;

use tauri::Manager;
use tokio::sync::Mutex as AsyncMutex;

pub const CLI_ONLINE_ASR_BATCH_UNAVAILABLE: &str = "Cloud ASR batch is unavailable in sona serve because no desktop online ASR configuration is loaded. Start the API Server from the desktop app to use configured Cloud ASR providers.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiServerTranscriptionDefaults {
    pub gpu_acceleration: Option<String>,
    pub vad_model_id: Option<String>,
    pub punctuation_model_id: Option<String>,
}

impl Default for ApiServerTranscriptionDefaults {
    fn default() -> Self {
        Self {
            gpu_acceleration: Some(crate::cli::DEFAULT_GPU_ACCELERATION.to_string()),
            vad_model_id: Some(crate::core::preset_models::DEFAULT_SILERO_VAD_MODEL_ID.to_string()),
            punctuation_model_id: Some(
                crate::core::preset_models::DEFAULT_PUNCTUATION_MODEL_ID.to_string(),
            ),
        }
    }
}

pub struct ApiServerRuntimeConfig {
    pub app: Option<tauri::AppHandle>,
    pub host: String,
    pub port: u16,
    pub api_key: String,
    pub temp_dir: PathBuf,
    pub models_dir: PathBuf,
    pub max_concurrent: usize,
    pub max_queue_size: usize,
    pub max_upload_size_mb: usize,
    pub job_ttl_minutes: u64,
    pub max_streaming: usize,
    pub ip_whitelist: Arc<Vec<IpNet>>,
    pub online_asr_config: Arc<tokio::sync::RwLock<HashMap<String, serde_json::Value>>>,
    pub transcription_defaults: ApiServerTranscriptionDefaults,
    pub shutdown_rx: tokio::sync::oneshot::Receiver<()>,
    pub bind_tx: Option<tokio::sync::oneshot::Sender<Result<(), String>>>,
}

pub struct ApiServerController {
    pub shutdown_sender: std::sync::Arc<AsyncMutex<Option<tokio::sync::oneshot::Sender<()>>>>,
    pub online_asr_config:
        std::sync::Arc<tokio::sync::RwLock<std::collections::HashMap<String, serde_json::Value>>>,
}

impl Default for ApiServerController {
    fn default() -> Self {
        Self {
            shutdown_sender: std::sync::Arc::new(AsyncMutex::new(None)),
            online_asr_config: std::sync::Arc::new(tokio::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
        }
    }
}

pub fn load_online_asr_config(
    app: &tauri::AppHandle,
) -> std::collections::HashMap<String, serde_json::Value> {
    let mut online_asr_config = std::collections::HashMap::new();
    if let Ok(data_dir) = crate::app::paths::resolve_app_data_dir(app) {
        let config_path = data_dir.join("settings.json");
        match std::fs::read_to_string(&config_path) {
            Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(json) => {
                    if let Some(config) = json
                        .get("asr")
                        .and_then(|v| v.get("providers"))
                        .and_then(|v| v.get("online"))
                        && let Some(map) = config.as_object()
                    {
                        for (k, v) in map {
                            online_asr_config.insert(k.clone(), v.clone());
                        }
                    }
                }
                Err(e) => log::error!("Failed to parse settings.json: {}", e),
            },
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    log::warn!("Failed to read settings.json: {}", e);
                }
            }
        }
    }
    online_asr_config
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
    let parsed_arc = std::sync::Arc::new(parsed_whitelist);
    let transcription_defaults = ApiServerTranscriptionDefaults {
        gpu_acceleration: crate::cli::resolve_cli_gpu_acceleration(Some(gpu_acceleration))
            .map_err(|e| e.to_string())?,
        ..Default::default()
    };

    let mut sender_lock = controller.shutdown_sender.lock().await;

    // Stop existing server if running
    if let Some(sender) = sender_lock.take() {
        let _ = sender.send(());
    }

    let (tx, rx) = tokio::sync::oneshot::channel();
    *sender_lock = Some(tx);
    drop(sender_lock);

    let (bind_tx, bind_rx) = tokio::sync::oneshot::channel();

    let app_local_data_dir = crate::app::paths::resolve_app_local_data_dir(&app)?;
    let temp_dir = app_local_data_dir.join("api_temp");
    let models_dir = app_local_data_dir.join("models");

    let new_config = load_online_asr_config(&app);
    *controller.online_asr_config.write().await = new_config;
    let online_asr_config = controller.online_asr_config.clone();

    tauri::async_runtime::spawn(async move {
        if let Err(e) = run_server(ApiServerRuntimeConfig {
            app: Some(app.clone()),
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
        let app_data_dir = match crate::app::paths::resolve_app_data_dir(&app_handle) {
            Ok(dir) => dir,
            Err(e) => {
                log::error!("Failed to get app_data_dir: {}", e);
                return;
            }
        };
        let config_path = app_data_dir.join("settings.json");
        let mut http_server_enabled = false;
        let mut host = "127.0.0.1".to_string();
        let mut port = 14200;
        let mut api_key = "".to_string();
        let mut max_concurrent = 2;
        let mut max_queue_size = 100;
        let mut max_upload_size_mb = 50;
        let mut job_ttl_minutes = 60;
        let mut max_streaming = 2;
        let mut ip_whitelist = "localhost".to_string();
        let mut gpu_acceleration = crate::cli::DEFAULT_GPU_ACCELERATION.to_string();

        if let Ok(content) = std::fs::read_to_string(&config_path)
            && let Ok(json) = serde_json::from_str::<serde_json::Value>(&content)
            && let Some(config) = json.get("sona-config")
        {
            if let Some(enabled) = config.get("httpServerEnabled").and_then(|v| v.as_bool()) {
                http_server_enabled = enabled;
            }
            if let Some(h) = config.get("httpServerHost").and_then(|v| v.as_str()) {
                host = h.to_string();
            }
            if let Some(p) = config.get("httpServerPort").and_then(|v| v.as_u64()) {
                port = p as u16;
            }
            if let Some(key) = config.get("httpServerApiKey").and_then(|v| v.as_str()) {
                api_key = key.to_string();
            }
            if let Some(mc) = config
                .get("httpServerMaxConcurrent")
                .and_then(|v| v.as_u64())
            {
                max_concurrent = mc as usize;
            }
            if let Some(mq) = config
                .get("httpServerMaxQueueSize")
                .and_then(|v| v.as_u64())
            {
                max_queue_size = mq as usize;
            }
            if let Some(ms) = config
                .get("httpServerMaxUploadSizeMB")
                .and_then(|v| v.as_u64())
            {
                max_upload_size_mb = ms as usize;
            }
            if let Some(ttl) = config
                .get("httpServerJobTtlMinutes")
                .and_then(|v| v.as_u64())
            {
                job_ttl_minutes = ttl;
            }
            if let Some(ms) = config
                .get("httpServerMaxStreaming")
                .and_then(|v| v.as_u64())
            {
                max_streaming = ms as usize;
            }
            if let Some(ip_list) = config.get("httpServerIpWhitelist").and_then(|v| v.as_str()) {
                ip_whitelist = ip_list.to_string();
            }
            if let Some(gpu) = config.get("gpuAcceleration").and_then(|v| v.as_str()) {
                gpu_acceleration = gpu.to_string();
            }
        }

        if http_server_enabled {
            let app_local_data_dir =
                match crate::app::paths::resolve_app_local_data_dir(&app_handle) {
                    Ok(dir) => dir,
                    Err(e) => {
                        log::error!("Failed to get app_local_data_dir: {}", e);
                        return;
                    }
                };
            let temp_dir = app_local_data_dir.join("api_temp");
            let models_dir = app_local_data_dir.join("models");

            let parsed_whitelist = match parse_ip_whitelist(&ip_whitelist) {
                Ok(nets) => nets,
                Err(e) => {
                    log::error!(
                        "HTTP API Server failed to start due to invalid IP whitelist: {}",
                        e
                    );
                    return;
                }
            };
            let parsed_arc = std::sync::Arc::new(parsed_whitelist);
            let transcription_defaults =
                match crate::cli::resolve_cli_gpu_acceleration(Some(gpu_acceleration)) {
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

            let online_asr_config = controller.online_asr_config.clone();

            if let Err(e) = run_server(ApiServerRuntimeConfig {
                app: Some(app_handle.clone()),
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

use crate::cli::{TranscribeCliOptions, resolve_transcribe_options};
use crate::integrations::asr::transcribe_batch_with_progress;

#[derive(Debug, Clone, serde::Serialize)]
pub enum JobStatus {
    Pending,
    Processing,
    Completed(Vec<crate::integrations::asr::TranscriptSegment>),
    Failed(String),
}

#[derive(Clone)]
pub struct TranscriptionJob {
    pub job_id: String,
    pub file_path: PathBuf,
    pub model_id: String,
    pub language: String,
    pub hotwords: Option<String>,
    pub webhook_url: Option<String>,
    pub webhook_secret: Option<String>,
    pub engine: String,
    pub online_provider_id: Option<String>,
    pub online_provider_config: Option<serde_json::Value>,
}

#[derive(Clone)]
pub struct JobEntry {
    pub status: JobStatus,
    pub completed_at: Option<std::time::Instant>,
}

#[derive(Clone)]
pub struct JobManager {
    jobs: Arc<RwLock<HashMap<String, JobEntry>>>,
    sender: mpsc::Sender<TranscriptionJob>,
}

impl JobManager {
    pub fn new(sender: mpsc::Sender<TranscriptionJob>) -> Self {
        Self {
            jobs: Arc::new(RwLock::new(HashMap::new())),
            sender,
        }
    }

    pub async fn submit_job(&self, job: TranscriptionJob) -> Result<(), String> {
        self.jobs.write().await.insert(
            job.job_id.clone(),
            JobEntry {
                status: JobStatus::Pending,
                completed_at: None,
            },
        );
        self.sender.send(job).await.map_err(|e| e.to_string())
    }

    pub async fn update_job(&self, job_id: &str, status: JobStatus) {
        if let Some(job) = self.jobs.write().await.get_mut(job_id) {
            let is_finished = matches!(status, JobStatus::Completed(_) | JobStatus::Failed(_));
            job.status = status;
            if is_finished {
                job.completed_at = Some(std::time::Instant::now());
            }
        }
    }

    pub async fn get_job(&self, job_id: &str) -> Option<JobStatus> {
        self.jobs
            .read()
            .await
            .get(job_id)
            .map(|entry| entry.status.clone())
    }

    pub async fn list_jobs(&self) -> HashMap<String, JobStatus> {
        self.jobs
            .read()
            .await
            .iter()
            .map(|(k, v)| (k.clone(), v.status.clone()))
            .collect()
    }

    pub async fn clean_expired_jobs(&self, ttl_duration: std::time::Duration) {
        self.jobs.write().await.retain(|_, entry| {
            if let Some(completed_at) = entry.completed_at {
                completed_at.elapsed() <= ttl_duration
            } else {
                true
            }
        });
    }
}

async fn send_webhook(job: &TranscriptionJob, status: &JobStatus) {
    let Some(webhook_url) = &job.webhook_url else {
        return;
    };
    if webhook_url.is_empty() {
        return;
    }

    let mut payload = serde_json::Map::new();
    payload.insert(
        "job_id".to_string(),
        serde_json::Value::String(job.job_id.clone()),
    );

    match status {
        JobStatus::Completed(segments) => {
            payload.insert(
                "status".to_string(),
                serde_json::Value::String("Completed".to_string()),
            );
            payload.insert(
                "segments".to_string(),
                serde_json::to_value(segments).unwrap_or_default(),
            );
        }
        JobStatus::Failed(error) => {
            payload.insert(
                "status".to_string(),
                serde_json::Value::String("Failed".to_string()),
            );
            payload.insert(
                "error".to_string(),
                serde_json::Value::String(error.clone()),
            );
        }
        _ => return, // Only send on completion or failure
    }

    let payload_str = serde_json::to_string(&payload).unwrap_or_default();

    static WEBHOOK_CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    let client = WEBHOOK_CLIENT.get_or_init(reqwest::Client::new);

    let mut request = client
        .post(webhook_url)
        .header("Content-Type", "application/json");

    if let Some(secret) = &job.webhook_secret
        && !secret.is_empty()
        && let Ok(mut mac) = HmacSha256::new_from_slice(secret.as_bytes())
    {
        mac.update(payload_str.as_bytes());
        let result = mac.finalize().into_bytes();
        let hex_signature = hex::encode(result);
        request = request.header("X-Sona-Signature", format!("sha256={}", hex_signature));
    }

    match request.body(payload_str).send().await {
        Ok(response) => {
            if !response.status().is_success() {
                log::warn!(
                    "[Server] webhook delivery failed: job_id={} url={} status={}",
                    job.job_id,
                    webhook_url,
                    response.status()
                );
            }
        }
        Err(error) => {
            log::warn!(
                "[Server] webhook delivery error: job_id={} url={} error={}",
                job.job_id,
                webhook_url,
                error
            );
        }
    }
}

async fn start_worker_loop(
    mut receiver: mpsc::Receiver<TranscriptionJob>,
    job_manager: JobManager,
    models_dir: PathBuf,
    max_concurrent: usize,
    app: Option<tauri::AppHandle>,
    transcription_defaults: ApiServerTranscriptionDefaults,
) {
    let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrent));

    while let Some(job) = receiver.recv().await {
        let job_manager = job_manager.clone();
        let models_dir = models_dir.clone();
        let semaphore = semaphore.clone();
        let app_for_spawn = app.clone();
        let defaults = transcription_defaults.clone();

        tokio::spawn(async move {
            let app = app_for_spawn;
            let _permit = match semaphore.acquire().await {
                Ok(permit) => permit,
                Err(_) => {
                    log::error!("[Server] semaphore closed, job {} abandoned", job.job_id);
                    job_manager
                        .update_job(
                            &job.job_id,
                            JobStatus::Failed("Internal: worker pool closed".to_string()),
                        )
                        .await;
                    return;
                }
            };
            job_manager
                .update_job(&job.job_id, JobStatus::Processing)
                .await;

            let final_status = if job.engine == "Online" {
                if let Some(provider_id) = job.online_provider_id.clone() {
                    let request = crate::integrations::asr::AsrTranscriptionRequest {
                        engine_config: crate::integrations::asr::AsrEngineConfig::Online {
                            provider: crate::integrations::asr::OnlineAsrProviderRequest {
                                provider_id,
                                profile_id: job.model_id.clone(),
                                config: job.online_provider_config.clone().unwrap_or_default(),
                            },
                        },
                        mode: crate::integrations::asr::AsrMode::Offline,
                        enable_itn: false,
                        language: if job.language == "auto" {
                            "".to_string()
                        } else {
                            job.language.clone()
                        },
                        hotwords: job.hotwords.clone(),
                        normalization_options: Default::default(),
                        postprocess_options: Default::default(),
                    };
                    if let Some(app_handle) = app.as_ref() {
                        use tauri::Manager;
                        let inner_app_clone = app_handle.clone();
                        let sherpa_state = app_handle.state::<crate::integrations::asr::AsrState>();
                        match crate::commands::asr::process_batch_file(
                            inner_app_clone,
                            sherpa_state,
                            job.file_path.to_string_lossy().to_string(),
                            None,
                            None,
                            request,
                        )
                        .await
                        {
                            Ok(segments) => JobStatus::Completed(segments),
                            Err(e) => JobStatus::Failed(e.to_string()),
                        }
                    } else {
                        JobStatus::Failed(CLI_ONLINE_ASR_BATCH_UNAVAILABLE.to_string())
                    }
                } else {
                    JobStatus::Failed("Missing online provider ID".to_string())
                }
            } else {
                let options = build_local_transcribe_options(&job, &models_dir, &defaults);

                match resolve_transcribe_options(options, None) {
                    Ok(resolved) => {
                        match transcribe_batch_with_progress(&resolved.request, |_| {}).await {
                            Ok(segments) => JobStatus::Completed(segments),
                            Err(e) => JobStatus::Failed(e),
                        }
                    }
                    Err(e) => JobStatus::Failed(e.to_string()),
                }
            };

            job_manager
                .update_job(&job.job_id, final_status.clone())
                .await;

            if job.webhook_url.is_some() {
                let job_clone = job.clone();
                let status_clone = final_status.clone();
                tokio::spawn(async move {
                    send_webhook(&job_clone, &status_clone).await;
                });
            }

            // Cleanup
            let _ = tokio::fs::remove_file(&job.file_path).await;
        });
    }
}

pub(crate) fn build_local_transcribe_options(
    job: &TranscriptionJob,
    models_dir: &StdPath,
    defaults: &ApiServerTranscriptionDefaults,
) -> TranscribeCliOptions {
    let (vad_model_id, punctuation_model_id) =
        companion_defaults_for_model(&job.model_id, defaults);
    TranscribeCliOptions {
        input: job.file_path.clone(),
        output: None,
        format: None,
        language: if job.language == "auto" {
            None
        } else {
            Some(job.language.clone())
        },
        model_id: Some(job.model_id.clone()),
        models_dir: Some(models_dir.to_path_buf()),
        vad_model_id,
        punctuation_model_id,
        threads: None,
        enable_itn: None,
        hotwords: job.hotwords.clone(),
        gpu_acceleration: defaults.gpu_acceleration.clone(),
        vad_buffer: None,
        save_wav: None,
        quiet: true,
        force: true,
    }
}

fn companion_defaults_for_model(
    model_id: &str,
    defaults: &ApiServerTranscriptionDefaults,
) -> (Option<String>, Option<String>) {
    let rules =
        crate::core::preset_models::find_preset_model(model_id).map(|model| model.resolved_rules());

    let vad_model_id = match defaults.vad_model_id.as_deref() {
        Some(id)
            if rules.map(|rules| rules.requires_vad).unwrap_or(true)
                || id != crate::core::preset_models::DEFAULT_SILERO_VAD_MODEL_ID =>
        {
            Some(id.to_string())
        }
        _ => None,
    };

    let punctuation_model_id = match defaults.punctuation_model_id.as_deref() {
        Some(id)
            if rules
                .map(|rules| rules.requires_punctuation)
                .unwrap_or(true)
                || id != crate::core::preset_models::DEFAULT_PUNCTUATION_MODEL_ID =>
        {
            Some(id.to_string())
        }
        _ => None,
    };

    (vad_model_id, punctuation_model_id)
}

#[derive(Clone)]
pub struct ServerState {
    pub job_manager: JobManager,
    pub temp_dir: PathBuf,
    pub models_dir: PathBuf,
    pub start_time: std::time::Instant,
    pub api_key: String,
    pub streaming_semaphore: Arc<tokio::sync::Semaphore>,
    pub recognizer_pool: crate::integrations::asr::RecognizerPool,
    pub ip_whitelist: Arc<Vec<IpNet>>,
    pub online_asr_config: Arc<tokio::sync::RwLock<HashMap<String, serde_json::Value>>>,
    pub transcription_defaults: ApiServerTranscriptionDefaults,
    pub app: Option<tauri::AppHandle>,
}

pub async fn handle_transcribe(
    State(state): State<ServerState>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let job_id = uuid::Uuid::new_v4().to_string();
    let mut temp_file_path = None;
    let mut model_id = None;
    let mut language = "auto".to_string();
    let mut hotwords = None;
    let mut webhook_url = None;
    let mut webhook_secret = None;

    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" {
            let file_path = state.temp_dir.join(format!("{}.tmp", job_id));
            let mut file = tokio::fs::File::create(&file_path)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            while let Some(chunk) = field.next().await {
                let data = chunk.map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
                file.write_all(&data)
                    .await
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            }
            temp_file_path = Some(file_path);

            if let Some(ref path) = temp_file_path
                && !crate::integrations::media_detector::is_valid_media_file(path).await
            {
                let _ = tokio::fs::remove_file(path).await;
                return Err((
                    StatusCode::BAD_REQUEST,
                    "Unsupported file type or corrupted file".to_string(),
                ));
            }
        } else if name == "model_id" {
            model_id = Some(field.text().await.unwrap_or_default());
        } else if name == "language" {
            language = field.text().await.unwrap_or_default();
        } else if name == "hotwords" {
            hotwords = Some(field.text().await.unwrap_or_default());
        } else if name == "webhook_url" {
            webhook_url = Some(field.text().await.unwrap_or_default());
        } else if name == "webhook_secret" {
            webhook_secret = Some(field.text().await.unwrap_or_default());
        }
    }

    let file_path = temp_file_path.ok_or((StatusCode::BAD_REQUEST, "Missing file".to_string()))?;
    let m_id = model_id.ok_or((StatusCode::BAD_REQUEST, "Missing model_id".to_string()))?;

    let mut engine = "LocalSherpa".to_string();
    let mut online_provider_id = None;
    let mut online_provider_config = None;

    if let Some(provider) = crate::integrations::asr_providers::find_online_asr_provider(&m_id) {
        engine = "Online".to_string();
        online_provider_id = Some(provider.id.clone());
        let configs = state.online_asr_config.read().await;
        online_provider_config = configs.get(&provider.id).cloned();
    }

    let job = TranscriptionJob {
        job_id: job_id.clone(),
        file_path,
        model_id: m_id,
        language,
        hotwords,
        webhook_url,
        webhook_secret,
        engine,
        online_provider_id,
        online_provider_config,
    };
    state
        .job_manager
        .submit_job(job)
        .await
        .map_err(|e| (StatusCode::SERVICE_UNAVAILABLE, e))?;

    Ok(Json(serde_json::json!({ "job_id": job_id })))
}

pub async fn handle_job_status(
    State(state): State<ServerState>,
    Path(job_id): Path<String>,
) -> Result<Json<JobStatus>, (StatusCode, String)> {
    let status = state
        .job_manager
        .get_job(&job_id)
        .await
        .ok_or((StatusCode::NOT_FOUND, "Job not found".to_string()))?;
    Ok(Json(status))
}

pub async fn handle_list_jobs(
    State(state): State<ServerState>,
) -> Json<HashMap<String, JobStatus>> {
    let jobs = state.job_manager.list_jobs().await;
    Json(jobs)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub status: String,
    pub uptime: u64,
    pub active_jobs: usize,
    pub pending_jobs: usize,
    pub cache_space_bytes: u64,
}

pub async fn handle_health(State(state): State<ServerState>) -> Json<HealthResponse> {
    let uptime = state.start_time.elapsed().as_secs();

    let cache_space_bytes: u64 = tokio::task::spawn_blocking({
        let temp_dir = state.temp_dir.clone();
        move || {
            walkdir::WalkDir::new(&temp_dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter_map(|e| e.metadata().ok())
                .filter(|m| m.is_file())
                .map(|m| m.len())
                .sum()
        }
    })
    .await
    .unwrap_or(0);

    let jobs = state.job_manager.list_jobs().await;
    let mut active_jobs = 0;
    let mut pending_jobs = 0;
    for status in jobs.values() {
        match status {
            JobStatus::Pending => pending_jobs += 1,
            JobStatus::Processing => active_jobs += 1,
            _ => {}
        }
    }

    Json(HealthResponse {
        status: "ok".to_string(),
        uptime,
        active_jobs,
        pending_jobs,
        cache_space_bytes,
    })
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineAsrProviderInfo {
    pub id: String,
    pub configured: bool,
    pub supports_batch: bool,
    pub supports_streaming: bool,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoResponse {
    pub platform: String,
    pub gpu_available: bool,
    pub models: Vec<String>,
    pub vad_installed: bool,
    pub punctuation_installed: bool,
    pub online_asr_providers: Vec<OnlineAsrProviderInfo>,
}

pub async fn handle_info(
    State(state): State<ServerState>,
) -> Result<Json<InfoResponse>, (StatusCode, String)> {
    let gpu_available = crate::app::hardware::check_gpu_availability()
        .await
        .unwrap_or(false);

    let models_dir = state.models_dir.clone();
    let snapshot = tokio::task::spawn_blocking(move || {
        crate::core::preset_models::build_model_catalog_snapshot(&models_dir)
    })
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to build model snapshot: {e}"),
        )
    })?;

    let installed_models: Vec<String> = snapshot
        .models
        .iter()
        .filter(|m| m.is_installed)
        .map(|m| m.id.clone())
        .collect();

    let vad_installed = snapshot
        .models
        .iter()
        .any(|m| m.id == crate::core::preset_models::DEFAULT_SILERO_VAD_MODEL_ID && m.is_installed);

    let punctuation_installed = snapshot.models.iter().any(|m| {
        m.id == crate::core::preset_models::DEFAULT_PUNCTUATION_MODEL_ID && m.is_installed
    });

    let online_providers = crate::integrations::asr_providers::online_asr_providers();
    let configs = state.online_asr_config.read().await;
    let mut online_asr_providers = Vec::new();

    for provider in online_providers {
        let mut configured = false;
        if let Some(config_value) = configs.get(&provider.id)
            && let Some(api_key) = config_value.get("apiKey").and_then(|v| v.as_str())
            && !api_key.is_empty()
        {
            configured = true;
        }
        online_asr_providers.push(OnlineAsrProviderInfo {
            id: provider.id.clone(),
            configured,
            supports_batch: provider.batch.local_file_mode.supported,
            supports_streaming: provider.streaming.supported.unwrap_or(false),
        });
    }

    Ok(Json(InfoResponse {
        platform: std::env::consts::OS.to_string(),
        gpu_available,
        models: installed_models,
        vad_installed,
        punctuation_installed,
        online_asr_providers,
    }))
}

pub fn parse_ip_whitelist(whitelist_str: &str) -> Result<Vec<IpNet>, String> {
    let rules = whitelist_str
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    let mut nets = Vec::new();

    for rule in rules {
        if rule == "localhost" {
            nets.push("127.0.0.0/8".parse().unwrap());
            nets.push("::1/128".parse().unwrap());
        } else if let Ok(net) = rule.parse::<IpNet>() {
            nets.push(net);
        } else if let Ok(exact_ip) = rule.parse::<IpAddr>() {
            nets.push(IpNet::new(exact_ip, if exact_ip.is_ipv4() { 32 } else { 128 }).unwrap());
        } else if rule.contains('*') {
            if rule == "*" {
                nets.push("0.0.0.0/0".parse().unwrap());
                nets.push("::/0".parse().unwrap());
            } else if rule.ends_with(".*") {
                let prefix = rule.trim_end_matches(".*");
                let parts: Vec<&str> = prefix.split('.').collect();
                if parts.len() == 1 {
                    let ip_str = format!("{}.0.0.0", parts[0]);
                    if let Ok(ip) = ip_str.parse::<IpAddr>() {
                        nets.push(IpNet::new(ip, 8).unwrap());
                        continue;
                    }
                } else if parts.len() == 2 {
                    let ip_str = format!("{}.{}.0.0", parts[0], parts[1]);
                    if let Ok(ip) = ip_str.parse::<IpAddr>() {
                        nets.push(IpNet::new(ip, 16).unwrap());
                        continue;
                    }
                } else if parts.len() == 3 {
                    let ip_str = format!("{}.{}.{}.0", parts[0], parts[1], parts[2]);
                    if let Ok(ip) = ip_str.parse::<IpAddr>() {
                        nets.push(IpNet::new(ip, 24).unwrap());
                        continue;
                    }
                }
                return Err(format!("Invalid IP wildcard format: {}", rule));
            } else {
                return Err(format!("Invalid IP wildcard format: {}", rule));
            }
        } else {
            return Err(format!("Invalid IP rule format: {}", rule));
        }
    }

    if nets.is_empty() {
        nets.push("127.0.0.0/8".parse().unwrap());
        nets.push("::1/128".parse().unwrap());
    }

    Ok(nets)
}

async fn ip_whitelist_middleware(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(whitelist): State<Arc<Vec<IpNet>>>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if whitelist.iter().any(|net| net.contains(&addr.ip())) {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

fn format_bind_error(e: std::io::Error, addr: &str) -> String {
    let raw_code = e.raw_os_error();
    let os_err_suffix = match raw_code {
        Some(code) => format!(" (os error {code})"),
        None => "".to_string(),
    };
    match e.kind() {
        std::io::ErrorKind::AddrInUse => {
            format!(
                "Address already in use: {addr}. Make sure the port is not being used by another process.{os_err_suffix}"
            )
        }
        std::io::ErrorKind::AddrNotAvailable => {
            format!("Address not available: {addr}.{os_err_suffix}")
        }
        std::io::ErrorKind::PermissionDenied => {
            format!("Permission denied: Failed to bind to {addr}.{os_err_suffix}")
        }
        _ => {
            if let Some(code) = raw_code {
                format!("Failed to bind to {addr} (os error {code})")
            } else {
                format!("Failed to bind to {addr}: {e}")
            }
        }
    }
}

pub async fn run_server(config: ApiServerRuntimeConfig) -> Result<(), String> {
    let ApiServerRuntimeConfig {
        app,
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
        ip_whitelist,
        online_asr_config,
        transcription_defaults,
        shutdown_rx,
        bind_tx,
    } = config;

    // Resource Cleanup: clean temp_dir on startup
    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    if let Err(e) = tokio::fs::create_dir_all(&temp_dir).await {
        let err_msg = e.to_string();
        if let Some(tx) = bind_tx {
            let _ = tx.send(Err(err_msg.clone()));
        }
        return Err(err_msg);
    }

    let actual_queue_size = if max_queue_size == 0 {
        100_000
    } else {
        max_queue_size
    };
    let (tx, rx) = mpsc::channel(actual_queue_size);
    let job_manager = JobManager::new(tx);
    let job_manager_clone = job_manager.clone();
    let models_dir_clone = models_dir.clone();
    let app_clone = app.clone();
    let worker_defaults = transcription_defaults.clone();

    tokio::spawn(async move {
        start_worker_loop(
            rx,
            job_manager_clone,
            models_dir_clone,
            max_concurrent,
            app_clone,
            worker_defaults,
        )
        .await;
    });

    let job_manager_ttl = job_manager.clone();
    let (shutdown_ttl_tx, mut shutdown_ttl_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        let ttl_duration = std::time::Duration::from_secs(job_ttl_minutes * 60);
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if job_ttl_minutes > 0 {
                        job_manager_ttl.clean_expired_jobs(ttl_duration).await;
                    }
                }
                _ = &mut shutdown_ttl_rx => {
                    log::info!("TTL cleaner loop shutting down");
                    break;
                }
            }
        }
    });

    let recognizer_pool = if let Some(app_handle) = &app {
        use tauri::Manager;
        let sherpa_state = app_handle.state::<crate::integrations::asr::AsrState>();
        sherpa_state.recognizer_pool.clone()
    } else {
        crate::integrations::asr::RecognizerPool::new()
    };

    let state = ServerState {
        job_manager,
        temp_dir: temp_dir.clone(),
        models_dir,
        start_time: std::time::Instant::now(),
        api_key: api_key.clone(),
        streaming_semaphore: Arc::new(tokio::sync::Semaphore::new(max_streaming)),
        recognizer_pool,
        ip_whitelist: ip_whitelist.clone(),
        online_asr_config,
        transcription_defaults,
        app,
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Create public routes
    let router = Router::new().route("/health", get(handle_health));

    // Create private/transcriptions routes
    let mut api_router = Router::new()
        .route("/info", get(handle_info))
        .route("/v1/transcriptions", post(handle_transcribe))
        .route("/v1/transcriptions/jobs", get(handle_list_jobs))
        .route("/v1/transcriptions/{job_id}", get(handle_job_status))
        .with_state(state.clone())
        .layer(axum::middleware::from_fn_with_state(
            ip_whitelist,
            ip_whitelist_middleware,
        ));

    if max_upload_size_mb == 0 {
        api_router = api_router.layer(axum::extract::DefaultBodyLimit::disable());
    } else {
        api_router = api_router.layer(axum::extract::DefaultBodyLimit::max(
            max_upload_size_mb * 1024 * 1024,
        ));
    }

    #[allow(deprecated)]
    if !api_key.is_empty() {
        api_router = api_router.route_layer(ValidateRequestHeaderLayer::bearer(&api_key));
    }

    let ws_router = Router::new()
        .route(
            "/v1/streaming",
            get(crate::integrations::streaming::handle_streaming),
        )
        .with_state(state.clone());

    // Merge and apply CORS & state
    let router = router
        .merge(ws_router)
        .merge(api_router)
        .layer(cors)
        .with_state(state);
    let addr = format!("{}:{}", host, port);
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            let err_msg = format_bind_error(e, &addr);
            if let Some(tx) = bind_tx {
                let _ = tx.send(Err(err_msg.clone()));
            }
            return Err(err_msg);
        }
    };
    if let Some(tx) = bind_tx {
        let _ = tx.send(Ok(()));
    }

    log::info!("Starting HTTP API server on {}", addr);
    let clean_temp_dir = temp_dir.clone();
    let serve_res = axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        let _ = shutdown_rx.await;
        let _ = shutdown_ttl_tx.send(());
        log::info!("HTTP API server shutting down gracefully");
    })
    .await
    .map_err(|e| e.to_string());

    // Clean up temporary files on shutdown
    log::info!(
        "Cleaning up API server temporary directory: {:?}",
        clean_temp_dir
    );
    if let Err(e) = tokio::fs::remove_dir_all(&clean_temp_dir).await {
        log::error!(
            "Failed to clean up API server temporary directory {:?}: {}",
            clean_temp_dir,
            e
        );
    }

    serve_res
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_clean_expired_jobs() {
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        let job_manager = JobManager::new(tx);

        job_manager.jobs.write().await.insert(
            "expired-job".to_string(),
            JobEntry {
                status: JobStatus::Completed(vec![]),
                completed_at: Some(std::time::Instant::now() - std::time::Duration::from_secs(120)),
            },
        );

        job_manager.jobs.write().await.insert(
            "fresh-job".to_string(),
            JobEntry {
                status: JobStatus::Completed(vec![]),
                completed_at: Some(std::time::Instant::now()),
            },
        );

        job_manager.jobs.write().await.insert(
            "pending-job".to_string(),
            JobEntry {
                status: JobStatus::Pending,
                completed_at: None,
            },
        );

        job_manager
            .clean_expired_jobs(std::time::Duration::from_secs(60))
            .await;

        let jobs = job_manager.list_jobs().await;
        assert!(!jobs.contains_key("expired-job"));
        assert!(jobs.contains_key("fresh-job"));
        assert!(jobs.contains_key("pending-job"));
    }

    #[tokio::test]
    async fn test_health_endpoint() {
        let temp_dir = tempfile::tempdir()
            .expect("Failed to create temporary directory")
            .path()
            .to_path_buf();
        let models_dir = tempfile::tempdir()
            .expect("Failed to create temporary directory")
            .path()
            .to_path_buf();
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        let job_manager = JobManager::new(tx);

        let state = ServerState {
            job_manager,
            temp_dir,
            models_dir,
            start_time: std::time::Instant::now(),
            api_key: "".to_string(),
            streaming_semaphore: std::sync::Arc::new(tokio::sync::Semaphore::new(1)),
            recognizer_pool: crate::integrations::asr::RecognizerPool::new(),
            ip_whitelist: std::sync::Arc::new(vec![]),
            online_asr_config: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            transcription_defaults: ApiServerTranscriptionDefaults::default(),
            app: None,
        };

        let app = Router::new()
            .route("/health", get(handle_health))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .expect("Failed to build request"),
            )
            .await
            .expect("Failed to dispatch request");

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("Failed to read response body buffer");
        let body: serde_json::Value =
            serde_json::from_slice(&body).expect("Failed to parse response body as valid JSON");

        assert_eq!(body["status"], "ok");
        assert!(body["uptime"].is_number());
        assert!(body["activeJobs"].is_number());
        assert!(body["pendingJobs"].is_number());
        assert!(body["cacheSpaceBytes"].is_number());
    }

    #[tokio::test]
    async fn test_info_endpoint() {
        let temp_dir = tempfile::tempdir()
            .expect("Failed to create temporary directory")
            .path()
            .to_path_buf();
        let models_dir = tempfile::tempdir()
            .expect("Failed to create temporary directory")
            .path()
            .to_path_buf();
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        let job_manager = JobManager::new(tx);

        let state = ServerState {
            job_manager,
            temp_dir,
            models_dir,
            start_time: std::time::Instant::now(),
            api_key: "".to_string(),
            streaming_semaphore: std::sync::Arc::new(tokio::sync::Semaphore::new(1)),
            recognizer_pool: crate::integrations::asr::RecognizerPool::new(),
            ip_whitelist: std::sync::Arc::new(vec![]),
            online_asr_config: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            transcription_defaults: ApiServerTranscriptionDefaults::default(),
            app: None,
        };

        let app = Router::new()
            .route("/info", get(handle_info))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/info")
                    .body(Body::empty())
                    .expect("Failed to build request"),
            )
            .await
            .expect("Failed to dispatch request");

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("Failed to read response body buffer");
        let body: serde_json::Value =
            serde_json::from_slice(&body).expect("Failed to parse response body as valid JSON");

        assert!(body["platform"].is_string());
        assert!(body["gpuAvailable"].is_boolean());
        assert!(body["models"].is_array());
        assert!(body["vadInstalled"].is_boolean());
        assert!(body["punctuationInstalled"].is_boolean());
    }

    #[tokio::test]
    async fn test_list_jobs_endpoint() {
        let temp_dir = tempfile::tempdir()
            .expect("Failed to create temporary directory")
            .path()
            .to_path_buf();
        let models_dir = tempfile::tempdir()
            .expect("Failed to create temporary directory")
            .path()
            .to_path_buf();
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        let job_manager = JobManager::new(tx);

        // Add a mock job
        job_manager.jobs.write().await.insert(
            "test-job-id".to_string(),
            JobEntry {
                status: JobStatus::Pending,
                completed_at: None,
            },
        );

        let state = ServerState {
            job_manager,
            temp_dir,
            models_dir,
            start_time: std::time::Instant::now(),
            api_key: "".to_string(),
            streaming_semaphore: std::sync::Arc::new(tokio::sync::Semaphore::new(1)),
            recognizer_pool: crate::integrations::asr::RecognizerPool::new(),
            ip_whitelist: std::sync::Arc::new(vec![]),
            online_asr_config: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            transcription_defaults: ApiServerTranscriptionDefaults::default(),
            app: None,
        };

        let app = Router::new()
            .route("/v1/transcriptions/jobs", get(handle_list_jobs))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/transcriptions/jobs")
                    .body(Body::empty())
                    .expect("Failed to build request"),
            )
            .await
            .expect("Failed to dispatch request");

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("Failed to read response body buffer");
        let body: serde_json::Value =
            serde_json::from_slice(&body).expect("Failed to parse response body as valid JSON");

        assert!(body.is_object());
        assert_eq!(body["test-job-id"], "Pending");
    }

    #[test]
    fn local_transcribe_options_use_server_defaults() {
        let models_dir = PathBuf::from("C:/models");
        let job = TranscriptionJob {
            job_id: "job-1".to_string(),
            file_path: PathBuf::from("sample.wav"),
            model_id: "sherpa-onnx-whisper-turbo".to_string(),
            language: "auto".to_string(),
            hotwords: Some("Sona".to_string()),
            webhook_url: None,
            webhook_secret: None,
            engine: "LocalSherpa".to_string(),
            online_provider_id: None,
            online_provider_config: None,
        };
        let defaults = ApiServerTranscriptionDefaults {
            gpu_acceleration: Some("cuda".to_string()),
            vad_model_id: Some("silero-vad".to_string()),
            punctuation_model_id: Some("punct-model".to_string()),
        };

        let options = build_local_transcribe_options(&job, &models_dir, &defaults);

        assert_eq!(options.gpu_acceleration.as_deref(), Some("cuda"));
        assert_eq!(options.vad_model_id.as_deref(), Some("silero-vad"));
        assert_eq!(options.punctuation_model_id.as_deref(), Some("punct-model"));
        assert_eq!(options.models_dir.as_deref(), Some(models_dir.as_path()));
        assert!(options.language.is_none());
        assert_eq!(options.hotwords.as_deref(), Some("Sona"));
    }

    #[test]
    fn local_transcribe_options_skip_builtin_punctuation_default_when_model_does_not_require_it() {
        let models_dir = PathBuf::from("C:/models");
        let job = TranscriptionJob {
            job_id: "job-1".to_string(),
            file_path: PathBuf::from("sample.wav"),
            model_id: "sherpa-onnx-whisper-turbo".to_string(),
            language: "auto".to_string(),
            hotwords: None,
            webhook_url: None,
            webhook_secret: None,
            engine: "LocalSherpa".to_string(),
            online_provider_id: None,
            online_provider_config: None,
        };
        let defaults = ApiServerTranscriptionDefaults::default();

        let options = build_local_transcribe_options(&job, &models_dir, &defaults);

        assert_eq!(options.vad_model_id.as_deref(), Some("silero-vad"));
        assert!(options.punctuation_model_id.is_none());
    }

    #[test]
    fn local_transcribe_options_apply_builtin_punctuation_default_when_model_requires_it() {
        let models_dir = PathBuf::from("C:/models");
        let job = TranscriptionJob {
            job_id: "job-1".to_string(),
            file_path: PathBuf::from("sample.wav"),
            model_id: "sherpa-onnx-funasr-nano-int8-2025-12-30".to_string(),
            language: "auto".to_string(),
            hotwords: None,
            webhook_url: None,
            webhook_secret: None,
            engine: "LocalSherpa".to_string(),
            online_provider_id: None,
            online_provider_config: None,
        };
        let defaults = ApiServerTranscriptionDefaults::default();

        let options = build_local_transcribe_options(&job, &models_dir, &defaults);

        assert_eq!(
            options.punctuation_model_id.as_deref(),
            Some(crate::core::preset_models::DEFAULT_PUNCTUATION_MODEL_ID)
        );
    }

    static TEST_LOGGER: std::sync::OnceLock<Arc<std::sync::Mutex<Vec<String>>>> =
        std::sync::OnceLock::new();

    struct SimpleTestLogger {
        logs: Arc<std::sync::Mutex<Vec<String>>>,
    }

    impl log::Log for SimpleTestLogger {
        fn enabled(&self, _metadata: &log::Metadata) -> bool {
            true
        }
        fn log(&self, record: &log::Record) {
            let mut logs = self.logs.lock().unwrap();
            logs.push(format!("{}: {}", record.level(), record.args()));
        }
        fn flush(&self) {}
    }

    fn init_test_logger() -> Arc<std::sync::Mutex<Vec<String>>> {
        let logs = TEST_LOGGER.get_or_init(|| {
            let logs = Arc::new(std::sync::Mutex::new(Vec::new()));
            let logger = SimpleTestLogger { logs: logs.clone() };
            let _ = log::set_boxed_logger(Box::new(logger));
            log::set_max_level(log::LevelFilter::Debug);
            logs
        });
        logs.clone()
    }

    async fn wait_for_log(logs: &Arc<std::sync::Mutex<Vec<String>>>, needle: &str) {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            if logs
                .lock()
                .unwrap()
                .iter()
                .any(|log_line| log_line.contains(needle))
            {
                return;
            }

            assert!(
                tokio::time::Instant::now() < deadline,
                "Timed out waiting for log {needle:?}. Current logs: {:?}",
                *logs.lock().unwrap()
            );

            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }

    #[tokio::test]
    async fn test_temp_directory_cleanup_failure_logging() {
        let logs = init_test_logger();
        {
            let mut logs_lock = logs.lock().unwrap();
            logs_lock.clear();
        }

        let temp_dir_handle = tempfile::tempdir().expect("Failed to create temporary directory");
        let temp_dir = temp_dir_handle.path().to_path_buf();
        let models_dir_handle = tempfile::tempdir().expect("Failed to create temporary directory");
        let models_dir = models_dir_handle.path().to_path_buf();

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

        let config = ApiServerRuntimeConfig {
            app: None,
            host: "127.0.0.1".to_string(),
            port: 0,
            api_key: "".to_string(),
            temp_dir: temp_dir.clone(),
            models_dir,
            max_concurrent: 1,
            max_queue_size: 0,
            max_upload_size_mb: 1,
            job_ttl_minutes: 1,
            max_streaming: 1,
            ip_whitelist: std::sync::Arc::new(vec![]),
            online_asr_config: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            transcription_defaults: ApiServerTranscriptionDefaults::default(),
            shutdown_rx,
            bind_tx: None,
        };

        let handle = tokio::spawn(async move { run_server(config).await });

        wait_for_log(&logs, "Starting HTTP API server on").await;

        // Replace the directory with a file so shutdown cleanup reliably fails.
        tokio::fs::remove_dir_all(&temp_dir)
            .await
            .expect("Failed to remove temp directory before shutdown");
        tokio::fs::write(&temp_dir, b"cleanup should fail on this file")
            .await
            .expect("Failed to replace temp directory with a file");

        let _ = shutdown_tx.send(());

        let run_result = handle.await.expect("Server task panicked");
        assert!(run_result.is_ok());

        {
            let logs_lock = logs.lock().unwrap();
            let error_log_found = logs_lock.iter().any(|log_line| {
                log_line.contains("Failed to clean up API server temporary directory")
            });

            assert!(
                error_log_found,
                "Expected error log message not found. Current logs: {:?}",
                *logs_lock
            );
        }

        let _ = tokio::fs::remove_file(&temp_dir).await;
    }

    #[test]
    fn test_format_bind_error() {
        let addr = "127.0.0.1:14200";

        let in_use_err = std::io::Error::new(std::io::ErrorKind::AddrInUse, "address in use");
        let formatted = format_bind_error(in_use_err, addr);
        assert!(formatted.contains("Address already in use: 127.0.0.1:14200"));

        let not_avail_err =
            std::io::Error::new(std::io::ErrorKind::AddrNotAvailable, "not available");
        let formatted = format_bind_error(not_avail_err, addr);
        assert!(formatted.contains("Address not available: 127.0.0.1:14200"));

        let permission_err =
            std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied");
        let formatted = format_bind_error(permission_err, addr);
        assert!(formatted.contains("Permission denied: Failed to bind to 127.0.0.1:14200"));

        #[cfg(target_os = "windows")]
        let os_code = 10048;
        #[cfg(target_os = "macos")]
        let os_code = 48;
        #[cfg(target_os = "linux")]
        let os_code = 98;
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        let os_code = 98;

        let custom_os_err = std::io::Error::from_raw_os_error(os_code);
        let formatted = format_bind_error(custom_os_err, addr);
        assert!(formatted.contains("Address already in use: 127.0.0.1:14200"));
        assert!(formatted.contains(&format!("os error {os_code}")));

        let other_err = std::io::Error::other("something went wrong");
        let formatted = format_bind_error(other_err, addr);
        assert!(formatted.contains("Failed to bind to 127.0.0.1:14200: something went wrong"));
    }

    #[tokio::test]
    async fn test_start_api_server_bind_failure() {
        // First bind to a port to occupy it
        let addr = "127.0.0.1:0"; // bind to any ephemeral port
        let occupier = tokio::net::TcpListener::bind(addr)
            .await
            .expect("Failed to bind occupier");
        let occupied_addr = occupier.local_addr().expect("Failed to get local address");
        let port = occupied_addr.port();

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let (bind_tx, bind_rx) = tokio::sync::oneshot::channel();

        let temp_dir_handle = tempfile::tempdir().expect("Failed to create tempdir");
        let temp_dir = temp_dir_handle.path().to_path_buf();
        let models_dir = temp_dir.join("models");

        let config = ApiServerRuntimeConfig {
            app: None,
            host: "127.0.0.1".to_string(),
            port,
            api_key: "".to_string(),
            temp_dir,
            models_dir,
            max_concurrent: 1,
            max_queue_size: 0,
            max_upload_size_mb: 1,
            job_ttl_minutes: 1,
            max_streaming: 1,
            ip_whitelist: std::sync::Arc::new(vec![]),
            online_asr_config: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            transcription_defaults: ApiServerTranscriptionDefaults::default(),
            shutdown_rx,
            bind_tx: Some(bind_tx),
        };

        // Spawning run_server should return Err because the port is occupied
        let handle = tokio::spawn(async move { run_server(config).await });

        let bind_res = bind_rx.await.expect("bind_rx closed");
        assert!(bind_res.is_err());
        let err_msg = bind_res.unwrap_err();
        assert!(err_msg.contains("Address already in use") || err_msg.contains("os error 10048"));

        let _ = shutdown_tx.send(());
        let _ = handle.await;
    }
}
