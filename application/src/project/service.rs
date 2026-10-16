use std::sync::Arc;

use sona_core::project::error::ProjectError;
use sona_core::project::ports::ProjectRepository;
use sona_core::project::{ProjectCreateInput, ProjectRecord, ProjectUpdateInput};

pub struct ProjectService {
    repository: Arc<dyn ProjectRepository>,
}

impl ProjectService {
    pub fn new(repository: Arc<dyn ProjectRepository>) -> Self {
        Self { repository }
    }

    pub fn list(&self) -> Result<Vec<ProjectRecord>, ProjectError> {
        self.repository.list_projects()
    }

    pub fn create(&self, input: ProjectCreateInput) -> Result<ProjectRecord, ProjectError> {
        if input.name.trim().is_empty() {
            return Err(ProjectError::InvalidInput(
                "Project name cannot be empty".to_string(),
            ));
        }
        self.repository.create_project(input)
    }

    pub fn update(
        &self,
        project_id: &str,
        updates: ProjectUpdateInput,
    ) -> Result<Option<ProjectRecord>, ProjectError> {
        if project_id.trim().is_empty() {
            return Err(ProjectError::InvalidInput(
                "Project id cannot be empty".to_string(),
            ));
        }
        if let Some(name) = &updates.name
            && name.trim().is_empty()
        {
            return Err(ProjectError::InvalidInput(
                "Project name cannot be empty".to_string(),
            ));
        }
        self.repository.update_project(project_id, updates)
    }

    pub fn delete(&self, project_id: &str) -> Result<(), ProjectError> {
        if project_id.trim().is_empty() {
            return Err(ProjectError::InvalidInput(
                "Project id cannot be empty".to_string(),
            ));
        }
        self.repository.delete_project(project_id)
    }

    pub fn delete_with_cascade(
        &self,
        project_id: &str,
        cascade_action: &str,
    ) -> Result<(), ProjectError> {
        if project_id.trim().is_empty() {
            return Err(ProjectError::InvalidInput(
                "Project id cannot be empty".to_string(),
            ));
        }
        self.repository
            .delete_project_with_cascade(project_id, cascade_action)
    }

    pub fn reorder(&self, project_ids: Vec<String>) -> Result<Vec<ProjectRecord>, ProjectError> {
        self.repository.reorder_projects(project_ids)
    }

    pub fn get_active_project_id(&self) -> Result<Option<String>, ProjectError> {
        self.repository.get_active_project_id()
    }

    pub fn set_active_project_id(&self, project_id: Option<String>) -> Result<(), ProjectError> {
        self.repository.set_active_project_id(project_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sona_runtime_fs::{SystemClock, UuidGenerator};
    use sona_sqlite::SqliteApplicationContext;

    #[test]
    fn test_project_service_lifecycle() {
        let temp = tempfile::tempdir().unwrap();
        let ctx = Arc::new(SqliteApplicationContext::open(temp.path()).unwrap());
        let adapter = Arc::new(ctx.tag_adapter(Arc::new(UuidGenerator), Arc::new(SystemClock)));
        let service = ProjectService::new(adapter);

        let projects = service.list().unwrap();
        assert_eq!(projects.len(), 0);

        let created = service
            .create(ProjectCreateInput {
                name: "Work".to_string(),
                description: Some("Work stuff".to_string()),
                icon: Some("briefcase".to_string()),
                color: Some("#ff0000".to_string()),
                pipeline: None,
            })
            .unwrap();
        assert_eq!(created.name, "Work");

        let empty_error = service.create(ProjectCreateInput {
            name: "   ".to_string(),
            description: None,
            icon: None,
            color: None,
            pipeline: None,
        });
        assert!(empty_error.is_err());

        let active = service.get_active_project_id().unwrap();
        assert_eq!(active, None);

        service
            .set_active_project_id(Some(created.id.clone()))
            .unwrap();
        let active = service.get_active_project_id().unwrap();
        assert_eq!(active, Some(created.id.clone()));

        let updated = service
            .update(
                &created.id,
                ProjectUpdateInput {
                    name: Some("Work Updated".to_string()),
                    description: None,
                    icon: None,
                    color: None,
                    pipeline: None,
                },
            )
            .unwrap()
            .unwrap();
        assert_eq!(updated.name, "Work Updated");

        let empty_update = service.update(
            &created.id,
            ProjectUpdateInput {
                name: Some("   ".to_string()),
                description: None,
                icon: None,
                color: None,
                pipeline: None,
            },
        );
        assert!(empty_update.is_err());

        let reordered = service.reorder(vec![created.id.clone()]).unwrap();
        assert_eq!(reordered.len(), 1);

        service.set_active_project_id(None).unwrap();
        assert_eq!(service.get_active_project_id().unwrap(), None);

        assert!(service.delete("   ").is_err());
        assert!(service.delete_with_cascade("   ", "moveToInbox").is_err());

        service
            .delete_with_cascade(&created.id, "moveToInbox")
            .unwrap();
        let projects = service.list().unwrap();
        assert_eq!(projects.len(), 0);
    }
}
