use super::repository::{
    AutomationProcessedInput, AutomationProcessedRecord, AutomationProfileInput,
    AutomationProfileRecord, AutomationRepositoryInput, AutomationRepositoryState,
    AutomationRuleInput, AutomationRuleRecord, AutomationRuleRecordExportConfig,
    AutomationRuleRecordStageConfig, AutomationStore,
};
use super::{
    AutomationError, AutomationRule, AutomationRuleActivationEnvironment,
    AutomationRuleValidationResult, normalize_automation_path, resolve_batch_model_path,
    validate_rule_activation,
};
use serde_json::{Value, json};

use crate::ports::fs::FileSystemError;

pub trait AutomationIdGenerator: Send + Sync {
    fn generate_id(&self) -> String;
}

pub trait AutomationFileSystem: Send + Sync {
    fn path_exists(&self, path: &str) -> Result<bool, FileSystemError>;
    fn create_dir_all(&self, path: &str) -> Result<(), FileSystemError>;
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

    pub fn replace_profiles(
        &self,
        profiles: Vec<AutomationProfileInput>,
    ) -> Result<(), AutomationError> {
        let profiles = profiles
            .into_iter()
            .map(|profile| normalize_profile_record(profile, self.ids))
            .collect::<Vec<_>>();
        self.store.replace_profiles(&profiles)
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
        let profiles = input
            .profiles
            .into_iter()
            .map(|profile| normalize_profile_record(profile, self.ids))
            .collect();
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
            profiles,
            rules,
            processed_entries,
        })
    }
}

fn normalize_profile_record(
    input: AutomationProfileInput,
    ids: &dyn AutomationIdGenerator,
) -> AutomationProfileRecord {
    AutomationProfileRecord {
        id: input.id.unwrap_or_else(|| ids.generate_id()),
        name: input.name,
        translation_language: input.translation_language,
        polish_preset_id: input.polish_preset_id,
        summary_template_id: input.summary_template_id,
        enabled_text_replacement_set_ids: input.enabled_text_replacement_set_ids,
        enabled_hotword_set_ids: input.enabled_hotword_set_ids,
        enabled_polish_keyword_set_ids: input.enabled_polish_keyword_set_ids,
        enabled_speaker_profile_ids: input.enabled_speaker_profile_ids,
        created_at: input.created_at,
        updated_at: input.updated_at,
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
    ) -> Result<AutomationRuleValidationResult, AutomationError> {
        let safe_preconditions = has_safe_path_preconditions(rule, tags);
        let watch_directory = rule.watch_directory.trim();
        let watch_directory_exists = if safe_preconditions {
            self.fs.path_exists(watch_directory)?
        } else {
            false
        };
        let export_directory_ready =
            if should_prepare_export_directory(rule, tags, watch_directory_exists) {
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
            tags,
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
    AutomationRuleRecord {
        id: input.id.unwrap_or_else(|| ids.generate_id()),
        name: input.name,
        kind: input.kind,
        priority: input.priority,
        profile_id: input.profile_id,
        profile_source: input.profile_source,
        save_history: input.save_history,
        tag_ids: input.tag_ids,
        preset_id: input.preset_id,
        watch_directory: input.watch_directory,
        recursive: input.recursive,
        enabled: input.enabled,
        actions: input.actions,
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
        migration_notice: input.migration_notice,
    }
}

fn normalize_processed_record(
    input: AutomationProcessedInput,
    ids: &dyn AutomationIdGenerator,
) -> AutomationProcessedRecord {
    AutomationProcessedRecord {
        id: input.id.unwrap_or_else(|| ids.generate_id()),
        rule_id: input.rule_id,
        kind: input.kind,
        input_version: input.input_version,
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

pub fn resolve_tag_rule<'a>(
    rules: &'a [AutomationRuleRecord],
    tag_ids: &[String],
) -> Option<&'a AutomationRuleRecord> {
    rules
        .iter()
        .filter(|rule| {
            rule.enabled
                && rule.kind == "tag"
                && rule
                    .tag_ids
                    .iter()
                    .any(|rule_tag_id| tag_ids.iter().any(|tag_id| tag_id == rule_tag_id))
        })
        .min_by_key(|rule| (std::cmp::Reverse(rule.priority), rule.id.as_str()))
}

pub fn resolve_rule_profile<'a>(
    rule: Option<&AutomationRuleRecord>,
    profiles: &'a [AutomationProfileRecord],
) -> Option<&'a AutomationProfileRecord> {
    rule.and_then(|rule| rule.profile_id.as_deref())
        .and_then(|profile_id| profiles.iter().find(|profile| profile.id == profile_id))
}

pub fn apply_profile_to_config(
    global_config: Value,
    profile: Option<&AutomationProfileRecord>,
) -> Value {
    let Some(profile) = profile else {
        return global_config;
    };
    let Some(mut config) = global_config.as_object().cloned() else {
        return global_config;
    };

    config.insert(
        "translationLanguage".to_string(),
        Value::String(profile.translation_language.clone()),
    );
    config.insert(
        "polishPresetId".to_string(),
        Value::String(profile.polish_preset_id.clone()),
    );
    config.insert(
        "summaryTemplateId".to_string(),
        Value::String(profile.summary_template_id.clone()),
    );

    for (field, ids) in [
        (
            "textReplacementSets",
            &profile.enabled_text_replacement_set_ids,
        ),
        ("hotwordSets", &profile.enabled_hotword_set_ids),
        ("polishKeywordSets", &profile.enabled_polish_keyword_set_ids),
        ("speakerProfiles", &profile.enabled_speaker_profile_ids),
    ] {
        let enabled_ids = ids
            .iter()
            .map(String::as_str)
            .collect::<std::collections::HashSet<_>>();
        let values = config
            .get(field)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|mut value| {
                if let Some(object) = value.as_object_mut() {
                    let enabled = object
                        .get("id")
                        .and_then(Value::as_str)
                        .is_some_and(|id| enabled_ids.contains(id));
                    object.insert("enabled".to_string(), json!(enabled));
                }
                value
            })
            .collect();
        config.insert(field.to_string(), Value::Array(values));
    }

    Value::Object(config)
}
