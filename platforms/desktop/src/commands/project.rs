use sona_core::project::{ProjectCreateInput, ProjectRecord, ProjectUpdateInput};
use tauri::State;

use crate::services::DesktopServices;

#[tauri::command]
pub async fn project_list(
    services: State<'_, DesktopServices>,
) -> Result<Vec<ProjectRecord>, String> {
    services.projects.list().await
}

#[tauri::command]
pub async fn project_create(
    services: State<'_, DesktopServices>,
    input: ProjectCreateInput,
) -> Result<ProjectRecord, String> {
    services.projects.create(input).await
}

#[tauri::command]
pub async fn project_update(
    services: State<'_, DesktopServices>,
    project_id: String,
    updates: ProjectUpdateInput,
) -> Result<Option<ProjectRecord>, String> {
    services.projects.update(project_id, updates).await
}

#[tauri::command]
pub async fn project_delete(
    services: State<'_, DesktopServices>,
    project_id: String,
    cascade_action: Option<String>,
) -> Result<(), String> {
    services
        .projects
        .delete_with_cascade(
            project_id,
            cascade_action.unwrap_or_else(|| "moveToInbox".into()),
        )
        .await
}

#[tauri::command]
pub async fn project_reorder(
    services: State<'_, DesktopServices>,
    project_ids: Vec<String>,
) -> Result<Vec<ProjectRecord>, String> {
    services.projects.reorder(project_ids).await
}

#[tauri::command]
pub async fn project_get_active_id(
    services: State<'_, DesktopServices>,
) -> Result<Option<String>, String> {
    services.projects.get_active_tag_id().await
}

#[tauri::command]
pub async fn project_set_active_id(
    services: State<'_, DesktopServices>,
    project_id: Option<String>,
) -> Result<(), String> {
    services.projects.set_active_tag_id(project_id).await
}
