use serde::{Deserialize, Serialize};

pub use sona_sqlite::automation::AutomationRepositoryState;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRule {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub project_id: String,
    #[serde(default)]
    pub watch_directory: String,
    #[serde(default)]
    pub stage_config: AutomationRuleStageConfig,
    #[serde(default)]
    pub export_config: AutomationRuleExportConfig,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuleStageConfig {
    #[serde(default)]
    pub auto_polish: bool,
    #[serde(default)]
    pub auto_translate: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuleExportConfig {
    #[serde(default)]
    pub directory: String,
    #[serde(default = "default_export_mode")]
    pub mode: String,
}

impl Default for AutomationRuleExportConfig {
    fn default() -> Self {
        Self {
            directory: String::new(),
            mode: default_export_mode(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuleValidationResult {
    pub valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

fn default_export_mode() -> String {
    "original".to_string()
}
