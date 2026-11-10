use tauri::State;

use crate::services::DesktopServices;

#[tauri::command]
pub async fn cancel_download(
    services: State<'_, DesktopServices>,
    id: String,
) -> Result<bool, String> {
    Ok(services.downloads.notify_download(&id).await)
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
pub async fn download_preset_model(
    services: State<'_, DesktopServices>,
    model_id: String,
    download_id: String,
    mirror: Option<String>,
) -> Result<String, String> {
    crate::platform::model_downloads::download_preset_model(
        services.emitter.clone(),
        &services.active_models_dir(),
        &services.downloads,
        model_id,
        download_id,
        mirror,
    )
    .await
}

#[tauri::command]
pub async fn delete_preset_model(
    services: State<'_, DesktopServices>,
    model_id: String,
) -> Result<(), String> {
    crate::platform::model_downloads::delete_preset_model(&services.active_models_dir(), &model_id)
        .await
}

#[tauri::command]
pub async fn check_interrupted_download(
    services: State<'_, DesktopServices>,
    model_id: String,
) -> Result<Option<sona_core::models::downloads::InterruptedDownloadInfo>, String> {
    let models_dir = services.active_models_dir();
    tokio::task::spawn_blocking(move || {
        sona_core::models::downloads::inspect_interrupted_download(&models_dir, &model_id)
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clean_interrupted_download(
    services: State<'_, DesktopServices>,
    model_id: String,
) -> Result<bool, String> {
    let models_dir = services.active_models_dir();
    tokio::task::spawn_blocking(move || {
        sona_core::models::downloads::clean_interrupted_download(&models_dir, &model_id)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_cuda_addon_status(
    services: State<'_, DesktopServices>,
) -> Result<sona_core::runtime::cuda_addon::CudaAddonInspection, String> {
    crate::platform::model_downloads::get_cuda_addon_status(&services.active_data_dir())
}

#[tauri::command]
pub async fn activate_cuda_addon(
    services: State<'_, DesktopServices>,
) -> Result<sona_core::runtime::cuda_addon::CudaAddonInspection, String> {
    crate::platform::model_downloads::activate_cuda_addon(&services.active_data_dir()).await
}

#[tauri::command]
pub async fn download_cuda_addon(
    services: State<'_, DesktopServices>,
    download_id: String,
    mirror: Option<String>,
    version: Option<String>,
    custom_url: Option<String>,
    expected_sha256: Option<String>,
) -> Result<sona_core::runtime::cuda_addon::CudaAddonInspection, String> {
    crate::platform::model_downloads::download_and_install_cuda_addon(
        services.emitter.clone(),
        &services.active_data_dir(),
        &services.downloads,
        download_id,
        mirror,
        version,
        custom_url,
        expected_sha256,
    )
    .await
}
