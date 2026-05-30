use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use futures_util::stream::StreamExt;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, RwLock};
use tower_http::{
    cors::{Any, CorsLayer},
    validate_request::ValidateRequestHeaderLayer,
};
use hmac::{Hmac, Mac, KeyInit};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

use crate::cli::{resolve_transcribe_options, TranscribeCliOptions};
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
}

async fn send_webhook(job: &TranscriptionJob, status: &JobStatus) {
    let Some(webhook_url) = &job.webhook_url else { return; };
    if webhook_url.is_empty() { return; }

    let mut payload = serde_json::Map::new();
    payload.insert("job_id".to_string(), serde_json::Value::String(job.job_id.clone()));
    
    match status {
        JobStatus::Completed(segments) => {
            payload.insert("status".to_string(), serde_json::Value::String("Completed".to_string()));
            payload.insert("segments".to_string(), serde_json::to_value(segments).unwrap_or_default());
        }
        JobStatus::Failed(error) => {
            payload.insert("status".to_string(), serde_json::Value::String("Failed".to_string()));
            payload.insert("error".to_string(), serde_json::Value::String(error.clone()));
        }
        _ => return, // Only send on completion or failure
    }

    let payload_str = serde_json::to_string(&payload).unwrap_or_default();
    
    let client = reqwest::Client::new();
    let mut request = client.post(webhook_url).header("Content-Type", "application/json");

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
) {
    while let Some(job) = receiver.recv().await {
        manager
            .update_job(&job.job_id, JobStatus::Processing)
            .await;

        let options = TranscribeCliOptions {
            input: job.file_path.clone(),
            output: None,
            format: None,
            language: if job.language == "auto" { None } else { Some(job.language.clone()) },
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
    }
}

#[derive(Clone)]
pub struct ServerState {
    pub job_manager: JobManager,
    pub temp_dir: PathBuf,
    pub models_dir: PathBuf,
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
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

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

pub async fn run_server(
    host: &str,
    port: u16,
    api_key: &str,
    temp_dir: PathBuf,
    models_dir: PathBuf,
    shutdown_rx: tokio::sync::oneshot::Receiver<()>,
) -> Result<(), String> {
    // Resource Cleanup: clean temp_dir on startup
    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    tokio::fs::create_dir_all(&temp_dir)
        .await
        .map_err(|e| e.to_string())?;

    let (tx, rx) = mpsc::channel(100);
    let job_manager = JobManager::new(tx);
    let manager_clone = job_manager.clone();
    let models_dir_clone = models_dir.clone();

    tokio::spawn(async move {
        start_worker_loop(rx, manager_clone, models_dir_clone).await;
    });

    let state = ServerState {
        job_manager,
        temp_dir,
        models_dir,
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let mut router = Router::new()
        .route("/v1/transcriptions", post(handle_transcribe))
        .route("/v1/transcriptions/:job_id", get(handle_job_status))
        .layer(cors);

    #[allow(deprecated)]
    if !api_key.is_empty() {
        router = router.route_layer(ValidateRequestHeaderLayer::bearer(api_key));
    }

    let router = router.with_state(state);
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
