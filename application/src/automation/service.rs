use serde_json::Value;
use sona_core::automation::repository::{
    AutomationProcessedInput, AutomationProcessedRecord, AutomationRepositoryInput,
    AutomationRepositoryState, AutomationRuleInput, AutomationRuleRecord,
    AutomationRuleRecordExportConfig, AutomationRuleRecordStageConfig, AutomationStore,
};
use sona_core::automation::{
    AutomationError, AutomationRule, AutomationRuleActivationEnvironment,
    AutomationRuleValidationResult, automation_project_exists,
    is_export_directory_overlapping_watch_directory, resolve_batch_model_path,
    validate_rule_activation,
};

use sona_core::automation::{AutomationFsPort, AutomationIdGenerator};

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
    fs: &'a dyn AutomationFsPort,
}

impl<'a> AutomationValidationService<'a> {
    pub fn new(fs: &'a dyn AutomationFsPort) -> Self {
        Self { fs }
    }

    pub fn validate_rule_activation(
        &self,
        rule: &AutomationRule,
        global_config: &Value,
        projects: &[Value],
    ) -> Result<AutomationRuleValidationResult, AutomationError> {
        let safe_preconditions = has_safe_path_preconditions(rule, projects);
        let watch_directory = rule.watch_directory.trim();
        let watch_directory_exists = if safe_preconditions {
            self.fs.path_exists(watch_directory)?
        } else {
            false
        };
        let export_directory_ready =
            if should_prepare_export_directory(rule, projects, watch_directory_exists) {
                let directory = rule.export_config.directory.trim();
                self.fs.create_dir_all(directory)?;
                true
            } else {
                false
            };
        let batch_model_path_exists = if safe_preconditions && watch_directory_exists {
            match resolve_batch_model_path(global_config) {
                Some(path) => self.fs.path_exists(&path)?,
                None => false,
            }
        } else {
            false
        };

        Ok(validate_rule_activation(
            rule,
            global_config,
            projects,
            AutomationRuleActivationEnvironment {
                watch_directory_exists,
                export_directory_ready,
                batch_model_path_exists,
            },
        ))
    }
}

fn normalize_rule_record(
    input: AutomationRuleInput,
    ids: &dyn AutomationIdGenerator,
) -> AutomationRuleRecord {
    let project_id = if input.save_history {
        input.project_id.trim().to_string()
    } else {
        String::new()
    };
    let (export_directory, export_prefix) = if input.save_history {
        (String::new(), String::new())
    } else {
        (input.export_config.directory, input.export_config.prefix)
    };
    AutomationRuleRecord {
        id: input.id.unwrap_or_else(|| ids.generate_id()),
        name: input.name,
        save_history: input.save_history,
        project_id,
        watch_directory: input.watch_directory,
        recursive: input.recursive,
        enabled: input.enabled,
        stage_config: AutomationRuleRecordStageConfig {
            auto_polish: input.stage_config.auto_polish,
            polish_preset_id: input.stage_config.polish_preset_id,
            auto_translate: input.stage_config.auto_translate,
            translation_language: input.stage_config.translation_language,
        },
        export_config: AutomationRuleRecordExportConfig {
            directory: export_directory,
            format: input.export_config.format,
            mode: input.export_config.mode,
            prefix: export_prefix,
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
        attempt: input.attempt.max(1),
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
    projects: &[Value],
    watch_directory_exists: bool,
) -> bool {
    !rule.save_history && has_safe_path_preconditions(rule, projects) && watch_directory_exists
}

fn has_safe_path_preconditions(rule: &AutomationRule, projects: &[Value]) -> bool {
    let watch_directory = rule.watch_directory.trim();
    if rule.name.trim().is_empty() || watch_directory.is_empty() {
        return false;
    }
    if rule.save_history {
        let project_id = rule.project_id.trim();
        return project_id.is_empty()
            || project_id == "inbox"
            || automation_project_exists(project_id, projects);
    }
    let export_directory = rule.export_config.directory.trim();
    !export_directory.is_empty()
        && !is_export_directory_overlapping_watch_directory(
            watch_directory,
            export_directory,
            rule.recursive,
        )
}
