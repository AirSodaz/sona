use serde::{Deserialize, Serialize};

use super::{TagError, TagRecord};

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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TagPatch {
    pub name: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
}

pub trait TagStore: Send + Sync {
    fn load_state(&self) -> Result<TagStoredState, TagError>;

    fn insert_tag(&self, tag: TagRecord) -> Result<TagRecord, TagError>;

    fn update_tag(
        &self,
        tag_id: &str,
        patch: TagPatch,
        updated_at: u64,
    ) -> Result<Option<TagRecord>, TagError>;

    fn delete_tag(&self, tag_id: &str) -> Result<(), TagError>;

    fn replace_tags(&self, tags: Vec<TagRecord>) -> Result<(), TagError>;

    fn reorder_tags(&self, tag_ids: Vec<String>) -> Result<Vec<TagRecord>, TagError>;

    fn set_active_tag_setting_json(&self, setting_json: String) -> Result<(), TagError>;
}
