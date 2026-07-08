use serde_json::Value;
use tauri::{AppHandle, Runtime};

use crate::platform::project_repository::{
    get_active_project_id, run_project_task, set_active_project_id,
};
use sona_core::project::{
    ProjectCreateInput, ProjectDefaultsInput, ProjectListOptions, ProjectRecord,
};

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
    get_active_project_id(&app).await
}

#[tauri::command]
pub async fn project_set_active_id<R: Runtime>(
    app: AppHandle<R>,
    project_id: Option<String>,
) -> Result<(), String> {
    set_active_project_id(&app, project_id).await
}
