use serde_json::Value;
use sona_sqlite::DatabaseError;
use tauri::{AppHandle, Runtime};
use tauri_plugin_store::StoreExt;

pub use sona_core::project::{
    ACTIVE_PROJECT_SETTINGS_KEY, DEFAULT_POLISH_PRESET_ID, DEFAULT_SUMMARY_TEMPLATE_ID,
    DEFAULT_TRANSLATION_LANGUAGE, active_project_id_from_value, non_empty_trimmed_string,
    normalize_defaults, normalize_project_record_for_import, normalize_project_value,
    positive_millis, string_array, string_value,
};

pub(crate) const SETTINGS_FILE_NAME: &str = "settings.json";

pub async fn run_project_task<R, T, F>(app: &AppHandle<R>, task: F) -> Result<T, String>
where
    R: Runtime,
    T: Send + 'static,
    F: FnOnce(sona_sqlite::project::SqliteProjectRepository) -> Result<T, DatabaseError>
        + Send
        + 'static,
{
    let db = crate::platform::database::sqlite_database(app);
    tauri::async_runtime::spawn_blocking(move || {
        task(sona_sqlite::project::SqliteProjectRepository::new(db))
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|e| e.to_string())
}

pub async fn get_active_project_id<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Option<String>, String> {
    let sqlite_store = crate::platform::database::sqlite_config_store(app);
    if let Some(value) = sqlite_store
        .get_setting(ACTIVE_PROJECT_SETTINGS_KEY)
        .map_err(|error| error.to_string())?
    {
        return Ok(active_project_id_from_value(&value));
    }

    let legacy_store = app
        .store(SETTINGS_FILE_NAME)
        .map_err(|error| error.to_string())?;
    let active_project_id = legacy_store
        .get(ACTIVE_PROJECT_SETTINGS_KEY)
        .and_then(|value| active_project_id_from_value(&value));

    if let Some(project_id) = &active_project_id {
        sqlite_store
            .set_setting(
                ACTIVE_PROJECT_SETTINGS_KEY,
                &Value::String(project_id.clone()),
            )
            .map_err(|error| error.to_string())?;
    }

    Ok(active_project_id)
}

pub async fn set_active_project_id<R: Runtime>(
    app: &AppHandle<R>,
    project_id: Option<String>,
) -> Result<(), String> {
    crate::platform::database::sqlite_config_store(app)
        .set_setting(
            ACTIVE_PROJECT_SETTINGS_KEY,
            &project_id.map(Value::String).unwrap_or(Value::Null),
        )
        .map_err(|error| error.to_string())
}
