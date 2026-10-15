use sona_application::project::ProjectService as ApplicationProjectService;
use sona_core::project::{ProjectCreateInput, ProjectError, ProjectRecord, ProjectUpdateInput};
use sona_runtime_fs::{SystemClock, UuidGenerator};
use std::sync::Arc;

use crate::platform::database::DesktopSqliteState;
use crate::services::db_runner::run_sqlite_task;

/// Pure Rust Project domain service without any Tauri dependency.
#[derive(Clone)]
pub struct ProjectService {
    sqlite: DesktopSqliteState,
}
impl ProjectService {
    pub fn new(sqlite: DesktopSqliteState) -> Self {
        Self { sqlite }
    }

    pub fn sqlite(&self) -> &DesktopSqliteState {
        &self.sqlite
    }

    async fn run_service<T, F>(&self, task: F) -> Result<T, String>
    where
        T: Send + 'static,
        F: FnOnce(&ApplicationProjectService) -> Result<T, ProjectError> + Send + 'static,
    {
        run_sqlite_task(&self.sqlite, move |context| {
            let adapter = Arc::new(context.tag_adapter(Arc::new(UuidGenerator), Arc::new(SystemClock)));
            let service = ApplicationProjectService::new(adapter);
            task(&service).map_err(|error| error.to_string())
        })
        .await
    }

    pub async fn list(&self) -> Result<Vec<ProjectRecord>, String> {
        self.run_service(|service| service.list()).await
    }

    pub async fn create(&self, input: ProjectCreateInput) -> Result<ProjectRecord, String> {
        self.run_service(move |service| service.create(input)).await
    }

    pub async fn update(
        &self,
        project_id: String,
        updates: ProjectUpdateInput,
    ) -> Result<Option<ProjectRecord>, String> {
        self.run_service(move |service| service.update(&project_id, updates))
            .await
    }

    pub async fn delete(&self, project_id: String) -> Result<(), String> {
        self.run_service(move |service| service.delete(&project_id))
            .await
    }

    pub async fn delete_with_cascade(
        &self,
        project_id: String,
        cascade_action: String,
    ) -> Result<(), String> {
        self.run_service(move |service| {
            service.delete_with_cascade(&project_id, &cascade_action)
        })
        .await
    }

    pub async fn reorder(&self, project_ids: Vec<String>) -> Result<Vec<ProjectRecord>, String> {
        self.run_service(move |service| service.reorder(project_ids))
            .await
    }

    pub async fn get_active_project_id(&self) -> Result<Option<String>, String> {
        self.run_service(|service| service.get_active_project_id())
            .await
    }

    pub async fn set_active_project_id(&self, project_id: Option<String>) -> Result<(), String> {
        self.run_service(move |service| service.set_active_project_id(project_id))
            .await
    }

    pub async fn get_active_tag_id(&self) -> Result<Option<String>, String> {
        self.get_active_project_id().await
    }

    pub async fn set_active_tag_id(&self, tag_id: Option<String>) -> Result<(), String> {
        self.set_active_project_id(tag_id).await
    }
}

// Standalone functions for convenience or transition
pub async fn list_projects(sqlite: &DesktopSqliteState) -> Result<Vec<ProjectRecord>, String> {
    ProjectService::new(sqlite.clone()).list().await
}

pub async fn create_project(
    sqlite: &DesktopSqliteState,
    input: ProjectCreateInput,
) -> Result<ProjectRecord, String> {
    ProjectService::new(sqlite.clone()).create(input).await
}

pub async fn update_project(
    sqlite: &DesktopSqliteState,
    project_id: String,
    updates: ProjectUpdateInput,
) -> Result<Option<ProjectRecord>, String> {
    ProjectService::new(sqlite.clone())
        .update(project_id, updates)
        .await
}

pub async fn delete_project(sqlite: &DesktopSqliteState, project_id: String) -> Result<(), String> {
    ProjectService::new(sqlite.clone()).delete(project_id).await
}

pub async fn delete_project_with_cascade(
    sqlite: &DesktopSqliteState,
    project_id: String,
    cascade_action: String,
) -> Result<(), String> {
    ProjectService::new(sqlite.clone())
        .delete_with_cascade(project_id, cascade_action)
        .await
}

pub async fn reorder_projects(
    sqlite: &DesktopSqliteState,
    project_ids: Vec<String>,
) -> Result<Vec<ProjectRecord>, String> {
    ProjectService::new(sqlite.clone())
        .reorder(project_ids)
        .await
}

pub async fn get_active_project_id(sqlite: &DesktopSqliteState) -> Result<Option<String>, String> {
    ProjectService::new(sqlite.clone())
        .get_active_project_id()
        .await
}

pub async fn set_active_project_id(
    sqlite: &DesktopSqliteState,
    project_id: Option<String>,
) -> Result<(), String> {
    ProjectService::new(sqlite.clone())
        .set_active_project_id(project_id)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_project_service_without_tauri() {
        let temp = tempfile::tempdir().unwrap();
        let ctx = Arc::new(sona_sqlite::SqliteApplicationContext::open(temp.path()).unwrap());
        let state = DesktopSqliteState::new(ctx);
        let project_service = ProjectService::new(state);
        let projects = project_service.list().await.unwrap();
        assert_eq!(projects.len(), 0);

        let created = project_service
            .create(ProjectCreateInput {
                name: "Test Project".to_string(),
                description: Some("Desc".to_string()),
                icon: None,
                color: None,
                pipeline: None,
            })
            .await
            .unwrap();
        assert_eq!(created.name, "Test Project");

        let listed = project_service.list().await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, created.id);
    }
}
