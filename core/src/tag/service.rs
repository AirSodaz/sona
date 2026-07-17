use serde_json::{Map, Value};

use crate::ports::time::UnixMillisClock;

use super::{
    ActiveTagSelection, DEFAULT_POLISH_PRESET_ID, DEFAULT_SUMMARY_TEMPLATE_ID,
    DEFAULT_TRANSLATION_LANGUAGE, TagCreateInput, TagDefaults, TagDefaultsInput, TagDefaultsPatch,
    TagError, TagListOptions, TagPatch, TagRecord, TagRepositorySnapshot, TagStore, TagStoredState,
    TagUpdateInput, active_tag_id_from_value, normalize_defaults,
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
            .map(|(index, tag)| normalize_replacement_tag(tag, index))
            .collect();
        self.store.replace_tags(tags)
    }

    pub fn replace_tags(&self, tags: Vec<TagRecord>) -> Result<(), TagError> {
        self.store.replace_tags(tags)
    }

    pub fn create_tag(&self, input: TagCreateInput) -> Result<TagRecord, TagError> {
        let now = self.clock.now_ms()?;
        let tag = TagRecord {
            id: self.ids.generate_id(),
            name: input.name,
            description: input.description.unwrap_or_default(),
            icon: input.icon.unwrap_or_default(),
            color: input.color.unwrap_or_default(),
            sort_order: self.store.load_state()?.tags.len(),
            created_at: now,
            updated_at: now,
            defaults: create_defaults(input.defaults),
        };
        self.store.insert_tag(tag)
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

        let patch = parse_tag_patch(updates);
        let updated_at = self.clock.now_ms()?;
        self.store.update_tag(tag_id, patch, updated_at)
    }

    pub fn update_tag(
        &self,
        tag_id: &str,
        updates: TagUpdateInput,
    ) -> Result<Option<TagRecord>, TagError> {
        let patch = TagPatch {
            name: updates.name,
            icon: updates.icon,
            color: updates.color,
            description: updates.description,
            defaults: updates.defaults.unwrap_or_default(),
        };
        let updated_at = self.clock.now_ms()?;
        self.store.update_tag(tag_id, patch, updated_at)
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
        let setting_json = serde_json::to_string(&value)?;
        self.store.set_active_tag_setting_json(setting_json)
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

fn create_defaults(input: TagDefaultsInput) -> TagDefaults {
    TagDefaults {
        summary_template_id: input
            .summary_template_id
            .unwrap_or_else(|| DEFAULT_SUMMARY_TEMPLATE_ID.to_string()),
        translation_language: input
            .translation_language
            .unwrap_or_else(|| DEFAULT_TRANSLATION_LANGUAGE.to_string()),
        polish_preset_id: input
            .polish_preset_id
            .unwrap_or_else(|| DEFAULT_POLISH_PRESET_ID.to_string()),
        polish_scenario: input.polish_scenario,
        polish_context: input.polish_context,
        export_file_name_prefix: input.export_file_name_prefix.unwrap_or_default(),
        enabled_text_replacement_set_ids: input
            .enabled_text_replacement_set_ids
            .unwrap_or_default(),
        enabled_hotword_set_ids: input.enabled_hotword_set_ids.unwrap_or_default(),
        enabled_polish_keyword_set_ids: input.enabled_polish_keyword_set_ids.unwrap_or_default(),
        enabled_speaker_profile_ids: input.enabled_speaker_profile_ids.unwrap_or_default(),
    }
}

fn normalize_replacement_tag(input: &Value, index: usize) -> TagRecord {
    let source = input.as_object();
    let defaults = source
        .and_then(|object| object.get("defaults"))
        .and_then(Value::as_object);

    TagRecord {
        id: replacement_string(source, "id"),
        name: replacement_string(source, "name"),
        description: replacement_string(source, "description"),
        icon: replacement_string(source, "icon"),
        color: replacement_string(source, "color"),
        sort_order: source
            .and_then(|source| source.get("sortOrder"))
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or(index),
        created_at: replacement_timestamp(source, "createdAt"),
        updated_at: replacement_timestamp(source, "updatedAt"),
        defaults: normalize_defaults(defaults, &TagListOptions::default()),
    }
}

fn replacement_string(source: Option<&Map<String, Value>>, key: &str) -> String {
    source
        .and_then(|object| object.get(key))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn replacement_timestamp(source: Option<&Map<String, Value>>, key: &str) -> u64 {
    source
        .and_then(|object| object.get(key))
        .and_then(Value::as_u64)
        .unwrap_or_default()
}

fn parse_tag_patch(updates: &Map<String, Value>) -> TagPatch {
    TagPatch {
        name: string_patch(updates.get("name")),
        icon: string_patch(updates.get("icon")),
        color: string_patch(updates.get("color")),
        description: string_patch(updates.get("description")),
        defaults: updates
            .get("defaults")
            .and_then(Value::as_object)
            .map(parse_defaults_patch)
            .unwrap_or_default(),
    }
}

fn parse_defaults_patch(updates: &Map<String, Value>) -> TagDefaultsPatch {
    TagDefaultsPatch {
        summary_template_id: string_patch(updates.get("summaryTemplateId")),
        translation_language: string_patch(updates.get("translationLanguage")),
        polish_preset_id: string_patch(updates.get("polishPresetId")),
        polish_scenario: string_patch(updates.get("polishScenario")),
        polish_context: string_patch(updates.get("polishContext")),
        export_file_name_prefix: string_patch(updates.get("exportFileNamePrefix")),
        enabled_text_replacement_set_ids: string_array_patch(
            updates.get("enabledTextReplacementSetIds"),
        ),
        enabled_hotword_set_ids: string_array_patch(updates.get("enabledHotwordSetIds")),
        enabled_polish_keyword_set_ids: string_array_patch(
            updates.get("enabledPolishKeywordSetIds"),
        ),
        enabled_speaker_profile_ids: string_array_patch(updates.get("enabledSpeakerProfileIds")),
    }
}

fn string_patch(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(ToOwned::to_owned)
}

fn string_array_patch(value: Option<&Value>) -> Option<Vec<String>> {
    Some(
        value?
            .as_array()?
            .iter()
            .filter_map(Value::as_str)
            .map(ToOwned::to_owned)
            .collect(),
    )
}
