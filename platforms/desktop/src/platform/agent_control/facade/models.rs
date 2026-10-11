use crate::services::DesktopServices;

use super::types::DownloadPresetModelRequest;

pub async fn get_model_catalog(
    services: &DesktopServices,
    app_handle: Option<&tauri::AppHandle>,
) -> Result<serde_json::Value, String> {
    if let Some(app) = app_handle {
        let snapshot =
            crate::platform::models::preset::get_model_catalog_snapshot_for_app(app).await?;
        serde_json::to_value(&snapshot).map_err(|e| e.to_string())
    } else {
        let models_dir = services
            .sqlite
            .current_context()
            .map_err(|e| e.to_string())?
            .app_data_dir()
            .join("models");
        let snapshot = tokio::task::spawn_blocking(move || {
            let _ = sona_runtime_fs::ensure_directory_exists(&models_dir);
            sona_runtime_fs::build_model_catalog_snapshot(&models_dir)
        })
        .await
        .map_err(|e| e.to_string())?;
        serde_json::to_value(&snapshot).map_err(|e| e.to_string())
    }
}

pub async fn download_preset_model(
    services: &DesktopServices,
    app_handle: Option<&tauri::AppHandle>,
    req: DownloadPresetModelRequest,
) -> Result<String, String> {
    let app = app_handle.ok_or_else(|| {
        "APP_HANDLE_UNAVAILABLE: Desktop UI handle is required to initiate downloads".to_string()
    })?;
    let download_id = req
        .download_id
        .unwrap_or_else(|| format!("mcp-{}", uuid::Uuid::new_v4()));
    let app_clone = app.clone();
    let downloads_service = services.downloads.clone();
    let dl_id_clone = download_id.clone();
    let model_id = req.model_id;
    let mirror = req.mirror;

    tauri::async_runtime::spawn(async move {
        if let Err(e) = crate::platform::model_downloads::download_preset_model(
            app_clone,
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
    services.downloads.notify_download(&download_id).await;
    Ok(true)
}

pub async fn delete_preset_model(
    app_handle: Option<&tauri::AppHandle>,
    model_id: String,
) -> Result<bool, String> {
    let app = app_handle.ok_or_else(|| {
        "APP_HANDLE_UNAVAILABLE: Desktop UI handle is required to delete models".to_string()
    })?;
    crate::platform::model_downloads::delete_preset_model(app, &model_id).await?;
    Ok(true)
}
