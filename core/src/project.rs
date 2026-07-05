use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default)]
pub struct ProjectListOptions {
    pub fallback_enabled_polish_keyword_set_ids: Vec<String>,
    pub fallback_enabled_speaker_profile_ids: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDefaultsInput {
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

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCreateInput {
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub defaults: ProjectDefaultsInput,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDefaults {
    pub summary_template_id: String,
    pub translation_language: String,
    pub polish_preset_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub polish_scenario: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub polish_context: Option<String>,
    pub export_file_name_prefix: String,
    pub enabled_text_replacement_set_ids: Vec<String>,
    pub enabled_hotword_set_ids: Vec<String>,
    pub enabled_polish_keyword_set_ids: Vec<String>,
    pub enabled_speaker_profile_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRecord {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub defaults: ProjectDefaults,
}
