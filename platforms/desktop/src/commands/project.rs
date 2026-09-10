use sona_core::project::{ProjectCreateInput, ProjectRecord, ProjectUpdateInput};
use tauri::{AppHandle, Runtime};

#[tauri::command]
pub async fn project_list<R: Runtime>(app: AppHandle<R>) -> Result<Vec<ProjectRecord>, String> {
    crate::platform::tag_repository::list_projects(&app).await
}

#[tauri::command]
pub async fn project_create<R: Runtime>(app: AppHandle<R>, input: ProjectCreateInput) -> Result<ProjectRecord, String> {
    crate::platform::tag_repository::create_project(&app, input).await
}

#[tauri::command]
pub async fn project_update<R: Runtime>(app: AppHandle<R>, project_id: String, updates: ProjectUpdateInput) -> Result<Option<ProjectRecord>, String> {
    crate::platform::tag_repository::update_project(&app, project_id, updates).await
}

#[tauri::command]
pub async fn project_delete<R: Runtime>(app: AppHandle<R>, project_id: String, cascade_action: Option<String>) -> Result<(), String> {
    crate::platform::tag_repository::delete_project_with_cascade(&app, project_id, cascade_action.unwrap_or_else(|| "moveToInbox".into())).await
}

#[tauri::command]
pub async fn project_reorder<R: Runtime>(app: AppHandle<R>, project_ids: Vec<String>) -> Result<Vec<ProjectRecord>, String> {
    crate::platform::tag_repository::reorder_projects(&app, project_ids).await
}

#[tauri::command]
pub async fn project_get_active_id<R: Runtime>(app: AppHandle<R>) -> Result<Option<String>, String> {
    crate::platform::tag_repository::get_active_tag_id(&app).await
}

#[tauri::command]
pub async fn project_set_active_id<R: Runtime>(app: AppHandle<R>, project_id: Option<String>) -> Result<(), String> {
    crate::platform::tag_repository::set_active_tag_id(&app, project_id).await
}
