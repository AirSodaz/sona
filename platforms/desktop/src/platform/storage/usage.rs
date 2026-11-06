pub use sona_core::storage_usage::{StorageUsageSnapshot, WebviewBrowsingDataClearResult};
use sona_sqlite::SqliteApplicationContext;
use std::path::PathBuf;
use std::sync::Arc;

pub async fn get_usage_snapshot(
    context: &Arc<SqliteApplicationContext>,
    models_dir: Option<PathBuf>,
) -> Result<StorageUsageSnapshot, String> {
    let ctx = context.clone();
    tokio::task::spawn_blocking(move || {
        sona_sqlite::load_storage_usage_snapshot_with_database_and_models_dir(
            ctx.app_data_dir().to_path_buf(),
            ctx.database(),
            models_dir,
            sona_runtime_fs::storage_usage_generated_at_now(),
        )
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

pub async fn clear_webview_browsing_data() -> Result<WebviewBrowsingDataClearResult, String> {
    // Pure Rust GPUI architecture contains no WebView browsing data.
    Ok(sona_application::storage_usage::build_webview_clear_result(
        None, None,
    ))
}
