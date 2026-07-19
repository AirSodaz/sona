use tauri::{AppHandle, Manager, Runtime};

use crate::platform::paths::{PathKind, PathProvider, TauriPathProvider};
pub use sona_core::storage_usage::{StorageUsageSnapshot, WebviewBrowsingDataClearResult};

pub async fn get_usage_snapshot<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<StorageUsageSnapshot, String> {
    let context = crate::platform::database::sqlite_application_context(app);

    let snapshot = tauri::async_runtime::spawn_blocking(move || {
        sona_sqlite::load_storage_usage_snapshot_with_database(
            context.app_data_dir().to_path_buf(),
            context.database(),
            sona_runtime_fs::storage_usage_generated_at_now(),
        )
        .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())??;
    sona_ts_bind::validate_storage_usage_snapshot_for_typescript(&snapshot)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

pub async fn clear_webview_browsing_data<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<WebviewBrowsingDataClearResult, String> {
    let app_local_data_dir = TauriPathProvider::from_app(app)
        .resolve_path(PathKind::AppLocalData)
        .map_err(|error| error.to_string())?;
    let before_bytes = observable_webview_cache_bytes(app_local_data_dir.clone()).await?;
    let Some(main_window) = app.get_webview_window("main") else {
        return Err("Main WebView window is not available.".to_string());
    };

    main_window
        .clear_all_browsing_data()
        .map_err(|error| error.to_string())?;

    let after_bytes = observable_webview_cache_bytes(app_local_data_dir).await?;
    let result = sona_core::storage_usage::build_webview_clear_result(before_bytes, after_bytes);
    sona_ts_bind::validate_webview_browsing_data_clear_result_for_typescript(&result)
        .map_err(|error| error.to_string())?;
    Ok(result)
}

async fn observable_webview_cache_bytes(
    app_local_data_dir: std::path::PathBuf,
) -> Result<Option<u64>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        sona_sqlite::storage_usage::observable_webview_cache_bytes(&app_local_data_dir)
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())
}
