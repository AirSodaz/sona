use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::AppConfig;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default, rename_all = "camelCase")]
pub struct ProjectPipelineConfig {
    pub enabled: bool,
    pub auto_polish: bool,
    pub polish_preset_id: Option<String>,
    pub polish_prompt_override: Option<String>,
    pub auto_translate: bool,
    pub target_language: Option<String>,
    pub auto_summary: bool,
    pub summary_template_id: Option<String>,
    pub hotword_set_ids: Vec<String>,
    pub replacement_set_ids: Vec<String>,
    pub auto_export: bool,
    pub export_format: Option<String>,
    pub export_directory: Option<String>,
    pub export_file_name_prefix: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ProjectRecord {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub sort_order: usize,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub created_at: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub updated_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pipeline: Option<ProjectPipelineConfig>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default, rename_all = "camelCase")]
pub struct ProjectCreateInput {
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub pipeline: Option<ProjectPipelineConfig>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default, rename_all = "camelCase")]
pub struct ProjectUpdateInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub pipeline: Option<ProjectPipelineConfig>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct EffectivePipelineSnapshot {
    pub is_project_pipeline: bool,
    pub auto_polish: bool,
    pub polish_preset_id: Option<String>,
    pub polish_prompt_override: Option<String>,
    pub auto_translate: bool,
    pub target_language: Option<String>,
    pub auto_summary: bool,
    pub summary_template_id: Option<String>,
    pub hotword_set_ids: Vec<String>,
    pub replacement_set_ids: Vec<String>,
    pub auto_export: bool,
    pub export_format: Option<String>,
    pub export_directory: Option<String>,
    pub export_file_name_prefix: Option<String>,
}

fn global_snapshot(config: &AppConfig) -> EffectivePipelineSnapshot {
    EffectivePipelineSnapshot {
        is_project_pipeline: false,
        auto_polish: config.auto_polish.unwrap_or(false),
        polish_preset_id: config.polish_preset_id.clone(),
        polish_prompt_override: None,
        auto_translate: false,
        target_language: config.translation_language.clone(),
        auto_summary: config.summary_enabled.unwrap_or(false),
        summary_template_id: config.summary_template_id.clone(),
        hotword_set_ids: Vec::new(),
        replacement_set_ids: Vec::new(),
        auto_export: false,
        export_format: None,
        export_directory: None,
        export_file_name_prefix: None,
    }
}

pub fn resolve_item_pipeline(
    project_id: Option<&str>,
    projects: &[ProjectRecord],
    global_config: &AppConfig,
) -> EffectivePipelineSnapshot {
    let fallback = global_snapshot(global_config);
    let Some(project) = project_id.and_then(|id| projects.iter().find(|p| p.id == id)) else {
        return fallback;
    };
    let Some(pipeline) = project.pipeline.as_ref().filter(|p| p.enabled) else {
        return fallback;
    };
    EffectivePipelineSnapshot {
        is_project_pipeline: true,
        auto_polish: pipeline.auto_polish,
        polish_preset_id: pipeline
            .polish_preset_id
            .clone()
            .or(fallback.polish_preset_id),
        polish_prompt_override: pipeline.polish_prompt_override.clone(),
        auto_translate: pipeline.auto_translate,
        target_language: pipeline
            .target_language
            .clone()
            .or(fallback.target_language),
        auto_summary: pipeline.auto_summary,
        summary_template_id: pipeline
            .summary_template_id
            .clone()
            .or(fallback.summary_template_id),
        hotword_set_ids: pipeline.hotword_set_ids.clone(),
        replacement_set_ids: pipeline.replacement_set_ids.clone(),
        auto_export: pipeline.auto_export,
        export_format: pipeline.export_format.clone(),
        export_directory: pipeline.export_directory.clone(),
        export_file_name_prefix: pipeline.export_file_name_prefix.clone(),
    }
}

pub fn project_pipeline_from_json(
    value: &Value,
) -> Result<ProjectPipelineConfig, serde_json::Error> {
    serde_json::from_value(value.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> AppConfig {
        AppConfig {
            auto_polish: Some(true),
            polish_preset_id: Some("global".into()),
            translation_language: Some("zh".into()),
            summary_enabled: Some(true),
            summary_template_id: Some("default".into()),
            ..Default::default()
        }
    }
    fn project(pipeline: Option<ProjectPipelineConfig>) -> ProjectRecord {
        ProjectRecord {
            id: "p1".into(),
            name: "Project".into(),
            description: String::new(),
            icon: None,
            color: None,
            sort_order: 0,
            created_at: 0,
            updated_at: 0,
            pipeline,
        }
    }

    #[test]
    fn inbox_and_missing_project_use_global_defaults() {
        let projects = vec![project(None)];
        assert!(!resolve_item_pipeline(None, &projects, &config()).is_project_pipeline);
        assert_eq!(
            resolve_item_pipeline(Some("missing"), &projects, &config())
                .polish_preset_id
                .as_deref(),
            Some("global")
        );
    }

    #[test]
    fn enabled_project_pipeline_overrides_and_inherits() {
        let pipeline = ProjectPipelineConfig {
            enabled: true,
            auto_polish: false,
            auto_translate: true,
            target_language: Some("en".into()),
            hotword_set_ids: vec!["hot".into()],
            ..Default::default()
        };
        let snapshot = resolve_item_pipeline(Some("p1"), &[project(Some(pipeline))], &config());
        assert!(snapshot.is_project_pipeline);
        assert!(!snapshot.auto_polish);
        assert!(snapshot.auto_translate);
        assert_eq!(snapshot.target_language.as_deref(), Some("en"));
        assert_eq!(snapshot.summary_template_id.as_deref(), Some("default"));
    }
}
