use tauri::{AppHandle, Runtime};

#[tauri::command]
pub async fn storage_get_usage_snapshot<R: Runtime>(
    app: AppHandle<R>,
) -> Result<crate::platform::storage_usage::StorageUsageSnapshot, String> {
    crate::platform::storage_usage::get_usage_snapshot(&app).await
}

#[tauri::command]
pub async fn storage_clear_webview_browsing_data<R: Runtime>(
    app: AppHandle<R>,
) -> Result<crate::platform::storage_usage::WebviewBrowsingDataClearResult, String> {
    crate::platform::storage_usage::clear_webview_browsing_data(&app).await
}

#[tauri::command]
pub async fn storage_get_directories<R: Runtime>(
    app: AppHandle<R>,
) -> Result<crate::platform::storage_location::StorageDirectoriesInfo, String> {
    crate::platform::storage_location::get_storage_directories_info(&app)
}

#[tauri::command]
pub async fn storage_migrate_data_directory<R: Runtime>(
    app: AppHandle<R>,
    target_dir: String,
    copy_existing: bool,
) -> Result<crate::platform::storage_location::StorageDirectoriesInfo, String> {
    crate::platform::storage_location::migrate_data_directory(&app, target_dir, copy_existing)
}

#[tauri::command]
pub async fn storage_reset_data_directory<R: Runtime>(
    app: AppHandle<R>,
) -> Result<crate::platform::storage_location::StorageDirectoriesInfo, String> {
    crate::platform::storage_location::reset_data_directory(&app)
}

#[tauri::command]
pub async fn storage_set_models_directory<R: Runtime>(
    app: AppHandle<R>,
    target_dir: String,
    move_existing: bool,
) -> Result<crate::platform::storage_location::StorageDirectoriesInfo, String> {
    crate::platform::storage_location::set_models_directory(&app, target_dir, move_existing)
}

#[tauri::command]
pub async fn storage_reset_models_directory<R: Runtime>(
    app: AppHandle<R>,
) -> Result<crate::platform::storage_location::StorageDirectoriesInfo, String> {
    crate::platform::storage_location::reset_models_directory(&app)
}

#[tauri::command]
pub async fn storage_open_path<R: Runtime>(app: AppHandle<R>, path: String) -> Result<(), String> {
    crate::platform::storage_location::open_storage_path(&app, path)
}
