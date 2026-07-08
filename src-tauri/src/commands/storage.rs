use tauri::{AppHandle, Manager, Runtime};

use crate::platform::paths::{PathKind, PathProvider, TauriPathProvider};
use sona_sqlite::storage_usage::{
    StorageUsageSnapshot, WebviewBrowsingDataClearResult, build_webview_clear_result,
    collect_storage_usage_snapshot, observable_webview_cache_bytes,
};

#[tauri::command]
pub async fn storage_get_usage_snapshot<R: Runtime>(
    app: AppHandle<R>,
) -> Result<StorageUsageSnapshot, String> {
    let app_local_data_dir =
        TauriPathProvider::from_app(&app).resolve_path(PathKind::AppLocalData)?;
    let db = crate::platform::database::sqlite_database(&app);

    tauri::async_runtime::spawn_blocking(move || {
        collect_storage_usage_snapshot(&app_local_data_dir, db.as_ref())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn storage_clear_webview_browsing_data<R: Runtime>(
    app: AppHandle<R>,
) -> Result<WebviewBrowsingDataClearResult, String> {
    let app_local_data_dir =
        TauriPathProvider::from_app(&app).resolve_path(PathKind::AppLocalData)?;
    let before_bytes = observable_webview_cache_bytes(&app_local_data_dir)?;
    let Some(main_window) = app.get_webview_window("main") else {
        return Err("Main WebView window is not available.".to_string());
    };

    main_window
        .clear_all_browsing_data()
        .map_err(|error| error.to_string())?;

    let after_bytes = observable_webview_cache_bytes(&app_local_data_dir)?;
    Ok(build_webview_clear_result(before_bytes, after_bytes))
}
