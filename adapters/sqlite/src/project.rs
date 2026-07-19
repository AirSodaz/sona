#![allow(deprecated)]

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;
use sona_core::ports::time::UnixMillisClock;
use sona_core::project::{
    ActiveProjectSelection, ProjectCreateInput, ProjectDefaults, ProjectDefaultsPatch,
    ProjectError, ProjectIdGenerator, ProjectListOptions, ProjectPatch, ProjectRecord,
    ProjectRepositoryService, ProjectRepositorySnapshot, ProjectStore, ProjectStoredState,
    ProjectUpdateInput,
};
use sona_core::tag::{TagDefaults, TagDefaultsPatch, TagError, TagPatch, TagRecord, TagStore};

use crate::SqliteTagRepository;
use crate::ports::Database as DatabasePort;

/// Compatibility repository for the legacy Project API.
///
/// Tag is the canonical persistence and Sync model. This adapter intentionally
/// contains no SQL and translates all legacy Project operations to `TagStore`.
#[derive(Clone)]
#[deprecated(note = "Project is a compatibility API; use SqliteTagRepository for new code")]
pub struct SqliteProjectRepository<D = crate::Database>
where
    D: DatabasePort,
{
    tags: SqliteTagRepository<D>,
}

impl<D> SqliteProjectRepository<D>
where
    D: DatabasePort,
{
    pub fn new(db: Arc<D>) -> Self {
        Self {
            tags: SqliteTagRepository::new(db),
        }
    }
}

#[deprecated(note = "Project is a compatibility API; use SqliteTagAdapter for new code")]
pub struct SqliteProjectAdapter<D = crate::Database>
where
    D: DatabasePort,
{
    repository: SqliteProjectRepository<D>,
    ids: Arc<dyn ProjectIdGenerator>,
    clock: Arc<dyn UnixMillisClock>,
}

impl<D> SqliteProjectAdapter<D>
where
    D: DatabasePort,
{
    pub fn new(
        db: Arc<D>,
        ids: Arc<dyn ProjectIdGenerator>,
        clock: Arc<dyn UnixMillisClock>,
    ) -> Self {
        Self {
            repository: SqliteProjectRepository::new(db),
            ids,
            clock,
        }
    }

    pub fn load_state(&self) -> Result<ProjectRepositorySnapshot, ProjectError> {
        self.service().load_state()
    }

    pub fn list_projects(
        &self,
        options: ProjectListOptions,
    ) -> Result<Vec<ProjectRecord>, ProjectError> {
        self.service().list_projects(options)
    }

    pub fn replace_projects_json(&self, projects: Vec<Value>) -> Result<(), ProjectError> {
        self.service().replace_projects_json(projects)
    }

    pub fn replace_projects(&self, projects: Vec<ProjectRecord>) -> Result<(), ProjectError> {
        self.service().replace_projects(projects)
    }

    pub fn create_project(&self, input: ProjectCreateInput) -> Result<ProjectRecord, ProjectError> {
        self.service().create_project(input)
    }

    pub fn update_project_json(
        &self,
        project_id: &str,
        updates: Value,
    ) -> Result<Option<ProjectRecord>, ProjectError> {
        self.service().update_project_json(project_id, updates)
    }

    pub fn update_project(
        &self,
        project_id: &str,
        updates: ProjectUpdateInput,
    ) -> Result<Option<ProjectRecord>, ProjectError> {
        self.service().update_project(project_id, updates)
    }

    pub fn delete_project(&self, project_id: &str) -> Result<(), ProjectError> {
        self.service().delete_project(project_id)
    }

    pub fn reorder_projects(
        &self,
        project_ids: Vec<String>,
    ) -> Result<Vec<ProjectRecord>, ProjectError> {
        self.service().reorder_projects(project_ids)
    }

    pub fn get_active_project_selection(&self) -> Result<ActiveProjectSelection, ProjectError> {
        self.service().get_active_project_selection()
    }

    pub fn set_active_project_id(&self, project_id: Option<String>) -> Result<(), ProjectError> {
        self.service().set_active_project_id(project_id)
    }

    fn service(&self) -> ProjectRepositoryService<'_> {
        ProjectRepositoryService::new(&self.repository, self.ids.as_ref(), self.clock.as_ref())
    }
}

impl<D> ProjectStore for SqliteProjectRepository<D>
where
    D: DatabasePort,
{
    fn load_state(&self) -> Result<ProjectStoredState, ProjectError> {
        let state = TagStore::load_state(&self.tags).map_err(project_error)?;
        Ok(ProjectStoredState {
            projects: state.tags.into_iter().map(project_from_tag).collect(),
            active_project_setting_json: state.active_tag_setting_json,
        })
    }

    fn insert_project(&self, project: ProjectRecord) -> Result<ProjectRecord, ProjectError> {
        let sort_order = TagStore::load_state(&self.tags)
            .map_err(project_error)?
            .tags
            .len();
        TagStore::insert_tag(
            &self.tags,
            tag_from_project(project, String::new(), sort_order),
        )
        .map(project_from_tag)
        .map_err(project_error)
    }

    fn update_project(
        &self,
        project_id: &str,
        patch: ProjectPatch,
        updated_at: u64,
    ) -> Result<Option<ProjectRecord>, ProjectError> {
        TagStore::update_tag(
            &self.tags,
            project_id,
            tag_patch_from_project(patch),
            updated_at,
        )
        .map(|tag| tag.map(project_from_tag))
        .map_err(project_error)
    }

    fn delete_project(&self, project_id: &str) -> Result<(), ProjectError> {
        TagStore::delete_tag(&self.tags, project_id).map_err(project_error)
    }

    fn replace_projects(&self, projects: Vec<ProjectRecord>) -> Result<(), ProjectError> {
        let existing_colors = TagStore::load_state(&self.tags)
            .map_err(project_error)?
            .tags
            .into_iter()
            .map(|tag| (tag.id, tag.color))
            .collect::<HashMap<_, _>>();
        let tags = projects
            .into_iter()
            .enumerate()
            .map(|(sort_order, project)| {
                let color = existing_colors
                    .get(&project.id)
                    .cloned()
                    .unwrap_or_default();
                tag_from_project(project, color, sort_order)
            })
            .collect();
        TagStore::replace_tags(&self.tags, tags).map_err(project_error)
    }

    fn reorder_projects(
        &self,
        project_ids: Vec<String>,
    ) -> Result<Vec<ProjectRecord>, ProjectError> {
        TagStore::reorder_tags(&self.tags, project_ids)
            .map(|tags| tags.into_iter().map(project_from_tag).collect())
            .map_err(project_error)
    }

    fn set_active_project_setting_json(&self, setting_json: String) -> Result<(), ProjectError> {
        TagStore::set_active_tag_setting_json(&self.tags, setting_json).map_err(project_error)
    }
}

fn project_from_tag(tag: TagRecord) -> ProjectRecord {
    ProjectRecord {
        id: tag.id,
        name: tag.name,
        description: tag.description,
        icon: tag.icon,
        created_at: tag.created_at,
        updated_at: tag.updated_at,
        defaults: project_defaults_from_tag(tag.defaults),
    }
}

fn tag_from_project(project: ProjectRecord, color: String, sort_order: usize) -> TagRecord {
    TagRecord {
        id: project.id,
        name: project.name,
        description: project.description,
        icon: project.icon,
        color,
        sort_order,
        created_at: project.created_at,
        updated_at: project.updated_at,
        defaults: tag_defaults_from_project(project.defaults),
    }
}

fn project_defaults_from_tag(defaults: TagDefaults) -> ProjectDefaults {
    ProjectDefaults {
        summary_template_id: defaults.summary_template_id,
        translation_language: defaults.translation_language,
        polish_preset_id: defaults.polish_preset_id,
        polish_scenario: defaults.polish_scenario,
        polish_context: defaults.polish_context,
        export_file_name_prefix: defaults.export_file_name_prefix,
        enabled_text_replacement_set_ids: defaults.enabled_text_replacement_set_ids,
        enabled_hotword_set_ids: defaults.enabled_hotword_set_ids,
        enabled_polish_keyword_set_ids: defaults.enabled_polish_keyword_set_ids,
        enabled_speaker_profile_ids: defaults.enabled_speaker_profile_ids,
    }
}

fn tag_defaults_from_project(defaults: ProjectDefaults) -> TagDefaults {
    TagDefaults {
        summary_template_id: defaults.summary_template_id,
        translation_language: defaults.translation_language,
        polish_preset_id: defaults.polish_preset_id,
        polish_scenario: defaults.polish_scenario,
        polish_context: defaults.polish_context,
        export_file_name_prefix: defaults.export_file_name_prefix,
        enabled_text_replacement_set_ids: defaults.enabled_text_replacement_set_ids,
        enabled_hotword_set_ids: defaults.enabled_hotword_set_ids,
        enabled_polish_keyword_set_ids: defaults.enabled_polish_keyword_set_ids,
        enabled_speaker_profile_ids: defaults.enabled_speaker_profile_ids,
    }
}

fn tag_patch_from_project(patch: ProjectPatch) -> TagPatch {
    TagPatch {
        name: patch.name,
        icon: patch.icon,
        color: None,
        description: patch.description,
        defaults: tag_defaults_patch_from_project(patch.defaults),
    }
}

fn tag_defaults_patch_from_project(patch: ProjectDefaultsPatch) -> TagDefaultsPatch {
    TagDefaultsPatch {
        summary_template_id: patch.summary_template_id,
        translation_language: patch.translation_language,
        polish_preset_id: patch.polish_preset_id,
        polish_scenario: patch.polish_scenario,
        polish_context: patch.polish_context,
        export_file_name_prefix: patch.export_file_name_prefix,
        enabled_text_replacement_set_ids: patch.enabled_text_replacement_set_ids,
        enabled_hotword_set_ids: patch.enabled_hotword_set_ids,
        enabled_polish_keyword_set_ids: patch.enabled_polish_keyword_set_ids,
        enabled_speaker_profile_ids: patch.enabled_speaker_profile_ids,
    }
}

fn project_error(error: TagError) -> ProjectError {
    match error {
        TagError::Repository(message) => ProjectError::Repository(message),
        TagError::Serialization(source) => ProjectError::Serialization(source),
        TagError::Clock(source) => ProjectError::Clock(source),
    }
}
