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
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::{RwLock, mpsc};
use tower_http::{
    cors::{Any, CorsLayer},
    validate_request::ValidateRequestHeaderLayer,
};

type HmacSha256 = Hmac<Sha256>;

use crate::asr::transcribe_batch_with_progress;
use crate::cli::{TranscribeCliOptions, resolve_transcribe_options};

#[derive(Debug, Clone, serde::Serialize)]
pub enum JobStatus {
    Pending,
    Processing,
    Completed(Vec<crate::asr::TranscriptSegment>),
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
    let client = WEBHOOK_CLIENT.get_or_init(|| reqwest::Client::new());

    let mut request = client
        .post(webhook_url)
        .header("Content-Type", "application/json");

    if let Some(secret) = &job.webhook_secret {
        if !secret.is_empty() {
            if let Ok(mut mac) = HmacSha256::new_from_slice(secret.as_bytes()) {
                mac.update(payload_str.as_bytes());
                let result = mac.finalize().into_bytes();
                let hex_signature = hex::encode(result);
                request = request.header("X-Sona-Signature", format!("sha256={}", hex_signature));
            }
        }
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
    manager: JobManager,
    models_dir: PathBuf,
    max_concurrent: usize,
    app: Option<tauri::AppHandle>,
) {
    let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrent));

    while let Some(job) = receiver.recv().await {
        let manager = manager.clone();
        let models_dir = models_dir.clone();
        let semaphore = semaphore.clone();
        let app_for_spawn = app.clone();

        tokio::spawn(async move {
            let app = app_for_spawn;
            let _permit = match semaphore.acquire().await {
                Ok(permit) => permit,
                Err(_) => {
                    log::error!("[Server] semaphore closed, job {} abandoned", job.job_id);
                    manager
                        .update_job(
                            &job.job_id,
                            JobStatus::Failed("Internal: worker pool closed".to_string()),
                        )
                        .await;
                    return;
                }
            };
            manager.update_job(&job.job_id, JobStatus::Processing).await;

            let final_status = if job.engine == "Online" {
                if let Some(provider_id) = job.online_provider_id.clone() {
                    let request = crate::asr::AsrTranscriptionRequest {
                        engine: crate::asr::AsrEngine::Online,
                        mode: crate::asr::AsrMode::Offline,
                        model_id: Some(job.model_id.clone()),
                        model_path: "".to_string(),
                        num_threads: 1,
                        enable_itn: false,
                        language: if job.language == "auto" {
                            "".to_string()
                        } else {
                            job.language.clone()
                        },
                        punctuation_model: None,
                        vad_model: None,
                        vad_buffer: 0.0,
                        batch_segmentation_mode: crate::asr::BatchSegmentationMode::Vad,
                        model_type: "".to_string(),
                        file_config: None,
                        hotwords: job.hotwords.clone(),
                        normalization_options: Default::default(),
                        postprocess_options: Default::default(),
                        online_provider: Some(crate::asr::OnlineAsrProviderRequest {
                            provider_id: provider_id,
                            profile_id: job.model_id.clone(),
                            config: job.online_provider_config.clone().unwrap_or_default(),
                        }),
                        gpu_acceleration: None,
                    };
                    if let Some(app_handle) = app.as_ref() {
                        use tauri::Manager;
                        let inner_app_clone = app_handle.clone();
                        let sherpa_state = app_handle.state::<crate::asr::AsrState>();
                        match crate::asr::online::process_batch_file_impl(
                            inner_app_clone,
                            sherpa_state.inner(),
                            job.file_path.to_string_lossy().to_string(),
                            request,
                        )
                        .await
                        {
                            Ok(segments) => JobStatus::Completed(segments),
                            Err(e) => JobStatus::Failed(e.to_string()),
                        }
                    } else {
                        JobStatus::Failed("AppHandle missing".to_string())
                    }
                } else {
                    JobStatus::Failed("Missing online provider ID".to_string())
                }
            } else {
                let options = TranscribeCliOptions {
                    input: job.file_path.clone(),
                    output: None,
                    format: None,
                    language: if job.language == "auto" {
                        None
                    } else {
                        Some(job.language.clone())
                    },
                    model_id: Some(job.model_id.clone()),
                    models_dir: Some(models_dir.clone()),
                    vad_model_id: None,
                    punctuation_model_id: None,
                    threads: None,
                    enable_itn: None,
                    hotwords: job.hotwords.clone(),
                    vad_buffer: None,
                    save_wav: None,
                    quiet: true,
                };

                match resolve_transcribe_options(options, None) {
                    Ok(resolved) => {
                        match transcribe_batch_with_progress(&resolved.request, |_| {}).await {
                            Ok(segments) => JobStatus::Completed(segments),
                            Err(e) => JobStatus::Failed(e),
                        }
                    }
                    Err(e) => JobStatus::Failed(e),
                }
            };

            manager.update_job(&job.job_id, final_status.clone()).await;

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

#[derive(Clone)]
pub struct ServerState {
    pub job_manager: JobManager,
    pub temp_dir: PathBuf,
    pub models_dir: PathBuf,
    pub start_time: std::time::Instant,
    pub api_key: String,
    pub streaming_semaphore: Arc<tokio::sync::Semaphore>,
    pub recognizer_pool:
        Arc<tokio::sync::Mutex<HashMap<crate::asr::ModelConfigKey, Arc<crate::asr::Recognizer>>>>,
    pub ip_whitelist: Arc<Vec<IpNet>>,
    pub online_asr_config: Arc<tokio::sync::RwLock<HashMap<String, serde_json::Value>>>,
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

            if let Some(ref path) = temp_file_path {
                if !crate::media_detector::is_valid_media_file(path).await {
                    let _ = tokio::fs::remove_file(path).await;
                    return Err((
                        StatusCode::BAD_REQUEST,
                        "Unsupported file type or corrupted file".to_string(),
                    ));
                }
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

    if let Some(provider) = crate::asr_providers::find_online_asr_provider(&m_id) {
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
    let gpu_available = crate::hardware::check_gpu_availability()
        .await
        .unwrap_or(false);

    let models_dir = state.models_dir.clone();
    let snapshot = tokio::task::spawn_blocking(move || {
        crate::preset_models::build_model_catalog_snapshot(&models_dir)
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
        .any(|m| m.id == crate::preset_models::DEFAULT_SILERO_VAD_MODEL_ID && m.is_installed);

    let punctuation_installed = snapshot
        .models
        .iter()
        .any(|m| m.id == crate::preset_models::DEFAULT_PUNCTUATION_MODEL_ID && m.is_installed);

    let online_providers = crate::asr_providers::online_asr_providers();
    let configs = state.online_asr_config.read().await;
    let mut online_asr_providers = Vec::new();

    for provider in online_providers {
        let mut configured = false;
        if let Some(config_value) = configs.get(&provider.id) {
            if let Some(api_key) = config_value.get("apiKey").and_then(|v| v.as_str()) {
                if !api_key.is_empty() {
                    configured = true;
                }
            }
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

#[allow(clippy::too_many_arguments)]
pub async fn run_server(
    app: Option<tauri::AppHandle>,
    host: &str,
    port: u16,
    api_key: &str,
    temp_dir: PathBuf,
    models_dir: PathBuf,
    max_concurrent: usize,
    max_queue_size: usize,
    max_upload_size_mb: usize,
    job_ttl_minutes: u64,
    max_streaming: usize,
    ip_whitelist: Arc<Vec<IpNet>>,
    online_asr_config: Arc<tokio::sync::RwLock<HashMap<String, serde_json::Value>>>,
    shutdown_rx: tokio::sync::oneshot::Receiver<()>,
) -> Result<(), String> {
    // Resource Cleanup: clean temp_dir on startup
    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    tokio::fs::create_dir_all(&temp_dir)
        .await
        .map_err(|e| e.to_string())?;

    let actual_queue_size = if max_queue_size == 0 {
        100_000
    } else {
        max_queue_size
    };
    let (tx, rx) = mpsc::channel(actual_queue_size);
    let job_manager = JobManager::new(tx);
    let manager_clone = job_manager.clone();
    let models_dir_clone = models_dir.clone();
    let app_clone = app.clone();

    tokio::spawn(async move {
        start_worker_loop(
            rx,
            manager_clone,
            models_dir_clone,
            max_concurrent,
            app_clone,
        )
        .await;
    });

    let manager_ttl_clone = job_manager.clone();
    tokio::spawn(async move {
        let ttl_duration = std::time::Duration::from_secs(job_ttl_minutes * 60);
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            if job_ttl_minutes > 0 {
                let mut jobs = manager_ttl_clone.jobs.write().await;
                jobs.retain(|_, entry| {
                    if let Some(completed_at) = entry.completed_at {
                        completed_at.elapsed() <= ttl_duration
                    } else {
                        true
                    }
                });
            }
        }
    });

    let state = ServerState {
        job_manager,
        temp_dir,
        models_dir,
        start_time: std::time::Instant::now(),
        api_key: api_key.to_string(),
        streaming_semaphore: Arc::new(tokio::sync::Semaphore::new(max_streaming)),
        recognizer_pool: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        ip_whitelist: ip_whitelist.clone(),
        online_asr_config,
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
        api_router = api_router.route_layer(ValidateRequestHeaderLayer::bearer(api_key));
    }

    let ws_router = Router::new()
        .route("/v1/streaming", get(crate::streaming::handle_streaming))
        .with_state(state.clone());

    // Merge and apply CORS & state
    let router = router
        .merge(ws_router)
        .merge(api_router)
        .layer(cors)
        .with_state(state);
    let addr = format!("{}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| e.to_string())?;

    log::info!("Starting HTTP API server on {}", addr);
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        let _ = shutdown_rx.await;
        log::info!("HTTP API server shutting down gracefully");
    })
    .await
    .map_err(|e| e.to_string())
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
            recognizer_pool: std::sync::Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            ip_whitelist: std::sync::Arc::new(vec![]),
            online_asr_config: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
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
            recognizer_pool: std::sync::Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            ip_whitelist: std::sync::Arc::new(vec![]),
            online_asr_config: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
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
            recognizer_pool: std::sync::Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            ip_whitelist: std::sync::Arc::new(vec![]),
            online_asr_config: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
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
}
