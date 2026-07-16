use tauri::{AppHandle, Runtime};

use crate::platform::project_repository::{
    create_project, delete_project, get_active_project_id, list_projects, reorder_projects,
    replace_projects, set_active_project_id, update_project,
};
use sona_core::project::{ProjectDefaultsInput, ProjectRecord, ProjectUpdateInput};

#[tauri::command]
pub async fn project_list<R: Runtime>(
    app: AppHandle<R>,
    fallback_enabled_polish_keyword_set_ids: Option<Vec<String>>,
    fallback_enabled_speaker_profile_ids: Option<Vec<String>>,
) -> Result<Vec<ProjectRecord>, String> {
    list_projects(
        &app,
        fallback_enabled_polish_keyword_set_ids,
        fallback_enabled_speaker_profile_ids,
    )
    .await
}

#[tauri::command]
pub async fn project_save_all<R: Runtime>(
    app: AppHandle<R>,
    projects: Vec<ProjectRecord>,
) -> Result<(), String> {
    replace_projects(&app, projects).await
}

#[tauri::command]
pub async fn project_create<R: Runtime>(
    app: AppHandle<R>,
    name: String,
    description: Option<String>,
    icon: Option<String>,
    defaults: ProjectDefaultsInput,
) -> Result<ProjectRecord, String> {
    create_project(&app, name, description, icon, defaults).await
}

#[tauri::command]
pub async fn project_update<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
    updates: ProjectUpdateInput,
) -> Result<Option<ProjectRecord>, String> {
    update_project(&app, project_id, updates).await
}

#[tauri::command]
pub async fn project_delete<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
) -> Result<(), String> {
    delete_project(&app, project_id).await
}

#[tauri::command]
pub async fn project_reorder<R: Runtime>(
    app: AppHandle<R>,
    project_ids: Vec<String>,
) -> Result<Vec<ProjectRecord>, String> {
    reorder_projects(&app, project_ids).await
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
