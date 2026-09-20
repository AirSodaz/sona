use indexmap::IndexMap;
use std::borrow::Cow;
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
use sha2::{Digest, Sha256};
use sona_core::llm::requests::LlmConfig;
use sona_core::ports::asr::find_online_asr_provider;
use sona_core::transcription::transcript::TranscriptSegment;
use tokio::io::AsyncWriteExt;
use tower::ServiceExt;
use tower_http::services::ServeFile;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ApiError {
    #[serde(skip)]
    pub status: StatusCode,
    pub message: String,
}

impl ApiError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = serde_json::json!({
            "error": {
                "code": self.status.as_u16(),
                "message": self.message,
            }
        });
        (self.status, Json(body)).into_response()
    }
}

impl From<(StatusCode, String)> for ApiError {
    fn from((status, message): (StatusCode, String)) -> Self {
        Self::new(status, message)
    }
}

impl From<(StatusCode, &'static str)> for ApiError {
    fn from((status, message): (StatusCode, &'static str)) -> Self {
        Self::new(status, message)
    }
}

pub fn constant_time_eq_str(a: &str, b: &str) -> bool {
    let hash_a = Sha256::digest(a.as_bytes());
    let hash_b = Sha256::digest(b.as_bytes());
    let mut diff = 0u8;
    for (x, y) in hash_a.iter().zip(hash_b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

pub fn percent_decode_str(input: &str) -> Cow<'_, str> {
    if !input.contains('%') {
        return Cow::Borrowed(input);
    }

    let bytes = input.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(byte) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..=i + 2]).unwrap_or(""), 16)
        {
            decoded.push(byte);
            i += 3;
            continue;
        }
        decoded.push(bytes[i]);
        i += 1;
    }
    match String::from_utf8(decoded) {
        Ok(s) => Cow::Owned(s),
        Err(_) => Cow::Borrowed(input),
    }
}

pub fn extract_api_key_from_request<'a>(req: &'a Request) -> Option<Cow<'a, str>> {
    if let Some(auth_val) = req.headers().get(axum::http::header::AUTHORIZATION)
        && let Ok(auth_str) = auth_val.to_str()
    {
        let trimmed = auth_str.trim();
        if trimmed.len() >= 7
            && trimmed[..6].eq_ignore_ascii_case("bearer")
            && trimmed.as_bytes()[6] == b' '
        {
            return Some(Cow::Borrowed(trimmed[7..].trim()));
        }
    }
    // Restrict query-based API keys to endpoints that genuinely require it (e.g. HTML5 audio, streaming, or test paths)
    let path = req.uri().path();
    let allow_query_token = path.ends_with("/audio")
        || path.ends_with("/streaming")
        || path == "/test"
        || path == "/api/test";

    if allow_query_token && let Some(query) = req.uri().query() {
        for pair in query.split('&') {
            let mut parts = pair.splitn(2, '=');
            let key = parts.next().unwrap_or_default();
            let val = parts.next().unwrap_or_default();
            if key == "token" || key == "api_key" {
                return Some(percent_decode_str(val));
            }
        }
    }

    None
}

pub fn is_request_api_key_valid(req: &Request, configured_api_key: &str) -> bool {
    if configured_api_key.is_empty() {
        return false;
    }
    match extract_api_key_from_request(req) {
        Some(token) => constant_time_eq_str(&token, configured_api_key),
        None => false,
    }
}

pub fn validate_webhook_url(url: &str) -> Result<(), &'static str> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|_| "Invalid webhook_url: must be a valid http or https URL")?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err("Invalid webhook_url: must be a valid http or https URL");
    }
    let host = parsed
        .host_str()
        .ok_or("Invalid webhook_url: missing host")?;
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        if ip.is_unspecified() {
            return Err("Invalid webhook_url: unspecified IP addresses are not permitted");
        }
        match ip {
            std::net::IpAddr::V4(ipv4) => {
                if ipv4.is_link_local() {
                    return Err("Invalid webhook_url: link-local IP addresses are not permitted");
                }
                if ipv4.is_broadcast() {
                    return Err("Invalid webhook_url: broadcast IP addresses are not permitted");
                }
            }
            std::net::IpAddr::V6(ipv6) => {
                let segments = ipv6.segments();
                if (segments[0] & 0xffc0) == 0xfe80 {
                    return Err("Invalid webhook_url: link-local IP addresses are not permitted");
                }
            }
        }
    }
    Ok(())
}

pub async fn ip_whitelist_middleware(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<ServerState>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let ip = addr.ip().to_canonical();

    // 1. Enforce network perimeter (IP whitelist) first
    if !state.ip_whitelist.iter().any(|net| net.contains(&ip)) {
        log::warn!(
            "[ApiServer] Request from {} rejected: IP not in whitelist ({:?})",
            ip,
            state.ip_whitelist
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // 2. Enforce API key authentication if configured (Defense in Depth)
    if !state.api_key.is_empty() && !is_request_api_key_valid(&req, &state.api_key) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(next.run(req).await)
}

pub async fn api_key_auth_middleware(
    State(state): State<ServerState>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if state.api_key.is_empty() {
        return Ok(next.run(req).await);
    }
    if is_request_api_key_valid(&req, &state.api_key) {
        return Ok(next.run(req).await);
    }
    Err(StatusCode::UNAUTHORIZED)
}

pub async fn handle_health(State(state): State<ServerState>) -> Json<HealthResponse> {
    Json(build_health_response(&state).await)
}

pub async fn handle_info(State(state): State<ServerState>) -> Result<Json<InfoResponse>, ApiError> {
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
) -> Result<Json<JobStatus>, ApiError> {
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
    pub full: Option<bool>,
}

pub async fn handle_list_jobs(
    State(state): State<ServerState>,
    Query(query): Query<ListJobsQuery>,
) -> Json<IndexMap<String, JobStatus>> {
    let mut ordered = state.job_manager.list_jobs_ordered().await;
    if let Some(filter_status) = &query.status {
        let filter_lower = filter_status.to_lowercase();
        ordered.retain(|(_, s)| match s {
            JobStatus::Pending => filter_lower == "pending",
            JobStatus::Processing => filter_lower == "processing",
            JobStatus::Completed(_) => filter_lower == "completed",
            JobStatus::Failed(_) => filter_lower == "failed",
        });
    }
    if let Some(offset) = query.offset {
        if offset < ordered.len() {
            ordered.drain(..offset);
        } else {
            ordered.clear();
        }
    }
    if let Some(limit) = query.limit {
        ordered.truncate(limit);
    }
    let include_full = query.full.unwrap_or(false);
    let map: IndexMap<String, JobStatus> = ordered
        .into_iter()
        .map(|(k, v)| {
            let status = match v {
                JobStatus::Completed(_) if !include_full => JobStatus::Completed(Vec::new()),
                other => other,
            };
            (k, status)
        })
        .collect();
    Json(map)
}

pub async fn handle_transcribe(
    State(state): State<ServerState>,
    multipart: Multipart,
) -> Result<Json<serde_json::Value>, ApiError> {
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
) -> Result<Json<serde_json::Value>, ApiError> {
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
            if temp_file_path.is_some() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "Multiple file fields are not allowed".to_string(),
                )
                    .into());
            }
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
            file.flush()
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            drop(file);

            if !state.media_validator.is_valid_media_file(&file_path).await {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "Unsupported file type or corrupted file".to_string(),
                )
                    .into());
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
    if m_id.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "model_id cannot be empty".to_string(),
        )
            .into());
    }
    let is_online = find_online_asr_provider(&m_id).is_some();
    let is_preset = sona_core::models::preset_models::preset_models()
        .iter()
        .any(|m| m.id == m_id || m.group_id.as_deref() == Some(&m_id));
    if !is_online && !is_preset {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("Unknown model_id: '{}'", m_id),
        )
            .into());
    }
    if let Some(url) = &webhook_url
        && !url.is_empty()
    {
        validate_webhook_url(url)
            .map_err(|err_msg| (StatusCode::BAD_REQUEST, err_msg.to_string()))?;
    }

    let mut engine = "Local".to_string();
    let mut online_provider_id = None;
    let mut online_provider_config = None;

    if let Some(provider) = find_online_asr_provider(&m_id) {
        let configs = state.online_asr_config.read().await;
        let is_configured = configs
            .get(&provider.id)
            .and_then(|c| c.get("apiKey"))
            .and_then(serde_json::Value::as_str)
            .is_some_and(|k| !k.is_empty());
        if !is_configured {
            return Err((
                StatusCode::BAD_REQUEST,
                format!(
                    "Online ASR provider '{}' is not configured with an API key",
                    provider.id
                ),
            )
                .into());
        }
        engine = "Online".to_string();
        online_provider_id = Some(provider.id.clone());
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
) -> Result<Response, ApiError> {
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
        )
            .into());
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
) -> Result<StatusCode, ApiError> {
    let file_path_opt = state
        .job_manager
        .remove_job(&job_id)
        .await
        .ok_or((StatusCode::NOT_FOUND, "Job not found".to_string()))?;

    if let Some(file_path) = file_path_opt
        && tokio::fs::remove_file(&file_path).await.is_err()
    {
        tokio::spawn(async move {
            for i in 0..5 {
                tokio::time::sleep(std::time::Duration::from_millis(200 * (i + 1))).await;
                if tokio::fs::remove_file(&file_path).await.is_ok() {
                    break;
                }
            }
        });
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
) -> Result<Response, ApiError> {
    let status = state
        .job_manager
        .get_job(&job_id)
        .await
        .ok_or((StatusCode::NOT_FOUND, "Job not found".to_string()))?;

    let segments = match status {
        JobStatus::Completed(segments) => segments,
        JobStatus::Pending | JobStatus::Processing => {
            return Err((StatusCode::CONFLICT, "Job is still processing".to_string()).into());
        }
        JobStatus::Failed(error) => {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("Job failed: {}", error),
            )
                .into());
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

    let extension = match format {
        sona_core::export::ExportFormat::Json => "json",
        sona_core::export::ExportFormat::Srt => "srt",
        sona_core::export::ExportFormat::Vtt => "vtt",
        sona_core::export::ExportFormat::Txt => "txt",
        sona_core::export::ExportFormat::Md => "md",
    };
    let filename = format!("{}.{}", job_id, extension);
    let disposition = format!("attachment; filename=\"{}\"", filename);

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, content_type)
        .header(axum::http::header::CONTENT_DISPOSITION, disposition)
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
) -> Result<Json<serde_json::Value>, ApiError> {
    if payload.segments.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "segments cannot be empty".to_string(),
        )
            .into());
    }
    if payload.segments.len() > 10_000 {
        return Err((
            StatusCode::BAD_REQUEST,
            "Too many segments (max 10000)".to_string(),
        )
            .into());
    }
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
) -> Result<Json<serde_json::Value>, ApiError> {
    if payload.segments.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "segments cannot be empty".to_string(),
        )
            .into());
    }
    if payload.segments.len() > 10_000 {
        return Err((
            StatusCode::BAD_REQUEST,
            "Too many segments (max 10000)".to_string(),
        )
            .into());
    }
    let target_lang = payload.target_language.trim();
    if target_lang.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "target_language cannot be empty".to_string(),
        )
            .into());
    }
    let translated = state
        .platform
        .translate_segments(payload.segments, target_lang.to_string(), payload.config)
        .await
        .map_err(|e| (StatusCode::SERVICE_UNAVAILABLE, e.to_string()))?;

    Ok(Json(serde_json::json!({ "segments": translated })))
}
