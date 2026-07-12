use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuleRecordStageConfig {
    pub auto_polish: bool,
    pub polish_preset_id: String,
    pub auto_translate: bool,
    pub translation_language: String,
    pub export_enabled: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuleRecordExportConfig {
    pub directory: String,
    pub format: String,
    pub mode: String,
    pub prefix: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuleRecord {
    pub id: String,
    pub name: String,
    pub project_id: String,
    pub preset_id: String,
    pub watch_directory: String,
    pub recursive: bool,
    pub enabled: bool,
    pub stage_config: AutomationRuleRecordStageConfig,
    pub export_config: AutomationRuleRecordExportConfig,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationProcessedRecord {
    pub id: String,
    pub rule_id: String,
    pub file_path: String,
    pub source_fingerprint: String,
    pub size: i64,
    pub mtime_ms: i64,
    pub status: String,
    pub processed_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub export_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRepositoryState {
    pub rules: Vec<AutomationRuleRecord>,
    pub processed_entries: Vec<AutomationProcessedRecord>,
}

pub trait AutomationStore: Send + Sync {
    fn load_state(&self) -> Result<AutomationRepositoryState, String>;
    fn replace_rules(&self, rules: &[AutomationRuleRecord]) -> Result<(), String>;
    fn replace_processed_entries(
        &self,
        entries: &[AutomationProcessedRecord],
    ) -> Result<(), String>;
    fn replace_state(&self, state: &AutomationRepositoryState) -> Result<(), String>;
}
