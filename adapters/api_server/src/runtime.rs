use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    Router,
    routing::{get, post},
};
use ipnet::IpNet;
use sona_core::ports::asr::BatchTranscriber;
use sona_core::ports::fs::{FileSystemError, FileSystemOperation};
use sona_core::ports::runtime::{
    BatchTranscribePlanResolver, GpuAvailabilityProvider, MediaFileValidator, ModelCatalogProvider,
};
use sona_core::runtime::serve::ResolvedServeRuntimeOptions;
use tokio::sync::{RwLock, mpsc};
use tower_http::{
    cors::{Any as CorsAny, CorsLayer},
    validate_request::ValidateRequestHeaderLayer,
};

use crate::ApiServerBindError;
use crate::ApiServerConfigurationError;
use crate::ApiServerDashboardError;
use crate::ApiServerRuntimeError;
use crate::ApiServerStartError;
use crate::ApiServerStopError;
use crate::handlers::{
    handle_health, handle_info, handle_job_status, handle_list_jobs, handle_transcribe,
    ip_whitelist_middleware,
};
use crate::info::{HealthResponse, InfoResponse, build_health_response, build_info_response};
use crate::ip_whitelist::parse_ip_whitelist;
use crate::jobs::{JobManager, JobStatus};
use crate::platform::{ApiServerPlatform, ApiServerTranscriptionDefaults};
use crate::state::ServerState;
use crate::worker::{TranscriptionWorkerDeps, start_worker_loop};

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
    pub(crate) dashboard: ApiServerDashboardHandle,
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
    pub(crate) state: ServerState,
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

pub(crate) async fn startup_channel_closed_error(
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
            TranscriptionWorkerDeps {
                job_manager: job_manager_clone,
                models_dir: models_dir_clone,
                max_concurrent,
                transcription_defaults: worker_defaults,
                batch_transcriber: worker_batch_transcriber,
                batch_plan_resolver: worker_batch_plan_resolver,
                platform: worker_platform,
            },
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
