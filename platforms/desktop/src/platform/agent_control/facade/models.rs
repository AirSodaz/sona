use crate::services::DesktopServices;

use super::types::DownloadPresetModelRequest;

pub async fn get_model_catalog(
    services: &DesktopServices,
    _app_handle: Option<&tauri::AppHandle>,
) -> Result<serde_json::Value, String> {
    let models_dir = services.active_models_dir();
    let snapshot = tokio::task::spawn_blocking(move || {
        let _ = sona_runtime_fs::ensure_directory_exists(&models_dir);
        sona_runtime_fs::build_model_catalog_snapshot(&models_dir)
    })
    .await
    .map_err(|e| e.to_string())?;
    serde_json::to_value(&snapshot).map_err(|e| e.to_string())
}

pub async fn download_preset_model(
    services: &DesktopServices,
    _app_handle: Option<&tauri::AppHandle>,
    req: DownloadPresetModelRequest,
) -> Result<String, String> {
    let download_id = req
        .download_id
        .unwrap_or_else(|| format!("mcp-{}", uuid::Uuid::new_v4()));
    let emitter = services.emitter.clone();
    let downloads_service = services.downloads.clone();
    let dl_id_clone = download_id.clone();
    let model_id = req.model_id;
    let mirror = req.mirror;

    let models_dir = services.active_models_dir();
    sona_core::models::downloads::resolve_model_download(&model_id, &models_dir)
        .map_err(|e| e.to_string())?;
    let notify = std::sync::Arc::new(tokio::sync::Notify::new());
    let _download_guard = services
        .downloads
        .register_download(download_id.clone(), notify);

    let models_dir_clone = models_dir.clone();
    tokio::spawn(async move {
        let _guard = _download_guard;
        if let Err(e) = crate::platform::model_downloads::download_preset_model(
            emitter,
            &models_dir_clone,
            &downloads_service,
            model_id,
            dl_id_clone,
            mirror,
        )
        .await
        {
            log::error!("[AgentControl] Background model download failed: {e}");
        }
    });

    Ok(download_id)
}

pub async fn cancel_download(
    services: &DesktopServices,
    download_id: String,
) -> Result<bool, String> {
    Ok(services.downloads.notify_download(&download_id).await)
}

pub async fn delete_preset_model(
    services: &DesktopServices,
    model_id: String,
) -> Result<bool, String> {
    crate::platform::model_downloads::delete_preset_model(
        &services.active_models_dir(),
        &model_id,
    )
    .await?;
    Ok(true)
}
