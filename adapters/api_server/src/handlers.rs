use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    Json,
    extract::{ConnectInfo, Multipart, Path, Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use futures_util::stream::StreamExt;
use ipnet::IpNet;
use sona_core::ports::asr::find_online_asr_provider;
use tokio::io::AsyncWriteExt;

use crate::info::{HealthResponse, InfoResponse, build_health_response, build_info_response};
use crate::jobs::{JobStatus, TranscriptionJob};
use crate::state::ServerState;

pub(crate) async fn ip_whitelist_middleware(
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
