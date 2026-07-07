use serde_json::Value;
use std::sync::Arc;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_store::StoreExt;

use crate::platform::project_repository::{
    SETTINGS_FILE_NAME, active_project_id_from_value, run_project_task,
};
use sona_core::project::{
    ACTIVE_PROJECT_SETTINGS_KEY, ProjectCreateInput, ProjectDefaultsInput, ProjectListOptions,
    ProjectRecord,
};

fn sqlite_config_store<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<sona_sqlite::config_store::SqliteConfigStore, String> {
    let db = Arc::clone(app.state::<Arc<sona_sqlite::Database>>().inner());
    Ok(sona_sqlite::config_store::SqliteConfigStore::new(db))
}

#[tauri::command]
pub async fn project_list<R: Runtime>(
    app: AppHandle<R>,
    fallback_enabled_polish_keyword_set_ids: Option<Vec<String>>,
    fallback_enabled_speaker_profile_ids: Option<Vec<String>>,
) -> Result<Vec<ProjectRecord>, String> {
    run_project_task(&app, move |repository| {
        repository.list(ProjectListOptions {
            fallback_enabled_polish_keyword_set_ids: fallback_enabled_polish_keyword_set_ids
                .unwrap_or_default(),
            fallback_enabled_speaker_profile_ids: fallback_enabled_speaker_profile_ids
                .unwrap_or_default(),
        })
    })
    .await
}

#[tauri::command]
pub async fn project_save_all<R: Runtime>(
    app: AppHandle<R>,
    projects: Vec<Value>,
) -> Result<(), String> {
    run_project_task(&app, move |repository| repository.save_all_values(projects)).await
}

#[tauri::command]
pub async fn project_create<R: Runtime>(
    app: AppHandle<R>,
    name: String,
    description: Option<String>,
    icon: Option<String>,
    defaults: ProjectDefaultsInput,
) -> Result<ProjectRecord, String> {
    run_project_task(&app, move |repository| {
        repository.create(ProjectCreateInput {
            name,
            description,
            icon,
            defaults,
        })
    })
    .await
}

#[tauri::command]
pub async fn project_update<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
    updates: Value,
) -> Result<Option<ProjectRecord>, String> {
    run_project_task(&app, move |repository| {
        repository.update(&project_id, updates)
    })
    .await
}

#[tauri::command]
pub async fn project_delete<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
) -> Result<(), String> {
    run_project_task(&app, move |repository| repository.delete(&project_id)).await
}

#[tauri::command]
pub async fn project_reorder<R: Runtime>(
    app: AppHandle<R>,
    project_ids: Vec<String>,
) -> Result<Vec<ProjectRecord>, String> {
    run_project_task(&app, move |repository| repository.reorder(project_ids)).await
}

#[tauri::command]
pub async fn project_get_active_id<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Option<String>, String> {
    let sqlite_store = sqlite_config_store(&app)?;
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

#[tauri::command]
pub async fn project_set_active_id<R: Runtime>(
    app: AppHandle<R>,
    project_id: Option<String>,
) -> Result<(), String> {
    sqlite_config_store(&app)?
        .set_setting(
            ACTIVE_PROJECT_SETTINGS_KEY,
            &project_id.map(Value::String).unwrap_or(Value::Null),
        )
        .map_err(|error| error.to_string())
}
