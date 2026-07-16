use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::{BuiltinLlmProvider, LlmProvider};
use crate::ports::asr::{
    VOLCENGINE_DOUBAO_LEGACY_PROVIDER_KEY, VOLCENGINE_DOUBAO_PROVIDER_ID, online_asr_providers,
};

pub mod repository;
pub mod service;

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
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

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuleStageConfig {
    #[serde(default)]
    pub auto_polish: bool,
    #[serde(default)]
    pub auto_translate: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
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
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuleValidationResult {
    pub valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AutomationRuleActivationEnvironment {
    pub watch_directory_exists: bool,
    pub export_directory_ready: bool,
    pub batch_model_path_exists: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuntimeRuleConfig {
    pub rule_id: String,
    pub watch_directory: String,
    pub recursive: bool,
    pub exclude_directory: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub debounce_ms: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub stable_window_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuntimeReplaceResult {
    pub rule_id: String,
    pub started: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuntimeCandidatePayload {
    pub rule_id: String,
    pub file_path: String,
    pub source_fingerprint: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub size: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub mtime_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum AutomationRuntimePathCollectionOutcome {
    Candidate,
    Missing,
    Unsupported,
    Excluded,
    NotFile,
    Error,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AutomationRuntimePathCollectionResult {
    pub file_path: String,
    pub outcome: AutomationRuntimePathCollectionOutcome,
    pub candidate: Option<AutomationRuntimeCandidatePayload>,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AutomationRuntimePathMetadata {
    pub is_file: bool,
    pub size: u64,
    pub mtime_ms: u64,
}

const SUPPORTED_MEDIA_EXTENSIONS: &[&str] = &[
    ".wav", ".mp3", ".m4a", ".aiff", ".flac", ".ogg", ".wma", ".aac", ".opus", ".amr", ".mp4",
    ".webm", ".mov", ".mkv", ".avi", ".wmv", ".flv", ".3gp",
];

pub fn validate_rule_activation(
    rule: &AutomationRule,
    global_config: &Value,
    project: Option<&Value>,
    environment: AutomationRuleActivationEnvironment,
) -> AutomationRuleValidationResult {
    if rule.name.trim().is_empty() {
        return invalid_validation("automation.name_required", "Rule name is required.");
    }

    if project.is_none() && !is_virtual_automation_project(&rule.project_id) {
        return invalid_validation("automation.project_missing", "Select a target project.");
    }

    let watch_directory = rule.watch_directory.trim();
    if watch_directory.is_empty() {
        return invalid_validation(
            "automation.watch_directory_required",
            "Choose a watch directory.",
        );
    }

    let export_directory = rule.export_config.directory.trim();
    if export_directory.is_empty() {
        return invalid_validation(
            "automation.output_directory_required",
            "Choose an output directory.",
        );
    }

    if is_same_automation_path(watch_directory, export_directory) {
        return invalid_validation(
            "automation.same_directory",
            "Watch and output directories must be different.",
        );
    }

    if !environment.watch_directory_exists {
        return invalid_validation(
            "automation.watch_directory_missing",
            "The watch directory does not exist.",
        );
    }

    if !environment.export_directory_ready {
        return invalid_validation(
            "automation.output_directory_invalid",
            "The output directory could not be created.",
        );
    }

    if !is_batch_asr_configured(global_config, environment) {
        return invalid_validation(
            "automation.batch_model_missing",
            "A batch ASR model or online ASR credential is required before automation can run.",
        );
    }

    if rule.stage_config.auto_polish && !is_feature_llm_config_complete(global_config, "polish") {
        return invalid_validation(
            "automation.polish_model_missing",
            "A polish model is required for Auto-Polish.",
        );
    }

    if rule.stage_config.auto_translate
        && !is_feature_llm_config_complete(global_config, "translation")
    {
        return invalid_validation(
            "automation.translation_model_missing",
            "A translation model is required for Auto-Translate.",
        );
    }

    if (rule.export_config.mode == "translation" || rule.export_config.mode == "bilingual")
        && !rule.stage_config.auto_translate
    {
        return invalid_validation(
            "automation.translation_required",
            "Enable Auto-Translate before exporting translations.",
        );
    }

    valid_validation()
}

pub fn normalize_automation_path(path: &str) -> String {
    path.trim()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
}

pub fn is_virtual_automation_project(project_id: &str) -> bool {
    matches!(project_id, "inbox" | "none")
}

pub fn resolve_batch_model_path(global_config: &Value) -> Option<String> {
    global_config
        .get("asr")
        .and_then(|asr| asr.get("selections"))
        .and_then(|selections| selections.get("batch"))
        .and_then(|selection| string_field(selection, "modelPath"))
        .or_else(|| string_field(global_config, "offlineModelPath"))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub fn should_consider_runtime_candidate_path(
    rule: &AutomationRuntimeRuleConfig,
    file_path: &str,
) -> bool {
    is_supported_runtime_media_path(file_path)
        && is_runtime_path_within_watch_scope(rule, file_path)
        && !is_path_inside_runtime_directory(file_path, &rule.exclude_directory)
}

pub fn collect_runtime_rule_path_result(
    rule: &AutomationRuntimeRuleConfig,
    file_path: &str,
    metadata: Result<Option<AutomationRuntimePathMetadata>, String>,
) -> AutomationRuntimePathCollectionResult {
    if !is_supported_runtime_media_path(file_path) {
        return runtime_path_result(
            file_path,
            AutomationRuntimePathCollectionOutcome::Unsupported,
            None,
            None,
        );
    }

    if !is_runtime_path_within_watch_scope(rule, file_path)
        || is_path_inside_runtime_directory(file_path, &rule.exclude_directory)
    {
        return runtime_path_result(
            file_path,
            AutomationRuntimePathCollectionOutcome::Excluded,
            None,
            None,
        );
    }

    match metadata {
        Ok(Some(metadata)) if metadata.is_file => runtime_path_result(
            file_path,
            AutomationRuntimePathCollectionOutcome::Candidate,
            Some(runtime_candidate_payload(
                rule,
                file_path,
                metadata.size,
                metadata.mtime_ms,
            )),
            None,
        ),
        Ok(Some(_)) => runtime_path_result(
            file_path,
            AutomationRuntimePathCollectionOutcome::NotFile,
            None,
            None,
        ),
        Ok(None) => runtime_path_result(
            file_path,
            AutomationRuntimePathCollectionOutcome::Missing,
            None,
            None,
        ),
        Err(error) => runtime_path_result(
            file_path,
            AutomationRuntimePathCollectionOutcome::Error,
            None,
            Some(error),
        ),
    }
}

fn runtime_candidate_payload(
    rule: &AutomationRuntimeRuleConfig,
    file_path: &str,
    size: u64,
    mtime_ms: u64,
) -> AutomationRuntimeCandidatePayload {
    let normalized_path = normalize_automation_path(file_path);
    AutomationRuntimeCandidatePayload {
        rule_id: rule.rule_id.clone(),
        file_path: file_path.to_string(),
        source_fingerprint: format!("{normalized_path}::{size}::{mtime_ms}"),
        size,
        mtime_ms,
    }
}

fn runtime_path_result(
    file_path: &str,
    outcome: AutomationRuntimePathCollectionOutcome,
    candidate: Option<AutomationRuntimeCandidatePayload>,
    error: Option<String>,
) -> AutomationRuntimePathCollectionResult {
    AutomationRuntimePathCollectionResult {
        file_path: file_path.to_string(),
        outcome,
        candidate,
        error,
    }
}

fn is_supported_runtime_media_path(file_path: &str) -> bool {
    let normalized = file_path.trim().to_lowercase();
    SUPPORTED_MEDIA_EXTENSIONS
        .iter()
        .any(|extension| normalized.ends_with(extension))
}

fn is_path_inside_runtime_directory(file_path: &str, directory_path: &str) -> bool {
    if directory_path.trim().is_empty() {
        return false;
    }

    let normalized_file = normalize_automation_path(file_path);
    let normalized_directory = normalize_automation_path(directory_path);

    normalized_file == normalized_directory
        || normalized_file.starts_with(&format!("{}\\", normalized_directory))
}

fn is_runtime_path_within_watch_scope(rule: &AutomationRuntimeRuleConfig, file_path: &str) -> bool {
    let watch_directory = rule.watch_directory.trim();
    if watch_directory.is_empty() {
        return false;
    }

    if !is_path_inside_runtime_directory(file_path, watch_directory) {
        return false;
    }

    if rule.recursive {
        return true;
    }

    parent_path_string(file_path)
        .map(|parent| normalize_automation_path(&parent))
        .map(|parent| parent == normalize_automation_path(watch_directory))
        .unwrap_or(false)
}

fn parent_path_string(file_path: &str) -> Option<String> {
    let normalized = normalize_automation_path(file_path);
    normalized
        .rsplit_once('\\')
        .map(|(parent, _)| parent.to_string())
}

fn default_export_mode() -> String {
    "original".to_string()
}

fn is_same_automation_path(a: &str, b: &str) -> bool {
    normalize_automation_path(a) == normalize_automation_path(b)
}

fn invalid_validation(code: &str, message: &str) -> AutomationRuleValidationResult {
    AutomationRuleValidationResult {
        valid: false,
        code: Some(code.to_string()),
        message: Some(message.to_string()),
    }
}

fn valid_validation() -> AutomationRuleValidationResult {
    AutomationRuleValidationResult {
        valid: true,
        code: None,
        message: None,
    }
}

fn is_feature_llm_config_complete(global_config: &Value, feature: &str) -> bool {
    let settings = match global_config.get("llmSettings") {
        Some(Value::Object(settings)) => settings,
        _ => return false,
    };
    let selection_key = match feature {
        "polish" => "polishModelId",
        "translation" => "translationModelId",
        "summary" => "summaryModelId",
        _ => return false,
    };
    let model_id = match settings
        .get("selections")
        .and_then(|selections| string_field(selections, selection_key))
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(model_id) => model_id,
        None => return false,
    };
    let model_entry = match settings
        .get("models")
        .and_then(|models| models.get(model_id))
        .and_then(Value::as_object)
    {
        Some(model_entry) => model_entry,
        None => return false,
    };
    let model = match model_entry
        .get("model")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(model) => model,
        None => return false,
    };
    if model.is_empty() {
        return false;
    }

    let provider = model_entry
        .get("provider")
        .and_then(|v| serde_json::from_value::<LlmProvider>(v.clone()).ok())
        .unwrap_or(LlmProvider::Builtin(
            BuiltinLlmProvider::GoogleTranslateFree,
        ));

    let provider_setting = match &provider {
        LlmProvider::Builtin(b) => settings
            .get("providers")
            .and_then(|p| p.get(serde_json::to_string(b).unwrap().trim_matches('"'))),
        LlmProvider::Custom(c) => settings.get("providers").and_then(|p| p.get(c)),
    };

    let api_host = provider_setting
        .and_then(|setting| string_field(setting, "apiHost"))
        .map(str::trim)
        .unwrap_or("");
    let api_key = provider_setting
        .and_then(|setting| string_field(setting, "apiKey"))
        .map(str::trim)
        .unwrap_or("");

    let (default_api_host, requires_api_key) = match &provider {
        LlmProvider::Builtin(b) => (b.default_api_host().to_string(), b.requires_api_key()),
        LlmProvider::Custom(c) => {
            let strategy = settings
                .get("customProviders")
                .and_then(|customs| customs.get(c))
                .and_then(|custom| custom.get("strategy"))
                .and_then(Value::as_str);
            if let Some(s) = strategy {
                if matches!(
                    s,
                    "openai_compatible" | "openai_responses" | "anthropic" | "gemini"
                ) {
                    (String::new(), true)
                } else {
                    return false;
                }
            } else {
                return false;
            }
        }
    };

    let has_api_host = !api_host.is_empty() || !default_api_host.is_empty();
    let has_api_key = !requires_api_key || !api_key.is_empty();

    has_api_host && has_api_key
}

fn is_batch_asr_configured(
    global_config: &Value,
    environment: AutomationRuleActivationEnvironment,
) -> bool {
    let batch_selection = global_config
        .get("asr")
        .and_then(|asr| asr.get("selections"))
        .and_then(|selections| selections.get("batch"));

    if batch_selection
        .and_then(|v| v.get("engine"))
        .and_then(Value::as_str)
        == Some("online")
    {
        let provider_id = batch_selection
            .and_then(|v| v.get("providerId"))
            .and_then(Value::as_str)
            .unwrap_or("");

        let Some(provider_def) = online_asr_providers().iter().find(|p| p.id == provider_id) else {
            return false;
        };

        if !provider_def.batch.local_file_mode.supported {
            return false;
        }

        let provider_settings = global_config
            .get("asr")
            .and_then(|asr| asr.get("providers"))
            .and_then(|providers| providers.get("online"))
            .and_then(|online| online.get(provider_id))
            .or_else(|| {
                if provider_id == VOLCENGINE_DOUBAO_PROVIDER_ID {
                    global_config
                        .get("asr")
                        .and_then(|asr| asr.get("providers"))
                        .and_then(|providers| providers.get(VOLCENGINE_DOUBAO_LEGACY_PROVIDER_KEY))
                } else {
                    None
                }
            });

        let Some(settings) = provider_settings else {
            return false;
        };

        let api_key = string_field(settings, "apiKey").unwrap_or("").trim();
        if api_key.is_empty() {
            return false;
        }

        if let Some(defaults) = provider_def.defaults.as_object() {
            for (key, default_val) in defaults {
                if key == "apiKey" || key == "unknownKey" {
                    continue;
                }
                let val = string_field(settings, key)
                    .unwrap_or(default_val.as_str().unwrap_or(""))
                    .trim();
                if val.is_empty() {
                    return false;
                }
            }
        }

        if provider_id == VOLCENGINE_DOUBAO_PROVIDER_ID {
            let batch_endpoint = string_field(settings, "batchEndpoint")
                .unwrap_or(
                    provider_def.defaults["batchEndpoint"]
                        .as_str()
                        .unwrap_or(""),
                )
                .trim();
            if batch_endpoint.contains("idle/submit") || batch_endpoint.ends_with("/submit") {
                return false;
            }
        }
        return true;
    }

    resolve_batch_model_path(global_config).is_some() && environment.batch_model_path_exists
}

fn string_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}
