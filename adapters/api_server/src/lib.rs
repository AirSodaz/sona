mod error;

pub use error::*;

use async_trait::async_trait;
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
use sona_core::ports::asr::{
    BatchTranscriber, OnlineAsrProviderRequest, find_online_asr_provider, online_asr_providers,
};
use sona_core::ports::fs::{FileSystemError, FileSystemOperation};
use sona_core::ports::runtime::{
    BatchTranscribePlanResolver, GpuAvailabilityProvider, MediaFileValidator, ModelCatalogProvider,
    RuntimeCapabilityError,
};
use sona_core::runtime::gpu::DEFAULT_GPU_ACCELERATION;
use sona_core::runtime::serve::ResolvedServeRuntimeOptions;
use sona_core::transcription::runtime::BatchTranscribeOptions;
use sona_core::transcription::transcript::TranscriptSegment;
use std::any::Any;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path as StdPath, PathBuf};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::{RwLock, mpsc};
use tower_http::{
    cors::{Any as CorsAny, CorsLayer},
    validate_request::ValidateRequestHeaderLayer,
};

type HmacSha256 = Hmac<Sha256>;

pub const ONLINE_ASR_BATCH_UNAVAILABLE: &str =
    "Online ASR batch is unavailable because no platform online ASR adapter is configured.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiServerTranscriptionDefaults {
    pub gpu_acceleration: Option<String>,
    pub vad_model_id: Option<String>,
    pub punctuation_model_id: Option<String>,
}

impl Default for ApiServerTranscriptionDefaults {
    fn default() -> Self {
        Self {
            gpu_acceleration: Some(DEFAULT_GPU_ACCELERATION.to_string()),
            vad_model_id: Some(
                sona_core::models::preset_models::DEFAULT_SILERO_VAD_MODEL_ID.to_string(),
            ),
            punctuation_model_id: Some(
                sona_core::models::preset_models::DEFAULT_PUNCTUATION_MODEL_ID.to_string(),
            ),
        }
    }
}

#[derive(Clone, Debug)]
pub struct OnlineBatchRequest {
    pub file_path: PathBuf,
    pub provider_id: String,
    pub profile_id: String,
    pub config: serde_json::Value,
    pub language: String,
    pub hotwords: Option<String>,
}

#[async_trait]
pub trait ApiServerPlatform: Send + Sync {
    async fn transcribe_online_batch(
        &self,
        _request: OnlineBatchRequest,
    ) -> Result<Vec<TranscriptSegment>, ApiServerPlatformError> {
        Err(ApiServerPlatformError::unavailable(
            ONLINE_ASR_BATCH_UNAVAILABLE,
        ))
    }

    fn streaming_context(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        None
    }
}

#[derive(Clone, Default)]
pub struct DefaultApiServerPlatform;

#[async_trait]
impl ApiServerPlatform for DefaultApiServerPlatform {}

pub struct ApiServerRuntimeConfig {
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
    pub online_asr_config: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    pub transcription_defaults: ApiServerTranscriptionDefaults,
    pub batch_transcriber: Arc<dyn BatchTranscriber>,
    pub media_validator: Arc<dyn MediaFileValidator>,
    pub gpu_availability: Arc<dyn GpuAvailabilityProvider>,
    pub model_catalog: Arc<dyn ModelCatalogProvider>,
    pub batch_plan_resolver: Arc<dyn BatchTranscribePlanResolver>,
    pub platform: Arc<dyn ApiServerPlatform>,
    pub streaming_router: Option<Router<ServerState>>,
    pub shutdown_rx: tokio::sync::oneshot::Receiver<()>,
    pub bind_tx: Option<
        tokio::sync::oneshot::Sender<Result<ApiServerDashboardHandle, ApiServerRuntimeError>>,
    >,
}

pub struct ApiServerRuntimeParts {
    pub resolved: ResolvedServeRuntimeOptions,
    pub temp_dir: PathBuf,
    pub online_asr_config: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    pub batch_transcriber: Arc<dyn BatchTranscriber>,
    pub media_validator: Arc<dyn MediaFileValidator>,
    pub gpu_availability: Arc<dyn GpuAvailabilityProvider>,
    pub model_catalog: Arc<dyn ModelCatalogProvider>,
    pub batch_plan_resolver: Arc<dyn BatchTranscribePlanResolver>,
    pub platform: Arc<dyn ApiServerPlatform>,
    pub streaming_router: Option<Router<ServerState>>,
    pub shutdown_rx: tokio::sync::oneshot::Receiver<()>,
    pub bind_tx: Option<
        tokio::sync::oneshot::Sender<Result<ApiServerDashboardHandle, ApiServerRuntimeError>>,
    >,
}

pub struct PreparedApiServerRuntime {
    pub config: ApiServerRuntimeConfig,
    pub normalized_ip_whitelist: String,
}

pub struct ApiServerServiceParts {
    pub resolved: ResolvedServeRuntimeOptions,
    pub temp_dir: PathBuf,
    pub online_asr_config: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    pub batch_transcriber: Arc<dyn BatchTranscriber>,
    pub media_validator: Arc<dyn MediaFileValidator>,
    pub gpu_availability: Arc<dyn GpuAvailabilityProvider>,
    pub model_catalog: Arc<dyn ModelCatalogProvider>,
    pub batch_plan_resolver: Arc<dyn BatchTranscribePlanResolver>,
    pub platform: Arc<dyn ApiServerPlatform>,
    pub streaming_router: Option<Router<ServerState>>,
}

pub struct RunningApiServer {
    pub normalized_ip_whitelist: String,
    pub shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    pub join_handle: tokio::task::JoinHandle<Result<(), ApiServerRuntimeError>>,
    dashboard: ApiServerDashboardHandle,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiServerDashboardSnapshot {
    pub health: HealthResponse,
    pub info: InfoResponse,
    pub jobs: HashMap<String, JobStatus>,
}

#[derive(Clone)]
pub struct ApiServerDashboardHandle {
    state: ServerState,
}

impl std::fmt::Debug for RunningApiServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunningApiServer")
            .field("normalized_ip_whitelist", &self.normalized_ip_whitelist)
            .finish_non_exhaustive()
    }
}

impl RunningApiServer {
    pub async fn dashboard_snapshot(
        &self,
    ) -> Result<ApiServerDashboardSnapshot, ApiServerDashboardError> {
        self.dashboard.snapshot().await
    }

    pub fn dashboard_handle(&self) -> ApiServerDashboardHandle {
        self.dashboard.clone()
    }

    pub fn signal_shutdown(&mut self) -> Result<(), ApiServerStopError> {
        if let Some(sender) = self.shutdown_tx.take() {
            sender
                .send(())
                .map_err(|_| ApiServerStopError::ShutdownSignalClosed)?;
        }
        Ok(())
    }

    pub async fn wait(self) -> Result<(), ApiServerRuntimeError> {
        self.join_handle
            .await
            .map_err(|error| ApiServerRuntimeError::TaskJoin {
                reason: error.to_string(),
            })?
    }

    pub async fn stop(mut self) -> Result<(), ApiServerStopError> {
        let shutdown_result = self.signal_shutdown();
        let wait_result = self.wait().await;
        match (shutdown_result, wait_result) {
            (_, Err(error)) => Err(ApiServerStopError::Runtime(error)),
            (Err(error), Ok(())) => Err(error),
            (Ok(()), Ok(())) => Ok(()),
        }
    }
}

impl ApiServerDashboardHandle {
    pub async fn snapshot(&self) -> Result<ApiServerDashboardSnapshot, ApiServerDashboardError> {
        let health = build_health_response(&self.state).await;
        let configs = self.state.online_asr_config.read().await.clone();
        let info = build_info_response(
            Arc::clone(&self.state.gpu_availability),
            Arc::clone(&self.state.model_catalog),
            &self.state.models_dir,
            &configs,
        )
        .await
        .map_err(ApiServerDashboardError::Platform)?;
        let jobs = self.state.job_manager.list_jobs().await;

        Ok(ApiServerDashboardSnapshot { health, info, jobs })
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum JobStatus {
    Pending,
    Processing,
    Completed(Vec<TranscriptSegment>),
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

    pub async fn submit_job(&self, job: TranscriptionJob) -> Result<(), ApiServerJobError> {
        let job_id = job.job_id.clone();
        self.jobs.write().await.insert(
            job_id.clone(),
            JobEntry {
                status: JobStatus::Pending,
                completed_at: None,
            },
        );
        if self.sender.send(job).await.is_err() {
            self.jobs.write().await.remove(&job_id);
            return Err(ApiServerJobError::QueueClosed { job_id });
        }
        Ok(())
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

#[derive(Clone)]
pub struct ServerState {
    pub job_manager: JobManager,
    pub temp_dir: PathBuf,
    pub models_dir: PathBuf,
    pub start_time: std::time::Instant,
    pub api_key: String,
    pub streaming_semaphore: Arc<tokio::sync::Semaphore>,
    pub ip_whitelist: Arc<Vec<IpNet>>,
    pub online_asr_config: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    pub transcription_defaults: ApiServerTranscriptionDefaults,
    pub media_validator: Arc<dyn MediaFileValidator>,
    pub gpu_availability: Arc<dyn GpuAvailabilityProvider>,
    pub model_catalog: Arc<dyn ModelCatalogProvider>,
    pub batch_plan_resolver: Arc<dyn BatchTranscribePlanResolver>,
    pub platform: Arc<dyn ApiServerPlatform>,
}

pub fn build_streaming_router<H, T>(handler: H) -> Router<ServerState>
where
    H: axum::handler::Handler<T, ServerState>,
    T: 'static,
{
    Router::new().route("/v1/streaming", get(handler))
}

pub fn authorize_streaming_request(
    state: &ServerState,
    addr: SocketAddr,
    token: Option<&str>,
) -> Result<tokio::sync::OwnedSemaphorePermit, StatusCode> {
    if !state
        .ip_whitelist
        .iter()
        .any(|net| net.contains(&addr.ip()))
    {
        return Err(StatusCode::FORBIDDEN);
    }

    if !state.api_key.is_empty() && token.unwrap_or_default() != state.api_key {
        return Err(StatusCode::UNAUTHORIZED);
    }

    state
        .streaming_semaphore
        .clone()
        .try_acquire_owned()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
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

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineAsrProviderInfo {
    pub id: String,
    pub configured: bool,
    pub supports_batch: bool,
    pub supports_streaming: bool,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoResponse {
    pub platform: String,
    pub gpu_available: bool,
    pub models: Vec<String>,
    pub vad_installed: bool,
    pub punctuation_installed: bool,
    pub online_asr_providers: Vec<OnlineAsrProviderInfo>,
}

pub async fn build_info_response(
    gpu_availability: Arc<dyn GpuAvailabilityProvider>,
    model_catalog: Arc<dyn ModelCatalogProvider>,
    models_dir: &StdPath,
    online_asr_config: &HashMap<String, serde_json::Value>,
) -> Result<InfoResponse, ApiServerPlatformError> {
    let gpu_available = gpu_availability.is_gpu_available().await;
    let models_dir = models_dir.to_path_buf();
    let snapshot = tokio::task::spawn_blocking(move || {
        model_catalog.build_model_catalog_snapshot(&models_dir)
    })
    .await
    .map_err(|error| {
        ApiServerPlatformError::information(format!("Failed to build model snapshot: {error}"))
    })?
    .map_err(|error| ApiServerPlatformError::information(error.to_string()))?;

    let installed_models = snapshot
        .models
        .iter()
        .filter(|m| m.is_installed)
        .map(|m| m.id.clone())
        .collect::<Vec<_>>();
    let vad_installed = snapshot.models.iter().any(|m| {
        m.id == sona_core::models::preset_models::DEFAULT_SILERO_VAD_MODEL_ID && m.is_installed
    });
    let punctuation_installed = snapshot.models.iter().any(|m| {
        m.id == sona_core::models::preset_models::DEFAULT_PUNCTUATION_MODEL_ID && m.is_installed
    });

    let online_asr_providers = online_asr_providers()
        .iter()
        .map(|provider| {
            let configured = online_asr_config
                .get(&provider.id)
                .and_then(|config| config.get("apiKey"))
                .and_then(serde_json::Value::as_str)
                .is_some_and(|api_key| !api_key.is_empty());
            OnlineAsrProviderInfo {
                id: provider.id.clone(),
                configured,
                supports_batch: provider.batch.local_file_mode.supported,
                supports_streaming: provider.streaming.supported.unwrap_or(false),
            }
        })
        .collect();

    Ok(InfoResponse {
        platform: std::env::consts::OS.to_string(),
        gpu_available,
        models: installed_models,
        vad_installed,
        punctuation_installed,
        online_asr_providers,
    })
}

pub fn parse_ip_whitelist(whitelist_str: &str) -> Result<Vec<IpNet>, ApiServerConfigurationError> {
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
                return Err(ApiServerConfigurationError::InvalidIpWildcard {
                    rule: rule.to_string(),
                });
            } else {
                return Err(ApiServerConfigurationError::InvalidIpWildcard {
                    rule: rule.to_string(),
                });
            }
        } else {
            return Err(ApiServerConfigurationError::InvalidIpRule {
                rule: rule.to_string(),
            });
        }
    }

    if nets.is_empty() {
        nets.push("127.0.0.0/8".parse().unwrap());
        nets.push("::1/128".parse().unwrap());
    }

    Ok(nets)
}

pub fn format_bind_error(error: std::io::Error, address: &str) -> ApiServerBindError {
    ApiServerBindError::from_io(error, address)
}

pub fn prepare_runtime_config(
    parts: ApiServerRuntimeParts,
) -> Result<PreparedApiServerRuntime, ApiServerConfigurationError> {
    let ApiServerRuntimeParts {
        resolved,
        temp_dir,
        online_asr_config,
        batch_transcriber,
        media_validator,
        gpu_availability,
        model_catalog,
        batch_plan_resolver,
        platform,
        streaming_router,
        shutdown_rx,
        bind_tx,
    } = parts;
    let parsed_whitelist = parse_ip_whitelist(&resolved.ip_whitelist)?;
    let normalized_ip_whitelist = parsed_whitelist
        .iter()
        .map(|net| net.to_string())
        .collect::<Vec<_>>()
        .join(",");

    Ok(PreparedApiServerRuntime {
        config: ApiServerRuntimeConfig {
            host: resolved.host,
            port: resolved.port,
            api_key: resolved.api_key,
            temp_dir,
            models_dir: resolved.models_dir,
            max_concurrent: resolved.max_concurrent,
            max_queue_size: resolved.max_queue_size,
            max_upload_size_mb: resolved.max_upload_size_mb,
            job_ttl_minutes: resolved.job_ttl_minutes,
            max_streaming: resolved.max_streaming,
            ip_whitelist: Arc::new(parsed_whitelist),
            online_asr_config,
            transcription_defaults: ApiServerTranscriptionDefaults {
                gpu_acceleration: resolved.transcription_defaults.gpu_acceleration,
                vad_model_id: resolved.transcription_defaults.vad_model_id,
                punctuation_model_id: resolved.transcription_defaults.punctuation_model_id,
            },
            batch_transcriber,
            media_validator,
            gpu_availability,
            model_catalog,
            batch_plan_resolver,
            platform,
            streaming_router,
            shutdown_rx,
            bind_tx,
        },
        normalized_ip_whitelist,
    })
}

pub async fn start_api_server_runtime(
    parts: ApiServerServiceParts,
) -> Result<RunningApiServer, ApiServerStartError> {
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let (bind_tx, bind_rx) = tokio::sync::oneshot::channel();
    let prepared = prepare_runtime_config(ApiServerRuntimeParts {
        resolved: parts.resolved,
        temp_dir: parts.temp_dir,
        online_asr_config: parts.online_asr_config,
        batch_transcriber: parts.batch_transcriber,
        media_validator: parts.media_validator,
        gpu_availability: parts.gpu_availability,
        model_catalog: parts.model_catalog,
        batch_plan_resolver: parts.batch_plan_resolver,
        platform: parts.platform,
        streaming_router: parts.streaming_router,
        shutdown_rx,
        bind_tx: Some(bind_tx),
    })
    .map_err(ApiServerStartError::Configuration)?;
    let normalized_ip_whitelist = prepared.normalized_ip_whitelist.clone();
    let join_handle = tokio::spawn(async move {
        let result = run_server(prepared.config).await;
        if let Err(error) = &result {
            log::error!("HTTP API Server failed: {}", error);
        }
        result
    });

    match bind_rx.await {
        Ok(Ok(dashboard)) => Ok(RunningApiServer {
            normalized_ip_whitelist,
            shutdown_tx: Some(shutdown_tx),
            join_handle,
            dashboard,
        }),
        Ok(Err(error)) => {
            let _ = shutdown_tx.send(());
            let _ = join_handle.await;
            Err(ApiServerStartError::Runtime(error))
        }
        Err(_) => {
            let _ = shutdown_tx.send(());
            let error = startup_channel_closed_error(join_handle).await;
            Err(ApiServerStartError::Runtime(error))
        }
    }
}

async fn startup_channel_closed_error(
    join_handle: tokio::task::JoinHandle<Result<(), ApiServerRuntimeError>>,
) -> ApiServerRuntimeError {
    join_handle
        .await
        .map_err(|join_error| ApiServerRuntimeError::TaskJoin {
            reason: join_error.to_string(),
        })
        .and_then(|result| result)
        .err()
        .unwrap_or(ApiServerRuntimeError::DashboardChannelClosed)
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

async fn build_health_response(state: &ServerState) -> HealthResponse {
    let uptime = state.start_time.elapsed().as_secs();

    let cache_space_bytes = tokio::task::spawn_blocking({
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

    HealthResponse {
        status: "ok".to_string(),
        uptime,
        active_jobs,
        pending_jobs,
        cache_space_bytes,
    }
}

pub async fn handle_health(State(state): State<ServerState>) -> Json<HealthResponse> {
    Json(build_health_response(&state).await)
}

pub async fn handle_info(
    State(state): State<ServerState>,
) -> Result<Json<InfoResponse>, (StatusCode, String)> {
    let configs = state.online_asr_config.read().await.clone();
    let info = build_info_response(
        Arc::clone(&state.gpu_availability),
        Arc::clone(&state.model_catalog),
        &state.models_dir,
        &configs,
    )
    .await
    .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(Json(info))
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
    Json(state.job_manager.list_jobs().await)
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
                && !state.media_validator.is_valid_media_file(path).await
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

    if let Some(provider) = find_online_asr_provider(&m_id) {
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
        .map_err(|error| (StatusCode::SERVICE_UNAVAILABLE, error.to_string()))?;

    Ok(Json(serde_json::json!({ "job_id": job_id })))
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
        _ => return,
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
    transcription_defaults: ApiServerTranscriptionDefaults,
    batch_transcriber: Arc<dyn BatchTranscriber>,
    batch_plan_resolver: Arc<dyn BatchTranscribePlanResolver>,
    platform: Arc<dyn ApiServerPlatform>,
) {
    let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrent));

    while let Some(job) = receiver.recv().await {
        let job_manager = job_manager.clone();
        let models_dir = models_dir.clone();
        let semaphore = semaphore.clone();
        let defaults = transcription_defaults.clone();
        let batch_transcriber = batch_transcriber.clone();
        let batch_plan_resolver = batch_plan_resolver.clone();
        let platform = platform.clone();

        tokio::spawn(async move {
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
                    let request = OnlineBatchRequest {
                        file_path: job.file_path.clone(),
                        provider_id,
                        profile_id: job.model_id.clone(),
                        config: job.online_provider_config.clone().unwrap_or_default(),
                        language: if job.language == "auto" {
                            "".to_string()
                        } else {
                            job.language.clone()
                        },
                        hotwords: job.hotwords.clone(),
                    };
                    match platform.transcribe_online_batch(request).await {
                        Ok(segments) => JobStatus::Completed(segments),
                        Err(error) => JobStatus::Failed(error.to_string()),
                    }
                } else {
                    JobStatus::Failed("Missing online provider ID".to_string())
                }
            } else {
                let options = build_local_transcribe_options(&job, &models_dir, &defaults);
                match batch_plan_resolver.resolve_batch_transcribe_plan(options) {
                    Ok(plan) => match batch_transcriber.transcribe(plan).await {
                        Ok(segments) => JobStatus::Completed(segments),
                        Err(e) => JobStatus::Failed(e.to_string()),
                    },
                    Err(RuntimeCapabilityError::BatchPlan { reason }) => JobStatus::Failed(reason),
                    Err(error) => JobStatus::Failed(error.to_string()),
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

            let _ = tokio::fs::remove_file(&job.file_path).await;
        });
    }
}

pub fn build_local_transcribe_options(
    job: &TranscriptionJob,
    models_dir: &StdPath,
    defaults: &ApiServerTranscriptionDefaults,
) -> BatchTranscribeOptions {
    let (vad_model_id, punctuation_model_id) =
        companion_defaults_for_model(&job.model_id, defaults);
    BatchTranscribeOptions {
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
        default_models_dir: None,
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
    let rules = sona_core::models::preset_models::find_preset_model(model_id)
        .map(|model| model.resolved_rules());

    let vad_model_id = match defaults.vad_model_id.as_deref() {
        Some(id)
            if rules.map(|rules| rules.requires_vad).unwrap_or(true)
                || id != sona_core::models::preset_models::DEFAULT_SILERO_VAD_MODEL_ID =>
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
                || id != sona_core::models::preset_models::DEFAULT_PUNCTUATION_MODEL_ID =>
        {
            Some(id.to_string())
        }
        _ => None,
    };

    (vad_model_id, punctuation_model_id)
}

pub async fn run_server(config: ApiServerRuntimeConfig) -> Result<(), ApiServerRuntimeError> {
    let ApiServerRuntimeConfig {
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
        batch_transcriber,
        media_validator,
        gpu_availability,
        model_catalog,
        batch_plan_resolver,
        platform,
        streaming_router,
        shutdown_rx,
        bind_tx,
    } = config;

    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    if let Err(e) = tokio::fs::create_dir_all(&temp_dir).await {
        let error = ApiServerRuntimeError::FileSystem(FileSystemError::new(
            FileSystemOperation::CreateDirectory,
            &temp_dir,
            e.to_string(),
        ));
        if let Some(tx) = bind_tx {
            let _ = tx.send(Err(error.clone()));
        }
        return Err(error);
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
    let worker_defaults = transcription_defaults.clone();
    let worker_batch_transcriber = batch_transcriber.clone();
    let worker_batch_plan_resolver = batch_plan_resolver.clone();
    let worker_platform = platform.clone();

    tokio::spawn(async move {
        start_worker_loop(
            rx,
            job_manager_clone,
            models_dir_clone,
            max_concurrent,
            worker_defaults,
            worker_batch_transcriber,
            worker_batch_plan_resolver,
            worker_platform,
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

    let state = ServerState {
        job_manager,
        temp_dir: temp_dir.clone(),
        models_dir,
        start_time: std::time::Instant::now(),
        api_key: api_key.clone(),
        streaming_semaphore: Arc::new(tokio::sync::Semaphore::new(max_streaming)),
        ip_whitelist: ip_whitelist.clone(),
        online_asr_config,
        transcription_defaults,
        media_validator,
        gpu_availability,
        model_catalog,
        batch_plan_resolver,
        platform,
    };

    let cors = CorsLayer::new()
        .allow_origin(CorsAny)
        .allow_methods(CorsAny)
        .allow_headers(CorsAny);

    let router = Router::new().route("/health", get(handle_health));

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

    let streaming_router = streaming_router
        .unwrap_or_default()
        .with_state(state.clone());
    let router = router
        .merge(streaming_router)
        .merge(api_router)
        .layer(cors)
        .with_state(state.clone());
    let addr = format!("{}:{}", host, port);
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            let error = ApiServerRuntimeError::Bind(format_bind_error(e, &addr));
            if let Some(tx) = bind_tx {
                let _ = tx.send(Err(error.clone()));
            }
            return Err(error);
        }
    };
    if let Some(tx) = bind_tx {
        let _ = tx.send(Ok(ApiServerDashboardHandle {
            state: state.clone(),
        }));
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
    .map_err(|error| ApiServerRuntimeError::Serve {
        reason: error.to_string(),
    });

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

pub fn online_batch_request_to_core_request(
    request: &OnlineBatchRequest,
) -> sona_core::ports::asr::AsrTranscriptionRequest {
    sona_core::ports::asr::AsrTranscriptionRequest {
        engine_config: sona_core::ports::asr::AsrEngineConfig::Online {
            provider: OnlineAsrProviderRequest {
                provider_id: request.provider_id.clone(),
                profile_id: request.profile_id.clone(),
                config: request.config.clone(),
            },
        },
        mode: sona_core::ports::asr::AsrMode::Batch,
        enable_itn: false,
        language: request.language.clone(),
        hotwords: request.hotwords.clone(),
        speaker_processing: None,
        normalization_options: Default::default(),
        postprocess_options: Default::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use sona_core::models::preset_models::{
        DEFAULT_SILERO_VAD_MODEL_ID, ModelCatalogSnapshot,
        build_model_catalog_snapshot_with_installed_ids,
    };
    use sona_core::ports::runtime::{
        BatchTranscribePlanResolver, GpuAvailabilityProvider, MediaFileValidator,
        ModelCatalogProvider, RuntimeCapabilityError,
    };
    use sona_core::transcription::runtime::{BatchTranscribePlan, OutputTarget};
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tower::ServiceExt;

    struct RejectingMediaValidator;

    #[async_trait]
    impl MediaFileValidator for RejectingMediaValidator {
        async fn is_valid_media_file(&self, _path: &StdPath) -> bool {
            false
        }
    }

    struct AcceptingMediaValidator;

    #[async_trait]
    impl MediaFileValidator for AcceptingMediaValidator {
        async fn is_valid_media_file(&self, _path: &StdPath) -> bool {
            true
        }
    }

    struct FixedGpuAvailability(bool);

    #[async_trait]
    impl GpuAvailabilityProvider for FixedGpuAvailability {
        async fn is_gpu_available(&self) -> bool {
            self.0
        }
    }

    struct FixedModelCatalog {
        snapshot: ModelCatalogSnapshot,
    }

    impl ModelCatalogProvider for FixedModelCatalog {
        fn build_model_catalog_snapshot(
            &self,
            _models_dir: &StdPath,
        ) -> Result<ModelCatalogSnapshot, RuntimeCapabilityError> {
            Ok(self.snapshot.clone())
        }
    }

    struct FailingModelCatalog;

    impl ModelCatalogProvider for FailingModelCatalog {
        fn build_model_catalog_snapshot(
            &self,
            _models_dir: &StdPath,
        ) -> Result<ModelCatalogSnapshot, RuntimeCapabilityError> {
            Err(RuntimeCapabilityError::ModelCatalog {
                reason: "catalog unavailable".to_string(),
            })
        }
    }

    struct RecordingBatchPlanResolver {
        calls: Arc<AtomicUsize>,
        plan: BatchTranscribePlan,
    }

    impl BatchTranscribePlanResolver for RecordingBatchPlanResolver {
        fn resolve_batch_transcribe_plan(
            &self,
            _options: BatchTranscribeOptions,
        ) -> Result<BatchTranscribePlan, RuntimeCapabilityError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.plan.clone())
        }
    }

    struct RecordingBatchTranscriber {
        resolver_calls: Arc<AtomicUsize>,
        calls: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl BatchTranscriber for RecordingBatchTranscriber {
        async fn transcribe(
            &self,
            _plan: BatchTranscribePlan,
        ) -> Result<Vec<TranscriptSegment>, sona_core::ports::asr::AsrPortError> {
            assert_eq!(self.resolver_calls.load(Ordering::SeqCst), 1);
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(vec![])
        }
    }

    struct NoopBatchTranscriber;

    #[async_trait]
    impl BatchTranscriber for NoopBatchTranscriber {
        async fn transcribe(
            &self,
            _plan: BatchTranscribePlan,
        ) -> Result<Vec<TranscriptSegment>, sona_core::ports::asr::AsrPortError> {
            Ok(vec![])
        }
    }

    fn test_batch_plan(input_path: PathBuf) -> BatchTranscribePlan {
        BatchTranscribePlan {
            input_path,
            save_to_path: None,
            model_path: "C:/models/test".to_string(),
            num_threads: 4,
            enable_itn: false,
            language: "auto".to_string(),
            punctuation_model: None,
            vad_model: None,
            vad_buffer: 5.0,
            model_type: "whisper".to_string(),
            file_config: None,
            hotwords: None,
            gpu_acceleration: None,
            export_format: sona_core::export::ExportFormat::Json,
            output_target: OutputTarget::Stdout,
            quiet: true,
        }
    }

    fn test_batch_transcriber() -> Arc<dyn BatchTranscriber> {
        Arc::new(NoopBatchTranscriber)
    }

    fn test_model_catalog() -> Arc<dyn ModelCatalogProvider> {
        Arc::new(FixedModelCatalog {
            snapshot: build_model_catalog_snapshot_with_installed_ids(
                StdPath::new("models"),
                &std::collections::HashSet::new(),
            ),
        })
    }

    fn test_batch_plan_resolver() -> Arc<dyn BatchTranscribePlanResolver> {
        Arc::new(RecordingBatchPlanResolver {
            calls: Arc::new(AtomicUsize::new(0)),
            plan: test_batch_plan(PathBuf::from("sample.wav")),
        })
    }

    #[tokio::test]
    async fn injected_runtime_capability_rejects_invalid_media_and_removes_upload() {
        let temp = tempfile::tempdir().unwrap();
        let (tx, _rx) = mpsc::channel(1);
        let state = ServerState {
            job_manager: JobManager::new(tx),
            temp_dir: temp.path().to_path_buf(),
            models_dir: PathBuf::from("models"),
            start_time: std::time::Instant::now(),
            api_key: String::new(),
            streaming_semaphore: Arc::new(tokio::sync::Semaphore::new(1)),
            ip_whitelist: Arc::new(vec![]),
            online_asr_config: Arc::new(RwLock::new(HashMap::new())),
            transcription_defaults: ApiServerTranscriptionDefaults::default(),
            media_validator: Arc::new(RejectingMediaValidator),
            gpu_availability: Arc::new(FixedGpuAvailability(false)),
            model_catalog: Arc::new(FixedModelCatalog {
                snapshot: build_model_catalog_snapshot_with_installed_ids(
                    StdPath::new("models"),
                    &std::collections::HashSet::new(),
                ),
            }),
            batch_plan_resolver: Arc::new(RecordingBatchPlanResolver {
                calls: Arc::new(AtomicUsize::new(0)),
                plan: test_batch_plan(PathBuf::from("sample.wav")),
            }),
            platform: Arc::new(DefaultApiServerPlatform),
        };
        let boundary = "sona-test-boundary";
        let body = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"sample.wav\"\r\nContent-Type: audio/wav\r\n\r\ninvalid\r\n--{boundary}--\r\n"
        );
        let response = Router::new()
            .route("/v1/transcriptions", post(handle_transcribe))
            .with_state(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/transcriptions")
                    .header(
                        "content-type",
                        format!("multipart/form-data; boundary={boundary}"),
                    )
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn injected_runtime_capability_builds_info_from_gpu_and_catalog_ports() {
        let mut installed = std::collections::HashSet::new();
        installed.insert("sherpa-onnx-whisper-turbo".to_string());
        installed.insert(DEFAULT_SILERO_VAD_MODEL_ID.to_string());
        let snapshot = build_model_catalog_snapshot_with_installed_ids(
            StdPath::new("injected-models"),
            &installed,
        );
        let mut configs = HashMap::new();
        configs.insert(
            "groq-whisper".to_string(),
            serde_json::json!({"apiKey": "configured"}),
        );

        let info = build_info_response(
            Arc::new(FixedGpuAvailability(true)),
            Arc::new(FixedModelCatalog { snapshot }),
            StdPath::new("ignored-models"),
            &configs,
        )
        .await
        .unwrap();

        assert_eq!(info.platform, std::env::consts::OS);
        assert!(info.gpu_available);
        assert_eq!(
            info.models,
            vec![
                "sherpa-onnx-whisper-turbo".to_string(),
                DEFAULT_SILERO_VAD_MODEL_ID.to_string(),
            ]
        );
        assert!(info.vad_installed);
        assert!(!info.punctuation_installed);
        assert!(
            info.online_asr_providers
                .iter()
                .any(|provider| provider.id == "groq-whisper" && provider.configured)
        );
    }

    #[tokio::test]
    async fn injected_runtime_capability_maps_catalog_failure() {
        let error = build_info_response(
            Arc::new(FixedGpuAvailability(false)),
            Arc::new(FailingModelCatalog),
            StdPath::new("models"),
            &HashMap::new(),
        )
        .await
        .unwrap_err();

        assert_eq!(
            error,
            ApiServerPlatformError::Information {
                reason: "Model catalog discovery failed: catalog unavailable".to_string(),
            }
        );
    }

    #[tokio::test]
    async fn injected_runtime_capability_resolves_local_plan_before_transcription() {
        let temp = tempfile::tempdir().unwrap();
        let input_path = temp.path().join("sample.wav");
        fs::write(&input_path, b"audio").unwrap();
        let resolver_calls = Arc::new(AtomicUsize::new(0));
        let transcriber_calls = Arc::new(AtomicUsize::new(0));
        let (tx, rx) = mpsc::channel(1);
        let job_manager = JobManager::new(tx);
        let worker = tokio::spawn(start_worker_loop(
            rx,
            job_manager.clone(),
            PathBuf::from("models"),
            1,
            ApiServerTranscriptionDefaults::default(),
            Arc::new(RecordingBatchTranscriber {
                resolver_calls: Arc::clone(&resolver_calls),
                calls: Arc::clone(&transcriber_calls),
            }),
            Arc::new(RecordingBatchPlanResolver {
                calls: Arc::clone(&resolver_calls),
                plan: test_batch_plan(input_path.clone()),
            }),
            Arc::new(DefaultApiServerPlatform),
        ));
        let job = TranscriptionJob {
            job_id: "injected-plan-job".to_string(),
            file_path: input_path,
            model_id: "model".to_string(),
            language: "auto".to_string(),
            hotwords: None,
            webhook_url: None,
            webhook_secret: None,
            engine: "LocalSherpa".to_string(),
            online_provider_id: None,
            online_provider_config: None,
        };
        job_manager.submit_job(job).await.unwrap();

        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                if matches!(
                    job_manager.get_job("injected-plan-job").await,
                    Some(JobStatus::Completed(_))
                ) {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        worker.abort();

        assert_eq!(resolver_calls.load(Ordering::SeqCst), 1);
        assert_eq!(transcriber_calls.load(Ordering::SeqCst), 1);
    }

    fn streaming_authorization_state(
        api_key: &str,
        max_streaming: usize,
        ip_whitelist: &str,
    ) -> ServerState {
        let (tx, _rx) = mpsc::channel(1);
        ServerState {
            job_manager: JobManager::new(tx),
            temp_dir: PathBuf::from("temp"),
            models_dir: PathBuf::from("models"),
            start_time: std::time::Instant::now(),
            api_key: api_key.to_string(),
            streaming_semaphore: Arc::new(tokio::sync::Semaphore::new(max_streaming)),
            ip_whitelist: Arc::new(parse_ip_whitelist(ip_whitelist).unwrap()),
            online_asr_config: Arc::new(RwLock::new(HashMap::new())),
            transcription_defaults: ApiServerTranscriptionDefaults::default(),
            media_validator: Arc::new(AcceptingMediaValidator),
            gpu_availability: Arc::new(FixedGpuAvailability(false)),
            model_catalog: test_model_catalog(),
            batch_plan_resolver: test_batch_plan_resolver(),
            platform: Arc::new(DefaultApiServerPlatform),
        }
    }

    fn test_dashboard_handle(
        model_catalog: Arc<dyn ModelCatalogProvider>,
    ) -> ApiServerDashboardHandle {
        let (tx, _rx) = mpsc::channel(1);
        ApiServerDashboardHandle {
            state: ServerState {
                job_manager: JobManager::new(tx),
                temp_dir: PathBuf::from("temp"),
                models_dir: PathBuf::from("models"),
                start_time: std::time::Instant::now(),
                api_key: String::new(),
                streaming_semaphore: Arc::new(tokio::sync::Semaphore::new(1)),
                ip_whitelist: Arc::new(vec![]),
                online_asr_config: Arc::new(RwLock::new(HashMap::new())),
                transcription_defaults: ApiServerTranscriptionDefaults::default(),
                media_validator: Arc::new(AcceptingMediaValidator),
                gpu_availability: Arc::new(FixedGpuAvailability(false)),
                model_catalog,
                batch_plan_resolver: test_batch_plan_resolver(),
                platform: Arc::new(DefaultApiServerPlatform),
            },
        }
    }

    #[tokio::test]
    async fn clean_expired_jobs() {
        let (tx, _rx) = mpsc::channel(1);
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
    async fn submit_job_reports_closed_queue_without_leaving_pending_state() {
        let (tx, rx) = mpsc::channel(1);
        drop(rx);
        let job_manager = JobManager::new(tx);
        let job = TranscriptionJob {
            job_id: "closed-queue-job".to_string(),
            file_path: PathBuf::from("sample.wav"),
            model_id: "model".to_string(),
            language: "auto".to_string(),
            hotwords: None,
            webhook_url: None,
            webhook_secret: None,
            engine: "LocalSherpa".to_string(),
            online_provider_id: None,
            online_provider_config: None,
        };

        let error = job_manager.submit_job(job).await.unwrap_err();

        assert_eq!(
            error,
            ApiServerJobError::QueueClosed {
                job_id: "closed-queue-job".to_string(),
            }
        );
        assert!(job_manager.get_job("closed-queue-job").await.is_none());
    }

    #[tokio::test]
    async fn dashboard_snapshot_preserves_catalog_failure_category() {
        let dashboard = test_dashboard_handle(Arc::new(FailingModelCatalog));

        let error = match dashboard.snapshot().await {
            Ok(_) => panic!("failing info platform should reject dashboard snapshot"),
            Err(error) => error,
        };

        assert_eq!(
            error,
            ApiServerDashboardError::Platform(ApiServerPlatformError::Information {
                reason: "Model catalog discovery failed: catalog unavailable".to_string(),
            })
        );
    }

    #[tokio::test]
    async fn stop_reports_closed_shutdown_channel() {
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        drop(shutdown_rx);
        let server = RunningApiServer {
            normalized_ip_whitelist: "127.0.0.0/8".to_string(),
            shutdown_tx: Some(shutdown_tx),
            join_handle: tokio::spawn(async { Ok(()) }),
            dashboard: test_dashboard_handle(test_model_catalog()),
        };

        let error = server.stop().await.unwrap_err();

        assert_eq!(error, ApiServerStopError::ShutdownSignalClosed);
    }

    #[tokio::test]
    async fn wait_reports_task_join_failure() {
        let server = RunningApiServer {
            normalized_ip_whitelist: "127.0.0.0/8".to_string(),
            shutdown_tx: None,
            join_handle: tokio::spawn(async {
                panic!("test task failure");
                #[allow(unreachable_code)]
                Ok(())
            }),
            dashboard: test_dashboard_handle(test_model_catalog()),
        };

        let error = server.wait().await.unwrap_err();

        assert!(matches!(error, ApiServerRuntimeError::TaskJoin { .. }));
    }

    #[tokio::test]
    async fn startup_channel_closure_has_a_typed_runtime_error() {
        let join_handle = tokio::spawn(async { Ok(()) });

        let error = startup_channel_closed_error(join_handle).await;

        assert_eq!(error, ApiServerRuntimeError::DashboardChannelClosed);
    }

    #[tokio::test]
    async fn health_endpoint_reports_stats() {
        let temp_dir = tempfile::tempdir().unwrap().path().to_path_buf();
        let models_dir = tempfile::tempdir().unwrap().path().to_path_buf();
        let (tx, _rx) = mpsc::channel(1);
        let state = ServerState {
            job_manager: JobManager::new(tx),
            temp_dir,
            models_dir,
            start_time: std::time::Instant::now(),
            api_key: String::new(),
            streaming_semaphore: Arc::new(tokio::sync::Semaphore::new(1)),
            ip_whitelist: Arc::new(vec![]),
            online_asr_config: Arc::new(RwLock::new(HashMap::new())),
            transcription_defaults: ApiServerTranscriptionDefaults::default(),
            media_validator: Arc::new(AcceptingMediaValidator),
            gpu_availability: Arc::new(FixedGpuAvailability(false)),
            model_catalog: test_model_catalog(),
            batch_plan_resolver: test_batch_plan_resolver(),
            platform: Arc::new(DefaultApiServerPlatform),
        };

        let app = Router::new()
            .route("/health", get(handle_health))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(body["status"], "ok");
        assert!(body["uptime"].is_number());
        assert!(body["activeJobs"].is_number());
        assert!(body["pendingJobs"].is_number());
        assert!(body["cacheSpaceBytes"].is_number());
    }

    #[tokio::test]
    async fn list_jobs_endpoint_reports_known_jobs() {
        let temp_dir = tempfile::tempdir().unwrap().path().to_path_buf();
        let models_dir = tempfile::tempdir().unwrap().path().to_path_buf();
        let (tx, _rx) = mpsc::channel(1);
        let job_manager = JobManager::new(tx);
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
            api_key: String::new(),
            streaming_semaphore: Arc::new(tokio::sync::Semaphore::new(1)),
            ip_whitelist: Arc::new(vec![]),
            online_asr_config: Arc::new(RwLock::new(HashMap::new())),
            transcription_defaults: ApiServerTranscriptionDefaults::default(),
            media_validator: Arc::new(AcceptingMediaValidator),
            gpu_availability: Arc::new(FixedGpuAvailability(false)),
            model_catalog: test_model_catalog(),
            batch_plan_resolver: test_batch_plan_resolver(),
            platform: Arc::new(DefaultApiServerPlatform),
        };

        let app = Router::new()
            .route("/v1/transcriptions/jobs", get(handle_list_jobs))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/transcriptions/jobs")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert!(body.is_object());
        assert_eq!(body["test-job-id"], "Pending");
    }

    #[test]
    fn local_transcribe_request_uses_server_defaults() {
        let temp = tempfile::tempdir().unwrap();
        let models_dir = temp.path().to_path_buf();
        let input_path = temp.path().join("sample.wav");
        fs::write(&input_path, b"audio").unwrap();
        let job = TranscriptionJob {
            job_id: "job-1".to_string(),
            file_path: input_path.clone(),
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
            vad_model_id: Some(
                sona_core::models::preset_models::DEFAULT_SILERO_VAD_MODEL_ID.to_string(),
            ),
            punctuation_model_id: None,
        };

        let options = build_local_transcribe_options(&job, &models_dir, &defaults);

        assert_eq!(options.gpu_acceleration.as_deref(), Some("cuda"));
        assert_eq!(
            options.vad_model_id.as_deref(),
            Some(sona_core::models::preset_models::DEFAULT_SILERO_VAD_MODEL_ID)
        );
        assert!(options.punctuation_model_id.is_none());
        assert_eq!(options.input, input_path);
        assert_eq!(options.hotwords.as_deref(), Some("Sona"));
    }

    #[test]
    fn format_bind_error_describes_common_failures() {
        let addr = "127.0.0.1:14200";
        let in_use_err = std::io::Error::new(std::io::ErrorKind::AddrInUse, "address in use");
        let in_use = format_bind_error(in_use_err, addr);
        assert_eq!(in_use.address, addr);
        assert_eq!(in_use.kind, ApiServerBindErrorKind::AddressInUse);
        assert!(in_use.to_string().contains("Address already in use"));

        let not_avail_err =
            std::io::Error::new(std::io::ErrorKind::AddrNotAvailable, "not available");
        let not_available = format_bind_error(not_avail_err, addr);
        assert_eq!(
            not_available.kind,
            ApiServerBindErrorKind::AddressNotAvailable
        );
        assert!(not_available.to_string().contains("Address not available"));

        let permission_err =
            std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied");
        let permission = format_bind_error(permission_err, addr);
        assert_eq!(permission.kind, ApiServerBindErrorKind::PermissionDenied);
        assert!(permission.to_string().contains("Permission denied"));
    }

    #[test]
    fn authorize_streaming_request_rejects_non_whitelisted_clients() {
        let state = streaming_authorization_state("secret", 1, "127.0.0.0/8");

        let error = match authorize_streaming_request(
            &state,
            "10.0.0.1:14200".parse().unwrap(),
            Some("secret"),
        ) {
            Ok(_) => panic!("non-whitelisted streaming client should be rejected"),
            Err(error) => error,
        };

        assert_eq!(error, StatusCode::FORBIDDEN);
    }

    #[test]
    fn authorize_streaming_request_rejects_invalid_tokens() {
        let state = streaming_authorization_state("secret", 1, "127.0.0.0/8");

        let error = match authorize_streaming_request(
            &state,
            "127.0.0.1:14200".parse().unwrap(),
            Some("wrong"),
        ) {
            Ok(_) => panic!("invalid streaming token should be rejected"),
            Err(error) => error,
        };

        assert_eq!(error, StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn authorize_streaming_request_holds_streaming_capacity_permit() {
        let state = streaming_authorization_state("", 1, "127.0.0.0/8");

        let permit =
            authorize_streaming_request(&state, "127.0.0.1:14200".parse().unwrap(), None).unwrap();
        let error =
            match authorize_streaming_request(&state, "127.0.0.1:14200".parse().unwrap(), None) {
                Ok(_) => panic!("streaming capacity should be exhausted"),
                Err(error) => error,
            };
        drop(permit);
        let next = authorize_streaming_request(&state, "127.0.0.1:14200".parse().unwrap(), None);

        assert_eq!(error, StatusCode::SERVICE_UNAVAILABLE);
        assert!(next.is_ok());
    }

    #[test]
    fn prepare_runtime_config_maps_resolved_options_and_normalizes_whitelist() {
        let temp_dir = tempfile::tempdir().unwrap().path().join("api-temp");
        let models_dir = tempfile::tempdir().unwrap().path().join("models");
        let (_shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let (bind_tx, _bind_rx) = tokio::sync::oneshot::channel();

        let prepared = prepare_runtime_config(ApiServerRuntimeParts {
            resolved: sona_core::runtime::serve::ResolvedServeRuntimeOptions {
                host: "0.0.0.0".to_string(),
                port: 15555,
                api_key: "secret".to_string(),
                models_dir: models_dir.clone(),
                ip_whitelist: "localhost,10.0.0.0/8".to_string(),
                max_streaming: 5,
                max_concurrent: 3,
                max_queue_size: 44,
                max_upload_size_mb: 256,
                job_ttl_minutes: 9,
                transcription_defaults: sona_core::runtime::serve::ServeTranscriptionDefaults {
                    gpu_acceleration: Some("cuda".to_string()),
                    vad_model_id: Some("vad-model".to_string()),
                    punctuation_model_id: Some("punct-model".to_string()),
                },
            },
            temp_dir: temp_dir.clone(),
            online_asr_config: Arc::new(RwLock::new(HashMap::new())),
            batch_transcriber: test_batch_transcriber(),
            media_validator: Arc::new(AcceptingMediaValidator),
            gpu_availability: Arc::new(FixedGpuAvailability(false)),
            model_catalog: test_model_catalog(),
            batch_plan_resolver: test_batch_plan_resolver(),
            platform: Arc::new(DefaultApiServerPlatform),
            streaming_router: None,
            shutdown_rx,
            bind_tx: Some(bind_tx),
        })
        .unwrap();

        assert_eq!(
            prepared.normalized_ip_whitelist,
            "127.0.0.0/8,::1/128,10.0.0.0/8"
        );
        assert_eq!(prepared.config.host, "0.0.0.0");
        assert_eq!(prepared.config.port, 15555);
        assert_eq!(prepared.config.api_key, "secret");
        assert_eq!(prepared.config.temp_dir, temp_dir);
        assert_eq!(prepared.config.models_dir, models_dir);
        assert_eq!(prepared.config.max_concurrent, 3);
        assert_eq!(prepared.config.max_queue_size, 44);
        assert_eq!(prepared.config.max_upload_size_mb, 256);
        assert_eq!(prepared.config.job_ttl_minutes, 9);
        assert_eq!(prepared.config.max_streaming, 5);
        assert_eq!(
            prepared
                .config
                .transcription_defaults
                .gpu_acceleration
                .as_deref(),
            Some("cuda")
        );
        assert_eq!(
            prepared
                .config
                .transcription_defaults
                .vad_model_id
                .as_deref(),
            Some("vad-model")
        );
        assert_eq!(
            prepared
                .config
                .transcription_defaults
                .punctuation_model_id
                .as_deref(),
            Some("punct-model")
        );
    }

    #[test]
    fn prepare_runtime_config_rejects_invalid_whitelist() {
        let (_shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let error = match prepare_runtime_config(ApiServerRuntimeParts {
            resolved: sona_core::runtime::serve::ResolvedServeRuntimeOptions {
                host: "127.0.0.1".to_string(),
                port: 14200,
                api_key: String::new(),
                models_dir: PathBuf::from("models"),
                ip_whitelist: "not-a-rule".to_string(),
                max_streaming: 1,
                max_concurrent: 1,
                max_queue_size: 1,
                max_upload_size_mb: 1,
                job_ttl_minutes: 1,
                transcription_defaults: Default::default(),
            },
            temp_dir: PathBuf::from("temp"),
            online_asr_config: Arc::new(RwLock::new(HashMap::new())),
            batch_transcriber: test_batch_transcriber(),
            media_validator: Arc::new(AcceptingMediaValidator),
            gpu_availability: Arc::new(FixedGpuAvailability(false)),
            model_catalog: test_model_catalog(),
            batch_plan_resolver: test_batch_plan_resolver(),
            platform: Arc::new(DefaultApiServerPlatform),
            streaming_router: None,
            shutdown_rx,
            bind_tx: None,
        }) {
            Ok(_) => panic!("invalid whitelist should be rejected"),
            Err(error) => error,
        };

        assert_eq!(
            error,
            ApiServerConfigurationError::InvalidIpRule {
                rule: "not-a-rule".to_string(),
            }
        );
    }

    #[tokio::test]
    async fn run_server_reports_bind_failure() {
        let occupier = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = occupier.local_addr().unwrap().port();
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let (bind_tx, bind_rx) = tokio::sync::oneshot::channel();
        let temp_dir = tempfile::tempdir().unwrap().path().to_path_buf();
        let models_dir = tempfile::tempdir().unwrap().path().to_path_buf();

        let config = ApiServerRuntimeConfig {
            host: "127.0.0.1".to_string(),
            port,
            api_key: String::new(),
            temp_dir,
            models_dir,
            max_concurrent: 1,
            max_queue_size: 0,
            max_upload_size_mb: 1,
            job_ttl_minutes: 1,
            max_streaming: 1,
            ip_whitelist: Arc::new(vec![]),
            online_asr_config: Arc::new(RwLock::new(HashMap::new())),
            transcription_defaults: ApiServerTranscriptionDefaults::default(),
            batch_transcriber: test_batch_transcriber(),
            media_validator: Arc::new(AcceptingMediaValidator),
            gpu_availability: Arc::new(FixedGpuAvailability(false)),
            model_catalog: test_model_catalog(),
            batch_plan_resolver: test_batch_plan_resolver(),
            platform: Arc::new(DefaultApiServerPlatform),
            streaming_router: None,
            shutdown_rx,
            bind_tx: Some(bind_tx),
        };

        let handle = tokio::spawn(async move { run_server(config).await });
        let bind_error = match bind_rx.await.unwrap() {
            Ok(_) => panic!("occupied port should fail to bind"),
            Err(error) => error,
        };
        assert!(matches!(
            bind_error,
            ApiServerRuntimeError::Bind(ApiServerBindError {
                ref address,
                kind: ApiServerBindErrorKind::AddressInUse,
                ..
            }) if address == &format!("127.0.0.1:{port}")
        ));

        let _ = shutdown_tx.send(());
        let _ = handle.await;
    }

    #[tokio::test]
    async fn start_api_server_runtime_reports_bind_failure() {
        let occupier = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = occupier.local_addr().unwrap().port();
        let temp_dir = tempfile::tempdir().unwrap().path().join("api-temp");
        let models_dir = tempfile::tempdir().unwrap().path().join("models");

        let result = start_api_server_runtime(ApiServerServiceParts {
            resolved: sona_core::runtime::serve::ResolvedServeRuntimeOptions {
                host: "127.0.0.1".to_string(),
                port,
                api_key: String::new(),
                models_dir,
                ip_whitelist: "localhost".to_string(),
                max_streaming: 1,
                max_concurrent: 1,
                max_queue_size: 1,
                max_upload_size_mb: 1,
                job_ttl_minutes: 1,
                transcription_defaults: Default::default(),
            },
            temp_dir,
            online_asr_config: Arc::new(RwLock::new(HashMap::new())),
            batch_transcriber: test_batch_transcriber(),
            media_validator: Arc::new(AcceptingMediaValidator),
            gpu_availability: Arc::new(FixedGpuAvailability(false)),
            model_catalog: test_model_catalog(),
            batch_plan_resolver: test_batch_plan_resolver(),
            platform: Arc::new(DefaultApiServerPlatform),
            streaming_router: None,
        })
        .await;

        let error = result.expect_err("occupied port should fail to bind");
        assert!(matches!(
            error,
            ApiServerStartError::Runtime(ApiServerRuntimeError::Bind(ApiServerBindError {
                kind: ApiServerBindErrorKind::AddressInUse,
                ..
            }))
        ));
        assert!(error.to_string().contains("Address already in use"));
    }

    #[tokio::test]
    async fn start_api_server_runtime_reports_configuration_failure() {
        let temp_dir = tempfile::tempdir().unwrap().path().join("api-temp");
        let models_dir = tempfile::tempdir().unwrap().path().join("models");

        let result = start_api_server_runtime(ApiServerServiceParts {
            resolved: sona_core::runtime::serve::ResolvedServeRuntimeOptions {
                host: "127.0.0.1".to_string(),
                port: 14200,
                api_key: String::new(),
                models_dir,
                ip_whitelist: "not-a-rule".to_string(),
                max_streaming: 1,
                max_concurrent: 1,
                max_queue_size: 1,
                max_upload_size_mb: 1,
                job_ttl_minutes: 1,
                transcription_defaults: Default::default(),
            },
            temp_dir,
            online_asr_config: Arc::new(RwLock::new(HashMap::new())),
            batch_transcriber: test_batch_transcriber(),
            media_validator: Arc::new(AcceptingMediaValidator),
            gpu_availability: Arc::new(FixedGpuAvailability(false)),
            model_catalog: test_model_catalog(),
            batch_plan_resolver: test_batch_plan_resolver(),
            platform: Arc::new(DefaultApiServerPlatform),
            streaming_router: None,
        })
        .await;

        let error = result.expect_err("invalid whitelist should fail before starting");
        assert_eq!(
            error,
            ApiServerStartError::Configuration(ApiServerConfigurationError::InvalidIpRule {
                rule: "not-a-rule".to_string(),
            })
        );
    }

    #[tokio::test]
    async fn start_api_server_runtime_returns_stoppable_server_with_dashboard_snapshot() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let temp_dir = tempfile::tempdir().unwrap().path().join("api-temp");
        let models_dir = tempfile::tempdir().unwrap().path().join("models");

        let server = start_api_server_runtime(ApiServerServiceParts {
            resolved: sona_core::runtime::serve::ResolvedServeRuntimeOptions {
                host: "127.0.0.1".to_string(),
                port,
                api_key: String::new(),
                models_dir,
                ip_whitelist: "localhost".to_string(),
                max_streaming: 1,
                max_concurrent: 1,
                max_queue_size: 1,
                max_upload_size_mb: 1,
                job_ttl_minutes: 1,
                transcription_defaults: Default::default(),
            },
            temp_dir,
            online_asr_config: Arc::new(RwLock::new(HashMap::new())),
            batch_transcriber: test_batch_transcriber(),
            media_validator: Arc::new(AcceptingMediaValidator),
            gpu_availability: Arc::new(FixedGpuAvailability(false)),
            model_catalog: test_model_catalog(),
            batch_plan_resolver: test_batch_plan_resolver(),
            platform: Arc::new(DefaultApiServerPlatform),
            streaming_router: None,
        })
        .await
        .unwrap();

        assert_eq!(server.normalized_ip_whitelist, "127.0.0.0/8,::1/128");
        let snapshot = server.dashboard_snapshot().await.unwrap();
        assert_eq!(snapshot.health.status, "ok");
        assert_eq!(snapshot.health.active_jobs, 0);
        assert_eq!(snapshot.health.pending_jobs, 0);
        assert!(snapshot.jobs.is_empty());
        server.stop().await.unwrap();
    }
}
