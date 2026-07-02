use crate::core::paths::{PathKind, PathProvider};
use crate::integrations::asr_providers::{
    VOLCENGINE_DOUBAO_LEGACY_PROVIDER_KEY, VOLCENGINE_DOUBAO_PROVIDER_ID, online_asr_providers,
};
use serde_json::Value;
use std::fs;
use std::path::Path;

use super::types::*;

pub fn normalize_automation_path(path: &str) -> String {
    path.trim()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
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

fn is_virtual_automation_project(project_id: &str) -> bool {
    matches!(project_id, "inbox" | "none")
}

pub fn validate_rule_activation_inner(
    rule: &AutomationRule,
    global_config: &Value,
    project: Option<&Value>,
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

    if !Path::new(watch_directory).exists() {
        return invalid_validation(
            "automation.watch_directory_missing",
            "The watch directory does not exist.",
        );
    }

    if let Err(error) = fs::create_dir_all(export_directory) {
        log::error!("[Automation] Failed to prepare output directory: {}", error);
        return invalid_validation(
            "automation.output_directory_invalid",
            "The output directory could not be created.",
        );
    }

    if !is_batch_asr_configured(global_config) {
        return invalid_validation(
            "automation.offline_model_missing",
            "A batch ASR model or cloud ASR credential is required before automation can run.",
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

    use crate::core::domain::{BuiltinLlmProvider, LlmProvider};

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

fn is_batch_asr_configured(global_config: &Value) -> bool {
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

        // Check required fields from defaults if missing in settings
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

    let offline_model_path = batch_selection
        .and_then(|selection| string_field(selection, "modelPath"))
        .filter(|value| !value.trim().is_empty())
        .or_else(|| string_field(global_config, "offlineModelPath"))
        .unwrap_or_default();
    !offline_model_path.trim().is_empty() && Path::new(offline_model_path.trim()).exists()
}

fn string_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

pub async fn run_repository_task<T, F>(provider: &dyn PathProvider, task: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(crate::repositories::automation::SqliteAutomationRepository) -> Result<T, String>
        + Send
        + 'static,
{
    let app_local_data_dir = provider.resolve_path(PathKind::AppLocalData)?;
    tauri::async_runtime::spawn_blocking(move || {
        task(crate::repositories::automation::SqliteAutomationRepository::new(app_local_data_dir))
    })
    .await
    .map_err(|error| error.to_string())?
}

pub async fn run_automation_task<T, F>(
    provider: &dyn PathProvider,
    lock: std::sync::Arc<std::sync::Mutex<()>>,
    task: F,
) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(crate::repositories::automation::SqliteAutomationRepository) -> Result<T, String>
        + Send
        + 'static,
{
    let app_local_data_dir = provider.resolve_path(PathKind::AppLocalData)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = lock.lock().map_err(|error| error.to_string())?;
        task(crate::repositories::automation::SqliteAutomationRepository::new(app_local_data_dir))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn feature_llm_config_accepts_custom_provider_definition() {
        let config = json!({
            "llmSettings": {
                "customProviders": {
                    "custom-acme": {
                        "id": "custom-acme",
                        "name": "Acme Gateway",
                        "strategy": "openai_responses",
                        "createdAt": "2026-05-18T08:00:00.000Z"
                    }
                },
                "providers": {
                    "custom-acme": {
                        "apiHost": "https://gateway.example.com",
                        "apiKey": "test-key"
                    }
                },
                "models": {
                    "model-1": {
                        "id": "model-1",
                        "provider": "custom-acme",
                        "model": "gpt-4o"
                    }
                },
                "selections": {
                    "polishModelId": "model-1"
                }
            }
        });

        assert!(is_feature_llm_config_complete(&config, "polish"));
    }

    #[test]
    fn feature_llm_config_rejects_custom_provider_without_required_key() {
        let config = json!({
            "llmSettings": {
                "customProviders": {
                    "custom-acme": {
                        "id": "custom-acme",
                        "name": "Acme Gateway",
                        "strategy": "gemini",
                        "createdAt": "2026-05-18T08:00:00.000Z"
                    }
                },
                "providers": {
                    "custom-acme": {
                        "apiHost": "https://gateway.example.com",
                        "apiKey": ""
                    }
                },
                "models": {
                    "model-1": {
                        "id": "model-1",
                        "provider": "custom-acme",
                        "model": "gemini-2.5-flash"
                    }
                },
                "selections": {
                    "translationModelId": "model-1"
                }
            }
        });

        assert!(!is_feature_llm_config_complete(&config, "translation"));
    }
}
