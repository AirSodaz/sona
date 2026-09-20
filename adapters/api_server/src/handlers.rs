use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use crate::info::{HealthResponse, InfoResponse, build_health_response, build_info_response};
use crate::jobs::{JobStatus, TranscriptionJob};
use crate::state::ServerState;
use axum::{
    Json,
    extract::{ConnectInfo, Multipart, Path, Query, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use futures_util::stream::StreamExt;
use sona_core::llm::requests::LlmConfig;
use sona_core::ports::asr::find_online_asr_provider;
use sona_core::transcription::transcript::TranscriptSegment;
use tokio::io::AsyncWriteExt;
use tower::ServiceExt;
use tower_http::services::ServeFile;

pub fn extract_api_key_from_request(req: &Request) -> Option<&str> {
    if let Some(auth_val) = req.headers().get(axum::http::header::AUTHORIZATION)
        && let Ok(auth_str) = auth_val.to_str()
        && let Some(token) = auth_str.strip_prefix("Bearer ")
    {
        return Some(token);
    }

    if let Some(query) = req.uri().query() {
        for pair in query.split('&') {
            let mut parts = pair.splitn(2, '=');
            let key = parts.next().unwrap_or_default();
            let val = parts.next().unwrap_or_default();
            if key == "token" || key == "api_key" {
                return Some(val);
            }
        }
    }

    None
}

pub async fn ip_whitelist_middleware(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<ServerState>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let ip = addr.ip().to_canonical();

    // If the request presents a valid API key, allow it through
    if !state.api_key.is_empty()
        && extract_api_key_from_request(&req) == Some(state.api_key.as_str())
    {
        return Ok(next.run(req).await);
    }

    if state.ip_whitelist.iter().any(|net| net.contains(&ip)) {
        Ok(next.run(req).await)
    } else {
        log::warn!(
            "[ApiServer] Request from {} rejected: IP not in whitelist ({:?})",
            ip,
            state.ip_whitelist
        );
        Err(StatusCode::FORBIDDEN)
    }
}
pub async fn api_key_auth_middleware(
    State(state): State<ServerState>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if state.api_key.is_empty() {
        return Ok(next.run(req).await);
    }
    if extract_api_key_from_request(&req) == Some(state.api_key.as_str()) {
        return Ok(next.run(req).await);
    }
    Err(StatusCode::UNAUTHORIZED)
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

#[derive(Default, serde::Deserialize)]
pub struct ListJobsQuery {
    pub status: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

pub async fn handle_list_jobs(
    State(state): State<ServerState>,
    Query(query): Query<ListJobsQuery>,
) -> Json<HashMap<String, JobStatus>> {
    let mut jobs = state.job_manager.list_jobs().await;
    if let Some(filter_status) = &query.status {
        let filter_lower = filter_status.to_lowercase();
        jobs.retain(|_, s| match s {
            JobStatus::Pending => filter_lower == "pending",
            JobStatus::Processing => filter_lower == "processing",
            JobStatus::Completed(_) => filter_lower == "completed",
            JobStatus::Failed(_) => filter_lower == "failed",
        });
    }
    if let Some(offset) = query.offset {
        let mut keys: Vec<_> = jobs.keys().cloned().collect();
        keys.sort();
        for key in keys.into_iter().take(offset) {
            jobs.remove(&key);
        }
    }
    if let Some(limit) = query.limit {
        let mut keys: Vec<_> = jobs.keys().cloned().collect();
        keys.sort();
        if keys.len() > limit {
            for key in keys.into_iter().skip(limit) {
                jobs.remove(&key);
            }
        }
    }
    Json(jobs)
}

pub async fn handle_transcribe(
    State(state): State<ServerState>,
    multipart: Multipart,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let mut temp_file_path = None;
    let result = handle_transcribe_inner(&state, multipart, &mut temp_file_path).await;
    if result.is_err()
        && let Some(path) = &temp_file_path
    {
        let _ = tokio::fs::remove_file(path).await;
    }
    result
}

async fn handle_transcribe_inner(
    state: &ServerState,
    mut multipart: Multipart,
    temp_file_path: &mut Option<std::path::PathBuf>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let job_id = uuid::Uuid::new_v4().to_string();
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
            let extension = field
                .file_name()
                .and_then(|name| std::path::Path::new(name).extension())
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.to_lowercase())
                .filter(|ext| !ext.is_empty() && ext.chars().all(|c| c.is_ascii_alphanumeric()))
                .unwrap_or_else(|| "wav".to_string());
            let file_path = state.temp_dir.join(format!("{}.{}", job_id, extension));
            let mut file = tokio::fs::File::create(&file_path)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            *temp_file_path = Some(file_path.clone());

            while let Some(chunk) = field.next().await {
                let data = chunk.map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
                file.write_all(&data)
                    .await
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            }

            if !state.media_validator.is_valid_media_file(&file_path).await {
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

    let file_path = temp_file_path
        .clone()
        .ok_or((StatusCode::BAD_REQUEST, "Missing file".to_string()))?;
    let m_id = model_id.ok_or((StatusCode::BAD_REQUEST, "Missing model_id".to_string()))?;

    if let Some(url) = &webhook_url
        && !url.is_empty()
    {
        match reqwest::Url::parse(url) {
            Ok(parsed) if parsed.scheme() == "http" || parsed.scheme() == "https" => {}
            _ => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "Invalid webhook_url: must be a valid http or https URL".to_string(),
                ));
            }
        }
    }

    let mut engine = "Local".to_string();
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

pub(crate) async fn handle_job_audio(
    State(state): State<ServerState>,
    Path(job_id): Path<String>,
    req: Request,
) -> Result<Response, (StatusCode, String)> {
    let _ = state
        .job_manager
        .get_job(&job_id)
        .await
        .ok_or((StatusCode::NOT_FOUND, "Job not found".to_string()))?;

    let file_path = state.job_manager.get_job_file_path(&job_id).await.ok_or((
        StatusCode::NOT_FOUND,
        "Audio file not found or already cleaned up".to_string(),
    ))?;

    if !file_path.exists() {
        return Err((
            StatusCode::NOT_FOUND,
            "Audio file not found or already cleaned up".to_string(),
        ));
    }

    let res = ServeFile::new(file_path)
        .oneshot(req)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(res.into_response())
}

pub(crate) async fn handle_delete_job(
    State(state): State<ServerState>,
    Path(job_id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let file_path_opt = state
        .job_manager
        .remove_job(&job_id)
        .await
        .ok_or((StatusCode::NOT_FOUND, "Job not found".to_string()))?;

    if let Some(file_path) = file_path_opt {
        let _ = tokio::fs::remove_file(file_path).await;
    }

    Ok(StatusCode::NO_CONTENT)
}

#[derive(serde::Deserialize)]
pub struct ExportQuery {
    pub format: Option<sona_core::export::ExportFormat>,
    pub mode: Option<sona_core::export::ExportMode>,
}

pub(crate) async fn handle_export_job(
    State(state): State<ServerState>,
    Path(job_id): Path<String>,
    Query(query): Query<ExportQuery>,
) -> Result<Response, (StatusCode, String)> {
    let status = state
        .job_manager
        .get_job(&job_id)
        .await
        .ok_or((StatusCode::NOT_FOUND, "Job not found".to_string()))?;

    let segments = match status {
        JobStatus::Completed(segments) => segments,
        JobStatus::Pending | JobStatus::Processing => {
            return Err((StatusCode::CONFLICT, "Job is still processing".to_string()));
        }
        JobStatus::Failed(error) => {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("Job failed: {}", error),
            ));
        }
    };

    let format = query.format.unwrap_or(sona_core::export::ExportFormat::Srt);
    let mode = query
        .mode
        .unwrap_or(sona_core::export::ExportMode::Original);
    let content = sona_core::export::export_segments_with_mode(&segments, format, mode)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let content_type = match format {
        sona_core::export::ExportFormat::Json => "application/json; charset=utf-8",
        sona_core::export::ExportFormat::Srt => "application/x-subrip; charset=utf-8",
        sona_core::export::ExportFormat::Vtt => "text/vtt; charset=utf-8",
        sona_core::export::ExportFormat::Txt => "text/plain; charset=utf-8",
        sona_core::export::ExportFormat::Md => "text/markdown; charset=utf-8",
    };

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, content_type)
        .body(axum::body::Body::from(content))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(response)
}

#[derive(serde::Deserialize)]
pub struct LlmPolishRequest {
    pub segments: Vec<TranscriptSegment>,
    pub config: Option<LlmConfig>,
}

#[derive(serde::Deserialize)]
pub struct LlmTranslateRequest {
    pub segments: Vec<TranscriptSegment>,
    pub target_language: String,
    pub config: Option<LlmConfig>,
}

pub(crate) async fn handle_llm_polish(
    State(state): State<ServerState>,
    Json(payload): Json<LlmPolishRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let polished = state
        .platform
        .polish_segments(payload.segments, payload.config)
        .await
        .map_err(|e| (StatusCode::SERVICE_UNAVAILABLE, e.to_string()))?;

    Ok(Json(serde_json::json!({ "segments": polished })))
}

pub(crate) async fn handle_llm_translate(
    State(state): State<ServerState>,
    Json(payload): Json<LlmTranslateRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let translated = state
        .platform
        .translate_segments(payload.segments, payload.target_language, payload.config)
        .await
        .map_err(|e| (StatusCode::SERVICE_UNAVAILABLE, e.to_string()))?;

    Ok(Json(serde_json::json!({ "segments": translated })))
}
