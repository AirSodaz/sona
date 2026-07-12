use serde_json::Value;

use super::repository::{
    AutomationProcessedRecord, AutomationRepositoryState, AutomationRuleRecord,
    AutomationRuleRecordExportConfig, AutomationRuleRecordStageConfig, AutomationStore,
};
use super::{
    AutomationRule, AutomationRuleActivationEnvironment, AutomationRuleValidationResult,
    is_virtual_automation_project, normalize_automation_path, resolve_batch_model_path,
    validate_rule_activation,
};

pub trait AutomationIdGenerator: Send + Sync {
    fn generate_id(&self) -> String;
}

pub trait AutomationFileSystem: Send + Sync {
    fn path_exists(&self, path: &str) -> bool;
    fn create_dir_all(&self, path: &str) -> bool;
}

pub struct AutomationRepositoryService<'a> {
    store: &'a dyn AutomationStore,
    ids: &'a dyn AutomationIdGenerator,
}

impl<'a> AutomationRepositoryService<'a> {
    pub fn new(store: &'a dyn AutomationStore, ids: &'a dyn AutomationIdGenerator) -> Self {
        Self { store, ids }
    }

    pub fn load_state(&self) -> Result<AutomationRepositoryState, String> {
        self.store.load_state()
    }

    pub fn replace_rules_json(&self, rules: Vec<Value>) -> Result<(), String> {
        let rules = rules
            .into_iter()
            .map(|rule| normalize_rule_record(&rule, self.ids))
            .collect::<Vec<_>>();
        self.store.replace_rules(&rules)
    }

    pub fn replace_processed_entries_json(&self, entries: Vec<Value>) -> Result<(), String> {
        let entries = entries
            .into_iter()
            .map(|entry| normalize_processed_record(&entry, self.ids))
            .collect::<Vec<_>>();
        self.store.replace_processed_entries(&entries)
    }

    pub fn replace_state_json(
        &self,
        rules: Vec<Value>,
        processed_entries: Vec<Value>,
    ) -> Result<(), String> {
        let rules = rules
            .into_iter()
            .map(|rule| normalize_rule_record(&rule, self.ids))
            .collect();
        let processed_entries = processed_entries
            .into_iter()
            .map(|entry| normalize_processed_record(&entry, self.ids))
            .collect();
        self.store.replace_state(&AutomationRepositoryState {
            rules,
            processed_entries,
        })
    }
}

pub struct AutomationValidationService<'a> {
    fs: &'a dyn AutomationFileSystem,
}

impl<'a> AutomationValidationService<'a> {
    pub fn new(fs: &'a dyn AutomationFileSystem) -> Self {
        Self { fs }
    }

    pub fn validate_rule_activation(
        &self,
        rule: &AutomationRule,
        global_config: &Value,
        project: Option<&Value>,
    ) -> AutomationRuleValidationResult {
        let safe_preconditions = has_safe_path_preconditions(rule, project);
        let watch_directory = rule.watch_directory.trim();
        let watch_directory_exists = safe_preconditions && self.fs.path_exists(watch_directory);
        let export_directory_ready =
            if should_prepare_export_directory(rule, project, watch_directory_exists) {
                let directory = rule.export_config.directory.trim();
                self.fs.create_dir_all(directory)
            } else {
                false
            };
        let batch_model_path_exists = if safe_preconditions && watch_directory_exists {
            resolve_batch_model_path(global_config)
                .map(|path| self.fs.path_exists(&path))
                .unwrap_or(false)
        } else {
            false
        };

        validate_rule_activation(
            rule,
            global_config,
            project,
            AutomationRuleActivationEnvironment {
                watch_directory_exists,
                export_directory_ready,
                batch_model_path_exists,
            },
        )
    }
}

fn normalize_rule_record(value: &Value, ids: &dyn AutomationIdGenerator) -> AutomationRuleRecord {
    let stage = value.get("stageConfig").unwrap_or(&Value::Null);
    let export = value.get("exportConfig").unwrap_or(&Value::Null);
    AutomationRuleRecord {
        id: value
            .get("id")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| ids.generate_id()),
        name: string_field(value, "name", ""),
        project_id: string_field(value, "projectId", ""),
        preset_id: string_field(value, "presetId", "custom"),
        watch_directory: string_field(value, "watchDirectory", ""),
        recursive: bool_field(value, "recursive"),
        enabled: bool_field(value, "enabled"),
        stage_config: AutomationRuleRecordStageConfig {
            auto_polish: bool_field(stage, "autoPolish"),
            polish_preset_id: string_field(stage, "polishPresetId", "general"),
            auto_translate: bool_field(stage, "autoTranslate"),
            translation_language: string_field(stage, "translationLanguage", "en"),
            export_enabled: bool_field(stage, "exportEnabled"),
        },
        export_config: AutomationRuleRecordExportConfig {
            directory: string_field(export, "directory", ""),
            format: string_field(export, "format", "txt"),
            mode: string_field(export, "mode", "original"),
            prefix: string_field(export, "prefix", ""),
        },
        created_at: integer_field(value, "createdAt"),
        updated_at: integer_field(value, "updatedAt"),
    }
}

fn normalize_processed_record(
    value: &Value,
    ids: &dyn AutomationIdGenerator,
) -> AutomationProcessedRecord {
    AutomationProcessedRecord {
        id: value
            .get("id")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| ids.generate_id()),
        rule_id: string_field(value, "ruleId", ""),
        file_path: string_field(value, "filePath", ""),
        source_fingerprint: string_field(value, "sourceFingerprint", ""),
        size: integer_field(value, "size"),
        mtime_ms: integer_field(value, "mtimeMs"),
        status: string_field(value, "status", "complete"),
        processed_at: integer_field(value, "processedAt"),
        history_id: optional_string_field(value, "historyId"),
        export_path: optional_string_field(value, "exportPath"),
        error_message: optional_string_field(value, "errorMessage"),
    }
}

fn string_field(value: &Value, key: &str, default: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_string()
}

fn bool_field(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn integer_field(value: &Value, key: &str) -> i64 {
    value.get(key).and_then(Value::as_i64).unwrap_or(0)
}

fn optional_string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn should_prepare_export_directory(
    rule: &AutomationRule,
    project: Option<&Value>,
    watch_directory_exists: bool,
) -> bool {
    has_safe_path_preconditions(rule, project) && watch_directory_exists
}

fn has_safe_path_preconditions(rule: &AutomationRule, project: Option<&Value>) -> bool {
    let watch_directory = rule.watch_directory.trim();
    let export_directory = rule.export_config.directory.trim();
    !rule.name.trim().is_empty()
        && (project.is_some() || is_virtual_automation_project(&rule.project_id))
        && !watch_directory.is_empty()
        && !export_directory.is_empty()
        && normalize_automation_path(watch_directory) != normalize_automation_path(export_directory)
}
