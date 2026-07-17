use super::repository::{
    AutomationProcessedInput, AutomationProcessedRecord, AutomationRepositoryInput,
    AutomationRepositoryState, AutomationRuleInput, AutomationRuleRecord,
    AutomationRuleRecordExportConfig, AutomationRuleRecordStageConfig, AutomationStore,
};
use super::{
    AutomationError, AutomationRule, AutomationRuleActivationEnvironment,
    AutomationRuleValidationResult, normalize_automation_path, resolve_batch_model_path,
    validate_rule_activation,
};
use serde_json::Value;

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

    pub fn load_state(&self) -> Result<AutomationRepositoryState, AutomationError> {
        self.store.load_state()
    }

    pub fn replace_rules(&self, rules: Vec<AutomationRuleInput>) -> Result<(), AutomationError> {
        let rules = rules
            .into_iter()
            .map(|rule| normalize_rule_record(rule, self.ids))
            .collect::<Vec<_>>();
        self.store.replace_rules(&rules)
    }

    pub fn replace_processed_entries(
        &self,
        entries: Vec<AutomationProcessedInput>,
    ) -> Result<(), AutomationError> {
        let entries = entries
            .into_iter()
            .map(|entry| normalize_processed_record(entry, self.ids))
            .collect::<Vec<_>>();
        self.store.replace_processed_entries(&entries)
    }

    pub fn replace_state(&self, input: AutomationRepositoryInput) -> Result<(), AutomationError> {
        let rules = input
            .rules
            .into_iter()
            .map(|rule| normalize_rule_record(rule, self.ids))
            .collect();
        let processed_entries = input
            .processed_entries
            .into_iter()
            .map(|entry| normalize_processed_record(entry, self.ids))
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
        tags: &[Value],
    ) -> AutomationRuleValidationResult {
        let safe_preconditions = has_safe_path_preconditions(rule, tags);
        let watch_directory = rule.watch_directory.trim();
        let watch_directory_exists = safe_preconditions && self.fs.path_exists(watch_directory);
        let export_directory_ready =
            if should_prepare_export_directory(rule, tags, watch_directory_exists) {
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
            tags,
            AutomationRuleActivationEnvironment {
                watch_directory_exists,
                export_directory_ready,
                batch_model_path_exists,
            },
        )
    }
}

fn normalize_rule_record(
    input: AutomationRuleInput,
    ids: &dyn AutomationIdGenerator,
) -> AutomationRuleRecord {
    AutomationRuleRecord {
        id: input.id.unwrap_or_else(|| ids.generate_id()),
        name: input.name,
        save_history: input.save_history,
        tag_ids: input.tag_ids,
        preset_id: input.preset_id,
        watch_directory: input.watch_directory,
        recursive: input.recursive,
        enabled: input.enabled,
        stage_config: AutomationRuleRecordStageConfig {
            auto_polish: input.stage_config.auto_polish,
            polish_preset_id: input.stage_config.polish_preset_id,
            auto_translate: input.stage_config.auto_translate,
            translation_language: input.stage_config.translation_language,
            export_enabled: input.stage_config.export_enabled,
        },
        export_config: AutomationRuleRecordExportConfig {
            directory: input.export_config.directory,
            format: input.export_config.format,
            mode: input.export_config.mode,
            prefix: input.export_config.prefix,
        },
        created_at: input.created_at,
        updated_at: input.updated_at,
    }
}

fn normalize_processed_record(
    input: AutomationProcessedInput,
    ids: &dyn AutomationIdGenerator,
) -> AutomationProcessedRecord {
    AutomationProcessedRecord {
        id: input.id.unwrap_or_else(|| ids.generate_id()),
        rule_id: input.rule_id,
        file_path: input.file_path,
        source_fingerprint: input.source_fingerprint,
        size: input.size,
        mtime_ms: input.mtime_ms,
        status: input.status,
        processed_at: input.processed_at,
        history_id: input.history_id,
        export_path: input.export_path,
        error_message: input.error_message,
    }
}

fn should_prepare_export_directory(
    rule: &AutomationRule,
    tags: &[Value],
    watch_directory_exists: bool,
) -> bool {
    has_safe_path_preconditions(rule, tags) && watch_directory_exists
}

fn has_safe_path_preconditions(rule: &AutomationRule, tags: &[Value]) -> bool {
    let watch_directory = rule.watch_directory.trim();
    let export_directory = rule.export_config.directory.trim();
    !rule.name.trim().is_empty()
        && (!rule.save_history
            || rule.tag_ids.iter().all(|tag_id| {
                tags.iter()
                    .any(|tag| tag.get("id").and_then(Value::as_str) == Some(tag_id.as_str()))
            }))
        && !watch_directory.is_empty()
        && !export_directory.is_empty()
        && normalize_automation_path(watch_directory) != normalize_automation_path(export_directory)
}
