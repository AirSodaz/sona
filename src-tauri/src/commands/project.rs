use serde_json::Value;
use tauri::{AppHandle, Runtime};
use tauri_plugin_store::StoreExt;

use crate::repositories::project::repository::{
    ACTIVE_PROJECT_SETTINGS_KEY, SETTINGS_FILE_NAME, run_project_task,
};
use crate::repositories::project::{
    ProjectCreateInput, ProjectDefaultsInput, ProjectListOptions, ProjectRecord,
};

#[tauri::command]
pub async fn project_list<R: Runtime>(
    app: AppHandle<R>,
    fallback_enabled_polish_keyword_set_ids: Option<Vec<String>>,
    fallback_enabled_speaker_profile_ids: Option<Vec<String>>,
) -> Result<Vec<ProjectRecord>, String> {
    run_project_task(app, move |repository| {
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
    run_project_task(app, move |repository| repository.save_all_values(projects)).await
}

#[tauri::command]
pub async fn project_create<R: Runtime>(
    app: AppHandle<R>,
    name: String,
    description: Option<String>,
    icon: Option<String>,
    defaults: ProjectDefaultsInput,
) -> Result<ProjectRecord, String> {
    run_project_task(app, move |repository| {
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
    run_project_task(app, move |repository| {
        repository.update(&project_id, updates)
    })
    .await
}

#[tauri::command]
pub async fn project_delete<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
) -> Result<(), String> {
    run_project_task(app, move |repository| repository.delete(&project_id)).await
}

#[tauri::command]
pub async fn project_reorder<R: Runtime>(
    app: AppHandle<R>,
    project_ids: Vec<String>,
) -> Result<Vec<ProjectRecord>, String> {
    run_project_task(app, move |repository| repository.reorder(project_ids)).await
}

#[tauri::command]
pub async fn project_get_active_id<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Option<String>, String> {
    let store = app
        .store(SETTINGS_FILE_NAME)
        .map_err(|error| error.to_string())?;
    Ok(store
        .get(ACTIVE_PROJECT_SETTINGS_KEY)
        .and_then(|value| value.as_str().map(str::trim).map(ToOwned::to_owned))
        .filter(|value| !value.is_empty()))
}

#[tauri::command]
pub async fn project_set_active_id<R: Runtime>(
    app: AppHandle<R>,
    project_id: Option<String>,
) -> Result<(), String> {
    let store = app
        .store(SETTINGS_FILE_NAME)
        .map_err(|error| error.to_string())?;
    store.set(
        ACTIVE_PROJECT_SETTINGS_KEY,
        project_id.map(Value::String).unwrap_or(Value::Null),
    );
    store.save().map_err(|error| error.to_string())
}
