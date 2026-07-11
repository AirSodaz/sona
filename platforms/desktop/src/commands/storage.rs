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
