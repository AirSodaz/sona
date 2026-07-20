use sona_core::automation::repository::{
    AutomationProcessedInput, AutomationProcessedRecord, AutomationProfileInput,
    AutomationProfileRecord, AutomationRepositoryInput, AutomationRepositoryState,
    AutomationRuleInput, AutomationRuleInputActions, AutomationRuleInputExportConfig,
    AutomationRuleInputStageConfig, AutomationRuleRecord, AutomationRuleRecordExportConfig,
    AutomationRuleRecordStageConfig,
};
use sona_core::automation::{
    AutomationRule, AutomationRuleExportConfig, AutomationRuleStageConfig,
    AutomationRuleValidationResult,
};

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiAutomationStageConfigV1 {
    pub auto_polish: bool,
    pub polish_preset_id: String,
    pub auto_translate: bool,
    pub translation_language: String,
    pub export_enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiAutomationExportConfigV1 {
    pub directory: String,
    pub format: String,
    pub mode: String,
    pub prefix: String,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiAutomationProfileInputV1 {
    pub id: Option<String>,
    pub name: String,
    pub translation_language: String,
    pub polish_preset_id: String,
    pub summary_template_id: String,
    pub enabled_text_replacement_set_ids: Vec<String>,
    pub enabled_hotword_set_ids: Vec<String>,
    pub enabled_polish_keyword_set_ids: Vec<String>,
    pub enabled_speaker_profile_ids: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiAutomationProfileRecordV1 {
    pub id: String,
    pub name: String,
    pub translation_language: String,
    pub polish_preset_id: String,
    pub summary_template_id: String,
    pub enabled_text_replacement_set_ids: Vec<String>,
    pub enabled_hotword_set_ids: Vec<String>,
    pub enabled_polish_keyword_set_ids: Vec<String>,
    pub enabled_speaker_profile_ids: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, uniffi::Record)]
pub struct FfiAutomationActionsV1 {
    pub auto_polish: bool,
    pub auto_translate: bool,
    pub auto_summary: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiAutomationRuleInputV1 {
    pub id: Option<String>,
    pub name: String,
    pub kind: String,
    pub priority: i64,
    pub profile_id: Option<String>,
    pub profile_source: String,
    pub save_history: bool,
    pub tag_ids: Vec<String>,
    pub preset_id: String,
    pub watch_directory: String,
    pub recursive: bool,
    pub enabled: bool,
    pub actions: FfiAutomationActionsV1,
    pub stage_config: FfiAutomationStageConfigV1,
    pub export_config: FfiAutomationExportConfigV1,
    pub created_at: i64,
    pub updated_at: i64,
    pub migration_notice: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiAutomationProcessedInputV1 {
    pub id: Option<String>,
    pub rule_id: String,
    pub kind: String,
    pub input_version: String,
    pub attempt: i64,
    pub file_path: String,
    pub source_fingerprint: String,
    pub size: i64,
    pub mtime_ms: i64,
    pub status: String,
    pub processed_at: i64,
    pub history_id: Option<String>,
    pub export_path: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiAutomationRepositoryInputV1 {
    pub profiles: Vec<FfiAutomationProfileInputV1>,
    pub rules: Vec<FfiAutomationRuleInputV1>,
    pub processed_entries: Vec<FfiAutomationProcessedInputV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiAutomationRuleRecordV1 {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub priority: i64,
    pub profile_id: Option<String>,
    pub profile_source: String,
    pub save_history: bool,
    pub tag_ids: Vec<String>,
    pub preset_id: String,
    pub watch_directory: String,
    pub recursive: bool,
    pub enabled: bool,
    pub actions: FfiAutomationActionsV1,
    pub stage_config: FfiAutomationStageConfigV1,
    pub export_config: FfiAutomationExportConfigV1,
    pub created_at: i64,
    pub updated_at: i64,
    pub migration_notice: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiAutomationProcessedRecordV1 {
    pub id: String,
    pub rule_id: String,
    pub kind: String,
    pub input_version: String,
    pub attempt: i64,
    pub file_path: String,
    pub source_fingerprint: String,
    pub size: i64,
    pub mtime_ms: i64,
    pub status: String,
    pub processed_at: i64,
    pub history_id: Option<String>,
    pub export_path: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiAutomationRepositoryStateV1 {
    pub profiles: Vec<FfiAutomationProfileRecordV1>,
    pub rules: Vec<FfiAutomationRuleRecordV1>,
    pub processed_entries: Vec<FfiAutomationProcessedRecordV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiAutomationValidationStageConfigV1 {
    pub auto_polish: bool,
    pub auto_translate: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiAutomationValidationExportConfigV1 {
    pub directory: String,
    pub mode: String,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiAutomationValidationRuleV1 {
    pub name: String,
    pub save_history: bool,
    pub tag_ids: Vec<String>,
    pub watch_directory: String,
    pub stage_config: FfiAutomationValidationStageConfigV1,
    pub export_config: FfiAutomationValidationExportConfigV1,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiAutomationTagReferenceV1 {
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiAutomationRuleValidationResultV1 {
    pub valid: bool,
    pub code: Option<String>,
    pub message: Option<String>,
}

impl From<FfiAutomationRuleInputV1> for AutomationRuleInput {
    fn from(value: FfiAutomationRuleInputV1) -> Self {
        Self {
            id: value.id,
            name: value.name,
            kind: value.kind,
            priority: value.priority,
            profile_id: value.profile_id,
            profile_source: value.profile_source,
            save_history: value.save_history,
            tag_ids: value.tag_ids,
            preset_id: value.preset_id,
            watch_directory: value.watch_directory,
            recursive: value.recursive,
            enabled: value.enabled,
            actions: value.actions.into(),
            stage_config: value.stage_config.into(),
            export_config: value.export_config.into(),
            created_at: value.created_at,
            updated_at: value.updated_at,
            migration_notice: value.migration_notice,
        }
    }
}

impl From<FfiAutomationActionsV1> for AutomationRuleInputActions {
    fn from(value: FfiAutomationActionsV1) -> Self {
        Self {
            auto_polish: value.auto_polish,
            auto_translate: value.auto_translate,
            auto_summary: value.auto_summary,
        }
    }
}

impl From<AutomationRuleInputActions> for FfiAutomationActionsV1 {
    fn from(value: AutomationRuleInputActions) -> Self {
        Self {
            auto_polish: value.auto_polish,
            auto_translate: value.auto_translate,
            auto_summary: value.auto_summary,
        }
    }
}

impl From<FfiAutomationProfileInputV1> for AutomationProfileInput {
    fn from(value: FfiAutomationProfileInputV1) -> Self {
        Self {
            id: value.id,
            name: value.name,
            translation_language: value.translation_language,
            polish_preset_id: value.polish_preset_id,
            summary_template_id: value.summary_template_id,
            enabled_text_replacement_set_ids: value.enabled_text_replacement_set_ids,
            enabled_hotword_set_ids: value.enabled_hotword_set_ids,
            enabled_polish_keyword_set_ids: value.enabled_polish_keyword_set_ids,
            enabled_speaker_profile_ids: value.enabled_speaker_profile_ids,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl From<AutomationProfileRecord> for FfiAutomationProfileRecordV1 {
    fn from(value: AutomationProfileRecord) -> Self {
        Self {
            id: value.id,
            name: value.name,
            translation_language: value.translation_language,
            polish_preset_id: value.polish_preset_id,
            summary_template_id: value.summary_template_id,
            enabled_text_replacement_set_ids: value.enabled_text_replacement_set_ids,
            enabled_hotword_set_ids: value.enabled_hotword_set_ids,
            enabled_polish_keyword_set_ids: value.enabled_polish_keyword_set_ids,
            enabled_speaker_profile_ids: value.enabled_speaker_profile_ids,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl From<FfiAutomationStageConfigV1> for AutomationRuleInputStageConfig {
    fn from(value: FfiAutomationStageConfigV1) -> Self {
        Self {
            auto_polish: value.auto_polish,
            polish_preset_id: value.polish_preset_id,
            auto_translate: value.auto_translate,
            translation_language: value.translation_language,
            export_enabled: value.export_enabled,
        }
    }
}

impl From<FfiAutomationExportConfigV1> for AutomationRuleInputExportConfig {
    fn from(value: FfiAutomationExportConfigV1) -> Self {
        Self {
            directory: value.directory,
            format: value.format,
            mode: value.mode,
            prefix: value.prefix,
        }
    }
}

impl From<FfiAutomationProcessedInputV1> for AutomationProcessedInput {
    fn from(value: FfiAutomationProcessedInputV1) -> Self {
        Self {
            id: value.id,
            rule_id: value.rule_id,
            kind: value.kind,
            input_version: value.input_version,
            attempt: value.attempt,
            file_path: value.file_path,
            source_fingerprint: value.source_fingerprint,
            size: value.size,
            mtime_ms: value.mtime_ms,
            status: value.status,
            processed_at: value.processed_at,
            history_id: value.history_id,
            export_path: value.export_path,
            error_message: value.error_message,
        }
    }
}

impl From<FfiAutomationRepositoryInputV1> for AutomationRepositoryInput {
    fn from(value: FfiAutomationRepositoryInputV1) -> Self {
        Self {
            profiles: value.profiles.into_iter().map(Into::into).collect(),
            rules: value.rules.into_iter().map(Into::into).collect(),
            processed_entries: value
                .processed_entries
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

impl From<AutomationRepositoryState> for FfiAutomationRepositoryStateV1 {
    fn from(value: AutomationRepositoryState) -> Self {
        Self {
            profiles: value.profiles.into_iter().map(Into::into).collect(),
            rules: value.rules.into_iter().map(Into::into).collect(),
            processed_entries: value
                .processed_entries
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

impl From<AutomationRuleRecord> for FfiAutomationRuleRecordV1 {
    fn from(value: AutomationRuleRecord) -> Self {
        Self {
            id: value.id,
            name: value.name,
            kind: value.kind,
            priority: value.priority,
            profile_id: value.profile_id,
            profile_source: value.profile_source,
            save_history: value.save_history,
            tag_ids: value.tag_ids,
            preset_id: value.preset_id,
            watch_directory: value.watch_directory,
            recursive: value.recursive,
            enabled: value.enabled,
            actions: value.actions.into(),
            stage_config: value.stage_config.into(),
            export_config: value.export_config.into(),
            created_at: value.created_at,
            updated_at: value.updated_at,
            migration_notice: value.migration_notice,
        }
    }
}

impl From<AutomationRuleRecordStageConfig> for FfiAutomationStageConfigV1 {
    fn from(value: AutomationRuleRecordStageConfig) -> Self {
        Self {
            auto_polish: value.auto_polish,
            polish_preset_id: value.polish_preset_id,
            auto_translate: value.auto_translate,
            translation_language: value.translation_language,
            export_enabled: value.export_enabled,
        }
    }
}

impl From<AutomationRuleRecordExportConfig> for FfiAutomationExportConfigV1 {
    fn from(value: AutomationRuleRecordExportConfig) -> Self {
        Self {
            directory: value.directory,
            format: value.format,
            mode: value.mode,
            prefix: value.prefix,
        }
    }
}

impl From<AutomationProcessedRecord> for FfiAutomationProcessedRecordV1 {
    fn from(value: AutomationProcessedRecord) -> Self {
        Self {
            id: value.id,
            rule_id: value.rule_id,
            kind: value.kind,
            input_version: value.input_version,
            attempt: value.attempt,
            file_path: value.file_path,
            source_fingerprint: value.source_fingerprint,
            size: value.size,
            mtime_ms: value.mtime_ms,
            status: value.status,
            processed_at: value.processed_at,
            history_id: value.history_id,
            export_path: value.export_path,
            error_message: value.error_message,
        }
    }
}

impl From<FfiAutomationValidationRuleV1> for AutomationRule {
    fn from(value: FfiAutomationValidationRuleV1) -> Self {
        Self {
            name: value.name,
            save_history: value.save_history,
            tag_ids: value.tag_ids,
            watch_directory: value.watch_directory,
            stage_config: AutomationRuleStageConfig {
                auto_polish: value.stage_config.auto_polish,
                auto_translate: value.stage_config.auto_translate,
            },
            export_config: AutomationRuleExportConfig {
                directory: value.export_config.directory,
                mode: value.export_config.mode,
            },
        }
    }
}

impl From<AutomationRuleValidationResult> for FfiAutomationRuleValidationResultV1 {
    fn from(value: AutomationRuleValidationResult) -> Self {
        Self {
            valid: value.valid,
            code: value.code,
            message: value.message,
        }
    }
}
