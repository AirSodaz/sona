use serde::{Deserialize, Serialize};
use serde_json::Value;

mod error;
mod repository;
mod service;

pub use error::TagError;
pub use repository::{
    ActiveTagSelection, TagPatch, TagRepositorySnapshot, TagStore, TagStoredState,
};
pub use service::{TagIdGenerator, TagRepositoryService};

#[derive(Clone, Debug, Default)]
pub struct TagListOptions {
    pub fallback_enabled_polish_keyword_set_ids: Vec<String>,
    pub fallback_enabled_speaker_profile_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TagCreateInput {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TagRecord {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub color: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub sort_order: usize,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub created_at: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub updated_at: u64,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default, rename_all = "camelCase")]
pub struct TagUpdateInput {
    pub name: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
}

pub fn normalize_tag_record_for_import(input: &Value) -> Result<Value, TagError> {
    normalize_tag_record_for_import_with_timestamp(input, 0)
}

pub fn normalize_tag_record_for_import_with_timestamp(
    input: &Value,
    fallback_timestamp: u64,
) -> Result<Value, TagError> {
    serde_json::to_value(normalize_tag_value_with_timestamp(
        input,
        fallback_timestamp,
    ))
    .map_err(TagError::Serialization)
}

pub fn normalize_tag_value(input: &Value) -> TagRecord {
    normalize_tag_value_with_timestamp(input, 0)
}

pub fn normalize_tag_value_with_timestamp(input: &Value, fallback_timestamp: u64) -> TagRecord {
    let source = input.as_object();
    let created_at = positive_millis(source.and_then(|object| object.get("createdAt")))
        .unwrap_or(fallback_timestamp);

    TagRecord {
        id: string_value(source.and_then(|object| object.get("id"))).unwrap_or_default(),
        name: non_empty_trimmed_string(source.and_then(|object| object.get("name")))
            .unwrap_or_else(|| "Untitled Tag".to_string()),
        description: string_value(source.and_then(|object| object.get("description")))
            .unwrap_or_default(),
        icon: string_value(source.and_then(|object| object.get("icon"))).unwrap_or_default(),
        color: string_value(source.and_then(|object| object.get("color"))).unwrap_or_default(),
        sort_order: non_negative_usize(source.and_then(|object| object.get("sortOrder")))
            .unwrap_or_default(),
        created_at,
        updated_at: positive_millis(source.and_then(|object| object.get("updatedAt")))
            .unwrap_or(created_at),
    }
}

pub fn positive_millis(value: Option<&Value>) -> Option<u64> {
    match value.and_then(Value::as_f64) {
        Some(value) if value.is_finite() && value > 0.0 => Some(value.round() as u64),
        _ => None,
    }
}

fn non_negative_usize(value: Option<&Value>) -> Option<usize> {
    value
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}

pub fn highest_priority_tag<'a>(
    tags: &'a [TagRecord],
    tag_ids: &[String],
) -> Option<&'a TagRecord> {
    tags.iter()
        .filter(|tag| tag_ids.iter().any(|tag_id| tag_id == &tag.id))
        .min_by_key(|tag| (tag.sort_order, tag.id.as_str()))
}

pub fn string_value(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(ToOwned::to_owned)
}

pub fn non_empty_trimmed_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub const ACTIVE_TAG_SETTINGS_KEY: &str = "sona-active-tag-id";
pub const LEGACY_ACTIVE_PROJECT_SETTINGS_KEY: &str = "sona-active-project-id";

pub fn active_tag_id_from_value(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .map(ToOwned::to_owned)
        .filter(|value| !value.is_empty())
}
