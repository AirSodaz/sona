use serde_json::{Map, Value};

use crate::ports::time::UnixMillisClock;

use super::{
    ActiveTagSelection, TagCreateInput, TagError, TagListOptions, TagPatch, TagRecord,
    TagRepositorySnapshot, TagStore, TagStoredState, TagUpdateInput, active_tag_id_from_value,
    normalize_tag_value,
};

pub trait TagIdGenerator: Send + Sync {
    fn generate_id(&self) -> String;
}

pub struct TagRepositoryService<'a> {
    store: &'a dyn TagStore,
    ids: &'a dyn TagIdGenerator,
    clock: &'a dyn UnixMillisClock,
}

impl<'a> TagRepositoryService<'a> {
    pub fn new(
        store: &'a dyn TagStore,
        ids: &'a dyn TagIdGenerator,
        clock: &'a dyn UnixMillisClock,
    ) -> Self {
        Self { store, ids, clock }
    }

    pub fn load_state(&self) -> Result<TagRepositorySnapshot, TagError> {
        snapshot_from_state(self.store.load_state()?)
    }

    pub fn list_tags(&self, _options: TagListOptions) -> Result<Vec<TagRecord>, TagError> {
        Ok(self.store.load_state()?.tags)
    }

    pub fn replace_tags_json(&self, tags: Vec<Value>) -> Result<(), TagError> {
        let tags = tags
            .iter()
            .enumerate()
            .map(|(index, tag)| {
                let mut normalized = normalize_tag_value(tag);
                normalized.sort_order = index;
                normalized
            })
            .collect();
        self.store.replace_tags(tags)
    }

    pub fn replace_tags(&self, tags: Vec<TagRecord>) -> Result<(), TagError> {
        self.store.replace_tags(tags)
    }

    pub fn create_tag(&self, input: TagCreateInput) -> Result<TagRecord, TagError> {
        let now = self.clock.now_ms()?;
        self.store.insert_tag(TagRecord {
            id: self.ids.generate_id(),
            name: input.name,
            description: input.description.unwrap_or_default(),
            icon: input.icon.unwrap_or_default(),
            color: input.color.unwrap_or_default(),
            sort_order: self.store.load_state()?.tags.len(),
            created_at: now,
            updated_at: now,
        })
    }

    pub fn update_tag_json(
        &self,
        tag_id: &str,
        updates: Value,
    ) -> Result<Option<TagRecord>, TagError> {
        let Some(updates) = updates.as_object() else {
            return Ok(self
                .store
                .load_state()?
                .tags
                .into_iter()
                .find(|tag| tag.id == tag_id));
        };

        let updated_at = self.clock.now_ms()?;
        self.store
            .update_tag(tag_id, parse_tag_patch(updates), updated_at)
    }

    pub fn update_tag(
        &self,
        tag_id: &str,
        updates: TagUpdateInput,
    ) -> Result<Option<TagRecord>, TagError> {
        let updated_at = self.clock.now_ms()?;
        self.store.update_tag(
            tag_id,
            TagPatch {
                name: updates.name,
                icon: updates.icon,
                color: updates.color,
                description: updates.description,
            },
            updated_at,
        )
    }

    pub fn delete_tag(&self, tag_id: &str) -> Result<(), TagError> {
        self.store.delete_tag(tag_id)
    }

    pub fn reorder_tags(&self, tag_ids: Vec<String>) -> Result<Vec<TagRecord>, TagError> {
        self.store.reorder_tags(tag_ids)
    }

    pub fn get_active_tag_id(&self) -> Result<Option<String>, TagError> {
        Ok(self.get_active_tag_selection()?.tag_id)
    }

    pub fn get_active_tag_selection(&self) -> Result<ActiveTagSelection, TagError> {
        active_selection_from_setting_json(
            self.store.load_state()?.active_tag_setting_json.as_deref(),
        )
    }

    pub fn set_active_tag_id(&self, tag_id: Option<String>) -> Result<(), TagError> {
        let value = tag_id.map(Value::String).unwrap_or(Value::Null);
        self.store
            .set_active_tag_setting_json(serde_json::to_string(&value)?)
    }
}

fn snapshot_from_state(state: TagStoredState) -> Result<TagRepositorySnapshot, TagError> {
    let active_tag_id =
        active_selection_from_setting_json(state.active_tag_setting_json.as_deref())?.tag_id;
    Ok(TagRepositorySnapshot {
        tags: state.tags,
        active_tag_id,
    })
}

fn active_selection_from_setting_json(
    setting_json: Option<&str>,
) -> Result<ActiveTagSelection, TagError> {
    let tag_id = setting_json
        .map(serde_json::from_str::<Value>)
        .transpose()
        .map_err(TagError::Serialization)?
        .as_ref()
        .and_then(active_tag_id_from_value);
    Ok(ActiveTagSelection {
        setting_exists: setting_json.is_some(),
        tag_id,
    })
}

fn parse_tag_patch(updates: &Map<String, Value>) -> TagPatch {
    TagPatch {
        name: string_patch(updates.get("name")),
        icon: string_patch(updates.get("icon")),
        color: string_patch(updates.get("color")),
        description: string_patch(updates.get("description")),
    }
}

fn string_patch(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(ToOwned::to_owned)
}
