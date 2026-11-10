use crate::services::DesktopServices;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn storage_get_usage_snapshot(
    services: State<'_, DesktopServices>,
) -> Result<crate::platform::storage_usage::StorageUsageSnapshot, String> {
    crate::platform::storage_usage::get_usage_snapshot_with_sqlite_and_models_dir(
        &services.sqlite,
        services.active_models_dir(),
    )
    .await
}

#[tauri::command]
pub async fn storage_clear_webview_browsing_data(
    app: AppHandle,
) -> Result<crate::platform::storage_usage::WebviewBrowsingDataClearResult, String> {
    crate::platform::storage_usage::clear_webview_browsing_data(&app).await
}

#[tauri::command]
pub async fn storage_get_directories(
    services: State<'_, DesktopServices>,
) -> Result<crate::platform::storage_location::StorageDirectoriesInfo, String> {
    Ok(services.storage_directories_info())
}

#[tauri::command]
pub async fn storage_migrate_data_directory(
    app: AppHandle,
    target_dir: String,
    copy_existing: bool,
) -> Result<crate::platform::storage_location::StorageDirectoriesInfo, String> {
    crate::platform::storage_location::migrate_data_directory(&app, target_dir, copy_existing).await
}

#[tauri::command]
pub async fn storage_reset_data_directory(
    app: AppHandle,
) -> Result<crate::platform::storage_location::StorageDirectoriesInfo, String> {
    crate::platform::storage_location::reset_data_directory(&app).await
}

#[tauri::command]
pub async fn storage_check_can_migrate(app: AppHandle) -> Result<(), String> {
    crate::platform::storage_location::check_storage_can_migrate(&app).await
}

#[tauri::command]
pub async fn storage_set_models_directory(
    app: AppHandle,
    target_dir: String,
    move_existing: bool,
) -> Result<crate::platform::storage_location::StorageDirectoriesInfo, String> {
    crate::platform::storage_location::set_models_directory(&app, target_dir, move_existing).await
}

#[tauri::command]
pub async fn storage_reset_models_directory(
    app: AppHandle,
) -> Result<crate::platform::storage_location::StorageDirectoriesInfo, String> {
    crate::platform::storage_location::reset_models_directory(&app).await
}

#[tauri::command]
pub async fn storage_open_path(app: AppHandle, path: String) -> Result<(), String> {
    crate::platform::storage_location::open_storage_path(&app, path)
}
