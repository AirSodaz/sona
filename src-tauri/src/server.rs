use axum::{
    Json, Router,
    extract::{Multipart, Path, State},
    http::StatusCode,
    routing::{get, post},
};
use futures_util::stream::StreamExt;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::{RwLock, mpsc};
use tower_http::{
    cors::{Any, CorsLayer},
    validate_request::ValidateRequestHeaderLayer,
};

type HmacSha256 = Hmac<Sha256>;

use crate::cli::{TranscribeCliOptions, resolve_transcribe_options};
use crate::sherpa::transcribe_batch_with_progress;

#[derive(Debug, Clone, serde::Serialize)]
pub enum JobStatus {
    Pending,
    Processing,
    Completed(Vec<crate::sherpa::TranscriptSegment>),
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
}

#[derive(Clone)]
pub struct JobManager {
    jobs: Arc<RwLock<HashMap<String, JobStatus>>>,
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
        self.jobs
            .write()
            .await
            .insert(job.job_id.clone(), JobStatus::Pending);
        self.sender.send(job).await.map_err(|e| e.to_string())
    }

    pub async fn update_job(&self, job_id: &str, status: JobStatus) {
        if let Some(job) = self.jobs.write().await.get_mut(job_id) {
            *job = status;
        }
    }

    pub async fn get_job(&self, job_id: &str) -> Option<JobStatus> {
        self.jobs.read().await.get(job_id).cloned()
    }

    pub async fn list_jobs(&self) -> HashMap<String, JobStatus> {
        self.jobs.read().await.clone()
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

    let client = reqwest::Client::new();
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

    let _ = request.body(payload_str).send().await;
}

async fn start_worker_loop(
    mut receiver: mpsc::Receiver<TranscriptionJob>,
    manager: JobManager,
    models_dir: PathBuf,
    max_concurrent: usize,
) {
    let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrent));

    while let Some(job) = receiver.recv().await {
        let manager = manager.clone();
        let models_dir = models_dir.clone();
        let semaphore = semaphore.clone();

        tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();
            manager.update_job(&job.job_id, JobStatus::Processing).await;

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

            let final_status = match resolve_transcribe_options(options, None) {
                Ok(resolved) => {
                    match transcribe_batch_with_progress(&resolved.request, |_| {}).await {
                        Ok(segments) => JobStatus::Completed(segments),
                        Err(e) => JobStatus::Failed(e),
                    }
                }
                Err(e) => JobStatus::Failed(e),
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

    let job = TranscriptionJob {
        job_id: job_id.clone(),
        file_path,
        model_id: m_id,
        language,
        hotwords,
        webhook_url,
        webhook_secret,
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

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoResponse {
    pub platform: String,
    pub gpu_available: bool,
    pub models: Vec<String>,
    pub vad_installed: bool,
    pub punctuation_installed: bool,
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

    Ok(Json(InfoResponse {
        platform: std::env::consts::OS.to_string(),
        gpu_available,
        models: installed_models,
        vad_installed,
        punctuation_installed,
    }))
}

pub async fn run_server(
    host: &str,
    port: u16,
    api_key: &str,
    temp_dir: PathBuf,
    models_dir: PathBuf,
    max_concurrent: usize,
    max_queue_size: usize,
    max_upload_size_mb: usize,
    shutdown_rx: tokio::sync::oneshot::Receiver<()>,
) -> Result<(), String> {
    // Resource Cleanup: clean temp_dir on startup
    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    tokio::fs::create_dir_all(&temp_dir)
        .await
        .map_err(|e| e.to_string())?;

    let (tx, rx) = mpsc::channel(max_queue_size);
    let job_manager = JobManager::new(tx);
    let manager_clone = job_manager.clone();
    let models_dir_clone = models_dir.clone();

    tokio::spawn(async move {
        start_worker_loop(rx, manager_clone, models_dir_clone, max_concurrent).await;
    });

    let state = ServerState {
        job_manager,
        temp_dir,
        models_dir,
        start_time: std::time::Instant::now(),
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Create public routes
    let router = Router::new()
        .route("/health", get(handle_health))
        .route("/info", get(handle_info));

    // Create private/transcriptions routes
    let mut api_router = Router::new()
        .route("/v1/transcriptions", post(handle_transcribe))
        .route("/v1/transcriptions/jobs", get(handle_list_jobs))
        .route("/v1/transcriptions/{job_id}", get(handle_job_status))
        .layer(axum::extract::DefaultBodyLimit::max(
            max_upload_size_mb * 1024 * 1024,
        ));

    #[allow(deprecated)]
    if !api_key.is_empty() {
        api_router = api_router.route_layer(ValidateRequestHeaderLayer::bearer(api_key));
    }

    // Merge and apply CORS & state
    let router = router.merge(api_router).layer(cors).with_state(state);
    let addr = format!("{}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| e.to_string())?;

    log::info!("Starting HTTP API server on {}", addr);
    axum::serve(listener, router)
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
        job_manager
            .jobs
            .write()
            .await
            .insert("test-job-id".to_string(), JobStatus::Pending);

        let state = ServerState {
            job_manager,
            temp_dir,
            models_dir,
            start_time: std::time::Instant::now(),
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
