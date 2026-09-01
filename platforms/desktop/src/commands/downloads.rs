use crate::platform::model_downloads::DownloadState;

#[tauri::command]
pub async fn cancel_download(
    state: tauri::State<'_, DownloadState>,
    id: String,
) -> Result<(), String> {
    crate::platform::model_downloads::cancel_download(state, id).await
}

#[tauri::command]
pub async fn has_active_downloads(state: tauri::State<'_, DownloadState>) -> Result<bool, String> {
    crate::platform::model_downloads::has_active_downloads(state).await
}

#[tauri::command]
pub async fn download_file<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, DownloadState>,
    url: String,
    output_path: String,
    id: String,
    expected_sha256: Option<String>,
) -> Result<(), String> {
    crate::platform::model_downloads::download_file(
        app,
        state,
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
    state: tauri::State<'_, DownloadState>,
    model_id: String,
    download_id: String,
    mirror: Option<String>,
) -> Result<String, String> {
    crate::platform::model_downloads::download_preset_model(
        app,
        state,
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
    state: tauri::State<'_, DownloadState>,
    download_id: String,
    mirror: Option<String>,
    version: Option<String>,
    custom_url: Option<String>,
    expected_sha256: Option<String>,
) -> Result<sona_core::runtime::cuda_addon::CudaAddonInspection, String> {
    crate::platform::model_downloads::download_and_install_cuda_addon(
        app,
        state,
        download_id,
        mirror,
        version,
        custom_url,
        expected_sha256,
    )
    .await
}
