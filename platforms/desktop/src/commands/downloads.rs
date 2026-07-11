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
