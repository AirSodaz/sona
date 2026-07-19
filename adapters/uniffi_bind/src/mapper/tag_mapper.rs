use sona_core::tag::{
    TagCreateInput, TagDefaults, TagDefaultsInput, TagDefaultsPatch, TagRecord,
    TagRepositorySnapshot, TagUpdateInput,
};

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiTagDefaultsInputV1 {
    pub summary_template_id: Option<String>,
    pub summary_template: Option<String>,
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

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiTagCreateInputV1 {
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub defaults: FfiTagDefaultsInputV1,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiTagDefaultsV1 {
    pub summary_template_id: String,
    pub translation_language: String,
    pub polish_preset_id: String,
    pub polish_scenario: Option<String>,
    pub polish_context: Option<String>,
    pub export_file_name_prefix: String,
    pub enabled_text_replacement_set_ids: Vec<String>,
    pub enabled_hotword_set_ids: Vec<String>,
    pub enabled_polish_keyword_set_ids: Vec<String>,
    pub enabled_speaker_profile_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiTagRecordV1 {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub color: String,
    pub sort_order: u64,
    pub created_at: u64,
    pub updated_at: u64,
    pub defaults: FfiTagDefaultsV1,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiTagDefaultsPatchV1 {
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

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiTagUpdateInputV1 {
    pub name: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
    pub defaults: Option<FfiTagDefaultsPatchV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiTagRepositorySnapshotV1 {
    pub tags: Vec<FfiTagRecordV1>,
    pub active_tag_id: Option<String>,
}

impl From<FfiTagDefaultsInputV1> for TagDefaultsInput {
    fn from(value: FfiTagDefaultsInputV1) -> Self {
        Self {
            summary_template_id: value.summary_template_id,
            summary_template: value.summary_template,
            translation_language: value.translation_language,
            polish_preset_id: value.polish_preset_id,
            polish_scenario: value.polish_scenario,
            polish_context: value.polish_context,
            export_file_name_prefix: value.export_file_name_prefix,
            enabled_text_replacement_set_ids: value.enabled_text_replacement_set_ids,
            enabled_hotword_set_ids: value.enabled_hotword_set_ids,
            enabled_polish_keyword_set_ids: value.enabled_polish_keyword_set_ids,
            enabled_speaker_profile_ids: value.enabled_speaker_profile_ids,
        }
    }
}

impl From<FfiTagCreateInputV1> for TagCreateInput {
    fn from(value: FfiTagCreateInputV1) -> Self {
        Self {
            name: value.name,
            description: value.description,
            icon: value.icon,
            color: value.color,
            defaults: value.defaults.into(),
        }
    }
}

impl From<TagDefaults> for FfiTagDefaultsV1 {
    fn from(value: TagDefaults) -> Self {
        Self {
            summary_template_id: value.summary_template_id,
            translation_language: value.translation_language,
            polish_preset_id: value.polish_preset_id,
            polish_scenario: value.polish_scenario,
            polish_context: value.polish_context,
            export_file_name_prefix: value.export_file_name_prefix,
            enabled_text_replacement_set_ids: value.enabled_text_replacement_set_ids,
            enabled_hotword_set_ids: value.enabled_hotword_set_ids,
            enabled_polish_keyword_set_ids: value.enabled_polish_keyword_set_ids,
            enabled_speaker_profile_ids: value.enabled_speaker_profile_ids,
        }
    }
}

impl From<FfiTagDefaultsV1> for TagDefaults {
    fn from(value: FfiTagDefaultsV1) -> Self {
        Self {
            summary_template_id: value.summary_template_id,
            translation_language: value.translation_language,
            polish_preset_id: value.polish_preset_id,
            polish_scenario: value.polish_scenario,
            polish_context: value.polish_context,
            export_file_name_prefix: value.export_file_name_prefix,
            enabled_text_replacement_set_ids: value.enabled_text_replacement_set_ids,
            enabled_hotword_set_ids: value.enabled_hotword_set_ids,
            enabled_polish_keyword_set_ids: value.enabled_polish_keyword_set_ids,
            enabled_speaker_profile_ids: value.enabled_speaker_profile_ids,
        }
    }
}

impl From<TagRecord> for FfiTagRecordV1 {
    fn from(value: TagRecord) -> Self {
        Self {
            id: value.id,
            name: value.name,
            description: value.description,
            icon: value.icon,
            color: value.color,
            sort_order: value.sort_order as u64,
            created_at: value.created_at,
            updated_at: value.updated_at,
            defaults: value.defaults.into(),
        }
    }
}

impl TryFrom<FfiTagRecordV1> for TagRecord {
    type Error = String;

    fn try_from(value: FfiTagRecordV1) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value.id,
            name: value.name,
            description: value.description,
            icon: value.icon,
            color: value.color,
            sort_order: usize::try_from(value.sort_order)
                .map_err(|_| format!("tag sort order {} is too large", value.sort_order))?,
            created_at: value.created_at,
            updated_at: value.updated_at,
            defaults: value.defaults.into(),
        })
    }
}

impl From<FfiTagDefaultsPatchV1> for TagDefaultsPatch {
    fn from(value: FfiTagDefaultsPatchV1) -> Self {
        Self {
            summary_template_id: value.summary_template_id,
            translation_language: value.translation_language,
            polish_preset_id: value.polish_preset_id,
            polish_scenario: value.polish_scenario,
            polish_context: value.polish_context,
            export_file_name_prefix: value.export_file_name_prefix,
            enabled_text_replacement_set_ids: value.enabled_text_replacement_set_ids,
            enabled_hotword_set_ids: value.enabled_hotword_set_ids,
            enabled_polish_keyword_set_ids: value.enabled_polish_keyword_set_ids,
            enabled_speaker_profile_ids: value.enabled_speaker_profile_ids,
        }
    }
}

impl From<FfiTagUpdateInputV1> for TagUpdateInput {
    fn from(value: FfiTagUpdateInputV1) -> Self {
        Self {
            name: value.name,
            icon: value.icon,
            color: value.color,
            description: value.description,
            defaults: value.defaults.map(Into::into),
        }
    }
}

impl From<TagRepositorySnapshot> for FfiTagRepositorySnapshotV1 {
    fn from(value: TagRepositorySnapshot) -> Self {
        Self {
            tags: value.tags.into_iter().map(Into::into).collect(),
            active_tag_id: value.active_tag_id,
        }
    }
}
