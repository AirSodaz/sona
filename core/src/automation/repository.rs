use serde::{Deserialize, Serialize};

use super::AutomationError;

#[cfg(feature = "specta")]
use specta::Type;

fn default_preset_id() -> String {
    "custom".to_string()
}

fn default_polish_preset_id() -> String {
    "general".to_string()
}

fn default_translation_language() -> String {
    "en".to_string()
}

fn default_export_format() -> String {
    "txt".to_string()
}

fn default_export_mode() -> String {
    "original".to_string()
}

fn default_processed_status() -> String {
    "complete".to_string()
}

fn default_save_history() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuleInputStageConfig {
    #[serde(default)]
    pub auto_polish: bool,
    #[serde(default = "default_polish_preset_id")]
    pub polish_preset_id: String,
    #[serde(default)]
    pub auto_translate: bool,
    #[serde(default = "default_translation_language")]
    pub translation_language: String,
    #[serde(default)]
    pub export_enabled: bool,
}

impl Default for AutomationRuleInputStageConfig {
    fn default() -> Self {
        Self {
            auto_polish: false,
            polish_preset_id: default_polish_preset_id(),
            auto_translate: false,
            translation_language: default_translation_language(),
            export_enabled: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuleInputExportConfig {
    #[serde(default)]
    pub directory: String,
    #[serde(default = "default_export_format")]
    pub format: String,
    #[serde(default = "default_export_mode")]
    pub mode: String,
    #[serde(default)]
    pub prefix: String,
}

impl Default for AutomationRuleInputExportConfig {
    fn default() -> Self {
        Self {
            directory: String::new(),
            format: default_export_format(),
            mode: default_export_mode(),
            prefix: String::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuleInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_save_history")]
    pub save_history: bool,
    #[serde(default)]
    pub tag_ids: Vec<String>,
    #[serde(default = "default_preset_id")]
    pub preset_id: String,
    #[serde(default)]
    pub watch_directory: String,
    #[serde(default)]
    pub recursive: bool,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub stage_config: AutomationRuleInputStageConfig,
    #[serde(default)]
    pub export_config: AutomationRuleInputExportConfig,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub created_at: i64,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub updated_at: i64,
}

impl Default for AutomationRuleInput {
    fn default() -> Self {
        Self {
            id: None,
            name: String::new(),
            save_history: true,
            tag_ids: Vec::new(),
            preset_id: default_preset_id(),
            watch_directory: String::new(),
            recursive: false,
            enabled: false,
            stage_config: AutomationRuleInputStageConfig::default(),
            export_config: AutomationRuleInputExportConfig::default(),
            created_at: 0,
            updated_at: 0,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationProcessedInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default)]
    pub rule_id: String,
    #[serde(default)]
    pub file_path: String,
    #[serde(default)]
    pub source_fingerprint: String,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub size: i64,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub mtime_ms: i64,
    #[serde(default = "default_processed_status")]
    pub status: String,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub processed_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

impl Default for AutomationProcessedInput {
    fn default() -> Self {
        Self {
            id: None,
            rule_id: String::new(),
            file_path: String::new(),
            source_fingerprint: String::new(),
            size: 0,
            mtime_ms: 0,
            status: default_processed_status(),
            processed_at: 0,
            history_id: None,
            export_path: None,
            error_message: None,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationRepositoryInput {
    #[serde(default)]
    pub rules: Vec<AutomationRuleInput>,
    #[serde(default)]
    pub processed_entries: Vec<AutomationProcessedInput>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuleRecordStageConfig {
    pub auto_polish: bool,
    pub polish_preset_id: String,
    pub auto_translate: bool,
    pub translation_language: String,
    pub export_enabled: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuleRecordExportConfig {
    pub directory: String,
    pub format: String,
    pub mode: String,
    pub prefix: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuleRecord {
    pub id: String,
    pub name: String,
    pub save_history: bool,
    pub tag_ids: Vec<String>,
    pub preset_id: String,
    pub watch_directory: String,
    pub recursive: bool,
    pub enabled: bool,
    pub stage_config: AutomationRuleRecordStageConfig,
    pub export_config: AutomationRuleRecordExportConfig,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub created_at: i64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub updated_at: i64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationProcessedRecord {
    pub id: String,
    pub rule_id: String,
    pub file_path: String,
    pub source_fingerprint: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub size: i64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub mtime_ms: i64,
    pub status: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub processed_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub export_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationRepositoryState {
    pub rules: Vec<AutomationRuleRecord>,
    pub processed_entries: Vec<AutomationProcessedRecord>,
}

pub trait AutomationStore: Send + Sync {
    fn load_state(&self) -> Result<AutomationRepositoryState, AutomationError>;
    fn replace_rules(&self, rules: &[AutomationRuleRecord]) -> Result<(), AutomationError>;
    fn replace_processed_entries(
        &self,
        entries: &[AutomationProcessedRecord],
    ) -> Result<(), AutomationError>;
    fn replace_state(&self, state: &AutomationRepositoryState) -> Result<(), AutomationError>;
}
