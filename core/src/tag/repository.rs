use serde::{Deserialize, Serialize};

use super::TagRecord;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TagStoredState {
    pub tags: Vec<TagRecord>,
    pub active_tag_setting_json: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ActiveTagSelection {
    pub setting_exists: bool,
    pub tag_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TagRepositorySnapshot {
    pub tags: Vec<TagRecord>,
    pub active_tag_id: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default, rename_all = "camelCase")]
pub struct TagDefaultsPatch {
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
pub struct TagPatch {
    pub name: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
    pub defaults: TagDefaultsPatch,
}

pub trait TagStore: Send + Sync {
    fn load_state(&self) -> Result<TagStoredState, String>;

    fn insert_tag(&self, tag: TagRecord) -> Result<TagRecord, String>;

    fn update_tag(
        &self,
        tag_id: &str,
        patch: TagPatch,
        updated_at: u64,
    ) -> Result<Option<TagRecord>, String>;

    fn delete_tag(&self, tag_id: &str) -> Result<(), String>;

    fn replace_tags(&self, tags: Vec<TagRecord>) -> Result<(), String>;

    fn reorder_tags(&self, tag_ids: Vec<String>) -> Result<Vec<TagRecord>, String>;

    fn set_active_tag_setting_json(&self, setting_json: String) -> Result<(), String>;
}
