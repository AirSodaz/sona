use tauri::{AppHandle, State};

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn start_api_server(
    app: AppHandle,
    controller: State<'_, crate::app::server::ApiServerController>,
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
    crate::app::server::start_api_server(
        app,
        controller,
        host,
        port,
        api_key,
        max_concurrent,
        max_queue_size,
        max_upload_size_mb,
        job_ttl_minutes,
        max_streaming,
        ip_whitelist,
        gpu_acceleration,
    )
    .await
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiServerActiveJobsInfo {
    pub has_active: bool,
    pub processing: usize,
    pub pending: usize,
}

#[tauri::command]
pub async fn has_active_api_server_jobs(
    controller: State<'_, crate::app::server::ApiServerController>,
) -> Result<ApiServerActiveJobsInfo, String> {
    let (processing, pending) = controller.active_job_count().await;
    Ok(ApiServerActiveJobsInfo {
        has_active: processing > 0 || pending > 0,
        processing,
        pending,
    })
}

#[tauri::command]
pub async fn stop_api_server(
    controller: State<'_, crate::app::server::ApiServerController>,
    force: Option<bool>,
) -> Result<(), String> {
    crate::app::server::stop_api_server(controller, force.unwrap_or(false))
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_api_server_dashboard_snapshot(
    controller: State<'_, crate::app::server::ApiServerController>,
) -> Result<sona_api_server::ApiServerDashboardSnapshot, String> {
    controller.dashboard_snapshot().await
}
