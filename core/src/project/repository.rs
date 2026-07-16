use serde::{Deserialize, Serialize};

use super::ProjectRecord;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProjectStoredState {
    pub projects: Vec<ProjectRecord>,
    pub active_project_setting_json: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ActiveProjectSelection {
    pub setting_exists: bool,
    pub project_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ProjectRepositorySnapshot {
    pub projects: Vec<ProjectRecord>,
    pub active_project_id: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default, rename_all = "camelCase")]
pub struct ProjectDefaultsPatch {
    pub summary_template_id: Option<String>,
    pub translation_language: Option<String>,
    pub polish_preset_id: Option<String>,
    pub polish_scenario: Option<String>,
    pub polish_context: Option<String>,
    pub export_file_name_prefix: Option<String>,
    pub enabled_text_replacement_set_ids: Option<Vec<String>>,
    pub enabled_hotword_set_ids: Option<Vec<String>>,
    pub enabled_polish_keyword_set_ids: Option<Vec<String>>,
    pub enabled_speaker_profile_ids: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectPatch {
    pub name: Option<String>,
    pub icon: Option<String>,
    pub description: Option<String>,
    pub defaults: ProjectDefaultsPatch,
}

pub trait ProjectStore: Send + Sync {
    fn load_state(&self) -> Result<ProjectStoredState, String>;

    fn insert_project(&self, project: ProjectRecord) -> Result<ProjectRecord, String>;

    fn update_project(
        &self,
        project_id: &str,
        patch: ProjectPatch,
        updated_at: u64,
    ) -> Result<Option<ProjectRecord>, String>;

    fn delete_project(&self, project_id: &str) -> Result<(), String>;

    fn replace_projects(&self, projects: Vec<ProjectRecord>) -> Result<(), String>;

    fn reorder_projects(&self, project_ids: Vec<String>) -> Result<Vec<ProjectRecord>, String>;

    fn set_active_project_setting_json(&self, setting_json: String) -> Result<(), String>;
}
