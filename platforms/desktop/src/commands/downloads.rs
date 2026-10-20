use tauri::State;

use crate::services::DesktopServices;

#[tauri::command]
pub async fn cancel_download(
    services: State<'_, DesktopServices>,
    id: String,
) -> Result<bool, String> {
    if services.downloads.notify_download(&id).await {
        Ok(true)
    } else {
        Err(format!("No active download found for id: {id}"))
    }
}

#[tauri::command]
pub async fn has_active_downloads(services: State<'_, DesktopServices>) -> Result<bool, String> {
    Ok(services.downloads.has_active_downloads().await)
}

#[tauri::command]
pub async fn download_file(
    services: State<'_, DesktopServices>,
    url: String,
    output_path: String,
    id: String,
    expected_sha256: Option<String>,
) -> Result<(), String> {
    crate::platform::model_downloads::download_file(
        services.emitter.clone(),
        &services.downloads,
        url,
        output_path,
        id,
        expected_sha256,
    )
    .await
}

#[tauri::command]
pub async fn download_preset_model<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    services: State<'_, DesktopServices>,
    model_id: String,
    download_id: String,
    mirror: Option<String>,
) -> Result<String, String> {
    crate::platform::model_downloads::download_preset_model(
        app,
        &services.downloads,
        model_id,
        download_id,
        mirror,
    )
    .await
}

#[tauri::command]
pub async fn delete_preset_model<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    model_id: String,
) -> Result<(), String> {
    crate::platform::model_downloads::delete_preset_model(&app, &model_id).await
}

#[tauri::command]
pub async fn check_interrupted_download<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    model_id: String,
) -> Result<Option<sona_core::models::downloads::InterruptedDownloadInfo>, String> {
    let models_dir = crate::platform::storage_location::resolve_active_models_dir_for_app(&app)?;
    Ok(sona_core::models::downloads::inspect_interrupted_download(
        &models_dir,
        &model_id,
    ))
}

#[tauri::command]
pub async fn clean_interrupted_download<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    model_id: String,
) -> Result<bool, String> {
    let models_dir = crate::platform::storage_location::resolve_active_models_dir_for_app(&app)?;
    sona_core::models::downloads::clean_interrupted_download(&models_dir, &model_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_cuda_addon_status<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<sona_core::runtime::cuda_addon::CudaAddonInspection, String> {
    crate::platform::model_downloads::get_cuda_addon_status(&app)
}

#[tauri::command]
pub async fn activate_cuda_addon<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<sona_core::runtime::cuda_addon::CudaAddonInspection, String> {
    crate::platform::model_downloads::activate_cuda_addon(app).await
}

#[tauri::command]
pub async fn download_cuda_addon<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    services: State<'_, DesktopServices>,
    download_id: String,
    mirror: Option<String>,
    version: Option<String>,
    custom_url: Option<String>,
    expected_sha256: Option<String>,
) -> Result<sona_core::runtime::cuda_addon::CudaAddonInspection, String> {
    crate::platform::model_downloads::download_and_install_cuda_addon(
        app,
        &services.downloads,
        download_id,
        mirror,
        version,
        custom_url,
        expected_sha256,
    )
    .await
}
