use crate::file_utils::current_time_millis;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

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

pub const DEFAULT_SUMMARY_TEMPLATE_ID: &str = "general";
pub const DEFAULT_TRANSLATION_LANGUAGE: &str = "zh";
pub const DEFAULT_POLISH_PRESET_ID: &str = "general";

pub fn normalize_project_record_for_import(input: &Value) -> Result<Value, String> {
    let project = normalize_project_value(input, &ProjectListOptions::default());
    serde_json::to_value(project).map_err(|error| error.to_string())
}

pub fn normalize_project_value(input: &Value, options: &ProjectListOptions) -> ProjectRecord {
    let now = current_time_millis().unwrap_or(0);
    let source = input.as_object();
    let defaults = source
        .and_then(|object| object.get("defaults"))
        .and_then(Value::as_object);
    let created_at =
        positive_millis(source.and_then(|object| object.get("createdAt"))).unwrap_or(now);

    ProjectRecord {
        id: string_value(source.and_then(|object| object.get("id"))).unwrap_or_default(),
        name: non_empty_trimmed_string(source.and_then(|object| object.get("name")))
            .unwrap_or_else(|| "Untitled Project".to_string()),
        description: string_value(source.and_then(|object| object.get("description")))
            .unwrap_or_default(),
        icon: string_value(source.and_then(|object| object.get("icon"))).unwrap_or_default(),
        created_at,
        updated_at: positive_millis(source.and_then(|object| object.get("updatedAt")))
            .unwrap_or(created_at),
        defaults: normalize_defaults(defaults, options),
    }
}

pub fn normalize_defaults(
    source: Option<&Map<String, Value>>,
    options: &ProjectListOptions,
) -> ProjectDefaults {
    let polish_scenario = string_value(source.and_then(|object| object.get("polishScenario")));
    let polish_context = string_value(source.and_then(|object| object.get("polishContext")));
    let polish_preset_id = string_value(source.and_then(|object| object.get("polishPresetId")))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            if polish_scenario.as_deref().unwrap_or_default().is_empty()
                && polish_context.as_deref().unwrap_or_default().is_empty()
            {
                DEFAULT_POLISH_PRESET_ID.to_string()
            } else {
                String::new()
            }
        });

    ProjectDefaults {
        summary_template_id: non_empty_trimmed_string(
            source.and_then(|object| object.get("summaryTemplateId")),
        )
        .or_else(|| {
            non_empty_trimmed_string(source.and_then(|object| object.get("summaryTemplate")))
        })
        .unwrap_or_else(|| DEFAULT_SUMMARY_TEMPLATE_ID.to_string()),
        translation_language: string_value(
            source.and_then(|object| object.get("translationLanguage")),
        )
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_TRANSLATION_LANGUAGE.to_string()),
        polish_preset_id,
        polish_scenario,
        polish_context,
        export_file_name_prefix: string_value(
            source.and_then(|object| object.get("exportFileNamePrefix")),
        )
        .unwrap_or_default(),
        enabled_text_replacement_set_ids: string_array(
            source.and_then(|object| object.get("enabledTextReplacementSetIds")),
        )
        .unwrap_or_default(),
        enabled_hotword_set_ids: string_array(
            source.and_then(|object| object.get("enabledHotwordSetIds")),
        )
        .unwrap_or_default(),
        enabled_polish_keyword_set_ids: string_array(
            source.and_then(|object| object.get("enabledPolishKeywordSetIds")),
        )
        .unwrap_or_else(|| options.fallback_enabled_polish_keyword_set_ids.clone()),
        enabled_speaker_profile_ids: string_array(
            source.and_then(|object| object.get("enabledSpeakerProfileIds")),
        )
        .unwrap_or_else(|| options.fallback_enabled_speaker_profile_ids.clone()),
    }
}

pub fn positive_millis(value: Option<&Value>) -> Option<u64> {
    match value.and_then(Value::as_f64) {
        Some(value) if value.is_finite() && value > 0.0 => Some(value.round() as u64),
        _ => None,
    }
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

pub fn string_array(value: Option<&Value>) -> Option<Vec<String>> {
    Some(
        value?
            .as_array()?
            .iter()
            .filter_map(Value::as_str)
            .map(ToOwned::to_owned)
            .collect(),
    )
}
