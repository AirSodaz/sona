use crate::services::DesktopServices;
use sona_core::project::{ProjectCreateInput, ProjectRecord, ProjectUpdateInput};

use super::types::{CreateProjectRequest, UpdateProjectRequest};

pub async fn list_projects(services: &DesktopServices) -> Result<Vec<ProjectRecord>, String> {
    services.projects.list().await
}

pub async fn set_active_project(
    services: &DesktopServices,
    project_id: Option<String>,
) -> Result<bool, String> {
    services.projects.set_active_project_id(project_id).await?;
    Ok(true)
}

pub async fn create_project(
    services: &DesktopServices,
    req: CreateProjectRequest,
) -> Result<ProjectRecord, String> {
    let input = ProjectCreateInput {
        name: req.name,
        description: req.description,
        icon: req.icon,
        color: req.color,
        pipeline: None,
    };
    services.projects.create(input).await
}

pub async fn update_project(
    services: &DesktopServices,
    project_id: String,
    req: UpdateProjectRequest,
) -> Result<Option<ProjectRecord>, String> {
    let input = ProjectUpdateInput {
        name: req.name,
        description: req.description,
        icon: req.icon,
        color: req.color,
        pipeline: None,
    };
    services.projects.update(project_id, input).await
}

pub async fn delete_project(
    services: &DesktopServices,
    project_id: String,
    cascade_action: Option<String>,
) -> Result<bool, String> {
    let action = match cascade_action.as_deref() {
        Some("trash") | Some("deleteItems") => "deleteItems",
        _ => "moveToInbox",
    };
    services
        .projects
        .delete_with_cascade(project_id, action.to_string())
        .await?;
    Ok(true)
}
