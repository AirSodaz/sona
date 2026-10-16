use super::error::ProjectError;
use super::{ProjectCreateInput, ProjectRecord, ProjectUpdateInput};

pub trait ProjectRepository: Send + Sync {
    fn list_projects(&self) -> Result<Vec<ProjectRecord>, ProjectError>;
    fn create_project(&self, input: ProjectCreateInput) -> Result<ProjectRecord, ProjectError>;
    fn update_project(
        &self,
        project_id: &str,
        updates: ProjectUpdateInput,
    ) -> Result<Option<ProjectRecord>, ProjectError>;
    fn delete_project(&self, project_id: &str) -> Result<(), ProjectError>;
    fn delete_project_with_cascade(
        &self,
        project_id: &str,
        cascade_action: &str,
    ) -> Result<(), ProjectError>;
    fn reorder_projects(
        &self,
        project_ids: Vec<String>,
    ) -> Result<Vec<ProjectRecord>, ProjectError>;
    fn get_active_project_id(&self) -> Result<Option<String>, ProjectError>;
    fn set_active_project_id(&self, project_id: Option<String>) -> Result<(), ProjectError>;
}
