use sona_sqlite::DatabaseError;
use std::sync::Arc;
use tauri::{AppHandle, Manager, Runtime};

pub use sona_core::file_utils::current_time_millis;
pub use sona_core::project::{
    DEFAULT_POLISH_PRESET_ID, DEFAULT_SUMMARY_TEMPLATE_ID, DEFAULT_TRANSLATION_LANGUAGE,
    non_empty_trimmed_string, normalize_defaults, normalize_project_record_for_import,
    normalize_project_value, positive_millis, string_array, string_value,
};

pub(crate) const SETTINGS_FILE_NAME: &str = "settings.json";
pub(crate) const ACTIVE_PROJECT_SETTINGS_KEY: &str = "sona-active-project-id";

pub async fn run_project_task<R, T, F>(app: &AppHandle<R>, task: F) -> Result<T, String>
where
    R: Runtime,
    T: Send + 'static,
    F: FnOnce(crate::repositories::project::SqliteProjectRepository) -> Result<T, DatabaseError>
        + Send
        + 'static,
{
    let db = Arc::clone(app.state::<Arc<sona_sqlite::Database>>().inner());
    tauri::async_runtime::spawn_blocking(move || {
        task(crate::repositories::project::SqliteProjectRepository::new(
            db,
        ))
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|e| e.to_string())
}
