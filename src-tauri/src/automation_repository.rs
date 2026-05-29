use crate::asr_providers::{
    online_asr_providers, VOLCENGINE_DOUBAO_LEGACY_PROVIDER_KEY, VOLCENGINE_DOUBAO_PROVIDER_ID,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, Runtime};
use uuid::Uuid;

const AUTOMATION_DIR_NAME: &str = "automation";
const RULES_FILE_NAME: &str = "rules.json";
const PROCESSED_FILE_NAME: &str = "processed.json";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationRepositoryState {
    pub rules: Vec<Value>,
    pub processed_entries: Vec<Value>,
}

#[derive(Clone, Debug)]
pub struct AutomationRepository {
    app_local_data_dir: PathBuf,
}

impl AutomationRepository {
    pub fn new(app_local_data_dir: PathBuf) -> Self {
        Self { app_local_data_dir }
    }

    fn automation_dir(&self) -> PathBuf {
        self.app_local_data_dir.join(AUTOMATION_DIR_NAME)
    }

    fn rules_path(&self) -> PathBuf {
        self.automation_dir().join(RULES_FILE_NAME)
    }

    fn processed_path(&self) -> PathBuf {
        self.automation_dir().join(PROCESSED_FILE_NAME)
    }

    pub fn ensure_ready(&self) -> Result<(), String> {
        fs::create_dir_all(self.automation_dir()).map_err(|error| error.to_string())?;
        ensure_json_array_file(&self.rules_path())?;
        ensure_json_array_file(&self.processed_path())?;
        Ok(())
    }

    pub fn load_state(&self) -> Result<AutomationRepositoryState, String> {
        self.ensure_ready()?;
        Ok(AutomationRepositoryState {
            rules: read_json_array_or_empty(&self.rules_path()),
            processed_entries: read_json_array_or_empty(&self.processed_path()),
        })
    }

    pub fn persist_rules(&self, rules: Vec<Value>) -> Result<(), String> {
        self.ensure_ready()?;
        write_json_pretty_atomic(&self.rules_path(), &rules)
    }

    pub fn persist_processed_entries(&self, processed_entries: Vec<Value>) -> Result<(), String> {
        self.ensure_ready()?;
        write_json_pretty_atomic(&self.processed_path(), &processed_entries)
    }

    pub fn persist_state(
        &self,
        rules: Vec<Value>,
        processed_entries: Vec<Value>,
    ) -> Result<(), String> {
        self.ensure_ready()?;
        write_json_pretty_atomic(&self.rules_path(), &rules)?;
        write_json_pretty_atomic(&self.processed_path(), &processed_entries)
    }
}

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

struct ProviderDefinition {
    default_api_host: String,
    requires_api_key: bool,
}

fn default_export_mode() -> String {
    "original".to_string()
}

fn ensure_json_array_file(path: &Path) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    fs::write(path, "[]").map_err(|error| error.to_string())
}

fn read_json_array_or_empty(path: &Path) -> Vec<Value> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(_) => return Vec::new(),
    };

    match serde_json::from_str::<Value>(&content) {
        Ok(Value::Array(items)) => items,
        _ => Vec::new(),
    }
}

fn write_json_pretty_atomic<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), String> {
    let serialized = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    write_binary_atomic(path, &serialized)
}

fn write_binary_atomic(path: &Path, contents: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let temp_path = path.with_extension(format!(
        "{}.tmp-{}",
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("json"),
        Uuid::new_v4()
    ));

    {
        let mut writer =
            BufWriter::new(File::create(&temp_path).map_err(|error| error.to_string())?);
        writer
            .write_all(contents)
            .map_err(|error| error.to_string())?;
        writer.flush().map_err(|error| error.to_string())?;
    }

    replace_path_atomically(&temp_path, path)
}

fn replace_path_atomically(temp_path: &Path, final_path: &Path) -> Result<(), String> {
    let backup_path = final_path.with_extension(format!(
        "{}.bak-{}",
        final_path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("tmp"),
        Uuid::new_v4()
    ));
    let had_existing = final_path.exists();

    if had_existing {
        fs::rename(final_path, &backup_path).map_err(|error| error.to_string())?;
    }

    match fs::rename(temp_path, final_path) {
        Ok(()) => {
            if had_existing {
                remove_path_if_exists(&backup_path)?;
            }
            Ok(())
        }
        Err(error) => {
            if had_existing && !final_path.exists() {
                let _ = fs::rename(&backup_path, final_path);
            }
            let _ = remove_path_if_exists(temp_path);
            Err(error.to_string())
        }
    }
}

fn remove_path_if_exists(path: &Path) -> Result<(), String> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => {
            fs::remove_dir_all(path).map_err(|error| error.to_string())
        }
        Ok(_) => fs::remove_file(path).map_err(|error| error.to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

pub fn normalize_automation_path(path: &str) -> String {
    path.trim()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
}

#[allow(dead_code)]
pub fn create_automation_fingerprint(file_path: &str, size: u64, mtime_ms: u64) -> String {
    format!(
        "{}::{}::{}",
        normalize_automation_path(file_path),
        size,
        mtime_ms
    )
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

    let provider = model_entry
        .get("provider")
        .and_then(Value::as_str)
        .map(normalize_provider)
        .unwrap_or("google_translate_free");
    let provider_definition = match provider_definition(provider, settings.get("customProviders")) {
        Some(definition) => definition,
        None => return false,
    };
    let provider_setting = settings
        .get("providers")
        .and_then(|providers| providers.get(provider));
    let api_host = provider_setting
        .and_then(|setting| string_field(setting, "apiHost"))
        .map(str::trim)
        .unwrap_or("");
    let api_key = provider_setting
        .and_then(|setting| string_field(setting, "apiKey"))
        .map(str::trim)
        .unwrap_or("");
    let has_api_host = !api_host.is_empty() || !provider_definition.default_api_host.is_empty();
    let has_api_key = !provider_definition.requires_api_key || !api_key.is_empty();

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

fn normalize_provider(provider: &str) -> &str {
    match provider {
        "azure_open_ai" => "azure_openai",
        "deepseek" => "deep_seek",
        "moonshot" => "kimi",
        "openai" => "open_ai",
        "openai_compatible" | "open_ai_compatible" => "custom-openai-compatible",
        "siliconflow" => "silicon_flow",
        value => value,
    }
}

fn custom_provider_strategy<'a>(
    provider: &str,
    custom_providers: Option<&'a Value>,
) -> Option<&'a str> {
    custom_providers?
        .get(provider)?
        .get("strategy")
        .and_then(Value::as_str)
}

fn provider_definition(
    provider: &str,
    custom_providers: Option<&Value>,
) -> Option<ProviderDefinition> {
    if provider.starts_with("custom-") {
        let strategy = custom_provider_strategy(provider, custom_providers)?;
        if !matches!(
            strategy,
            "openai_compatible" | "openai_responses" | "anthropic" | "gemini"
        ) {
            return None;
        }
        return Some(ProviderDefinition {
            default_api_host: String::new(),
            requires_api_key: true,
        });
    }

    let definition = match provider {
        "google_translate_free" => ProviderDefinition {
            default_api_host: "https://translate.googleapis.com/translate_a/single".to_string(),
            requires_api_key: false,
        },
        "google_translate" => ProviderDefinition {
            default_api_host: "https://translation.googleapis.com/language/translate/v2"
                .to_string(),
            requires_api_key: true,
        },
        "open_ai" => ProviderDefinition {
            default_api_host: "https://api.openai.com".to_string(),
            requires_api_key: true,
        },
        "open_ai_responses" => ProviderDefinition {
            default_api_host: "https://api.openai.com".to_string(),
            requires_api_key: true,
        },
        "azure_openai" => ProviderDefinition {
            default_api_host: String::new(),
            requires_api_key: true,
        },
        "anthropic" => ProviderDefinition {
            default_api_host: "https://api.anthropic.com".to_string(),
            requires_api_key: true,
        },
        "gemini" => ProviderDefinition {
            default_api_host: "https://generativelanguage.googleapis.com".to_string(),
            requires_api_key: true,
        },
        "ollama" => ProviderDefinition {
            default_api_host: "http://127.0.0.1:11434".to_string(),
            requires_api_key: false,
        },
        "deep_seek" => ProviderDefinition {
            default_api_host: "https://api.deepseek.com".to_string(),
            requires_api_key: true,
        },
        "moonshot_ai" => ProviderDefinition {
            default_api_host: "https://api.moonshot.ai".to_string(),
            requires_api_key: true,
        },
        "moonshot_cn" => ProviderDefinition {
            default_api_host: "https://api.moonshot.cn".to_string(),
            requires_api_key: true,
        },
        "xiaomi" => ProviderDefinition {
            default_api_host: "https://api.xiaomimimo.com".to_string(),
            requires_api_key: true,
        },
        "kimi" => ProviderDefinition {
            default_api_host: "https://api.moonshot.cn".to_string(),
            requires_api_key: true,
        },
        "silicon_flow" => ProviderDefinition {
            default_api_host: "https://api.siliconflow.cn".to_string(),
            requires_api_key: true,
        },
        "qwen" => ProviderDefinition {
            default_api_host: "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string(),
            requires_api_key: true,
        },
        "qwen_portal" => ProviderDefinition {
            default_api_host: "https://portal.qwen.ai/v1".to_string(),
            requires_api_key: true,
        },
        "minimax_global" => ProviderDefinition {
            default_api_host: "https://api.minimaxi.chat/v1".to_string(),
            requires_api_key: true,
        },
        "minimax_cn" => ProviderDefinition {
            default_api_host: "https://api.minimax.chat/v1".to_string(),
            requires_api_key: true,
        },
        "openrouter" => ProviderDefinition {
            default_api_host: "https://openrouter.ai/api/v1".to_string(),
            requires_api_key: true,
        },
        "lm_studio" => ProviderDefinition {
            default_api_host: "http://localhost:1234/v1".to_string(),
            requires_api_key: false,
        },
        "groq" => ProviderDefinition {
            default_api_host: "https://api.groq.com/openai".to_string(),
            requires_api_key: true,
        },
        "x_ai" => ProviderDefinition {
            default_api_host: "https://api.x.ai".to_string(),
            requires_api_key: true,
        },
        "mistral_ai" => ProviderDefinition {
            default_api_host: "https://api.mistral.ai/v1".to_string(),
            requires_api_key: true,
        },
        "perplexity" => ProviderDefinition {
            default_api_host: "https://api.perplexity.ai".to_string(),
            requires_api_key: true,
        },
        "volcengine" => ProviderDefinition {
            default_api_host: "https://ark.cn-beijing.volces.com".to_string(),
            requires_api_key: true,
        },
        "chatglm" => ProviderDefinition {
            default_api_host: "https://open.bigmodel.cn/api/paas/v4/".to_string(),
            requires_api_key: true,
        },
        _ => return None,
    };
    Some(definition)
}

fn resolve_app_local_data_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    app.path()
        .app_local_data_dir()
        .map_err(|error| error.to_string())
}

async fn run_repository_task<R, T, F>(app: AppHandle<R>, task: F) -> Result<T, String>
where
    R: Runtime,
    T: Send + 'static,
    F: FnOnce(AutomationRepository) -> Result<T, String> + Send + 'static,
{
    let app_local_data_dir = resolve_app_local_data_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        task(AutomationRepository::new(app_local_data_dir))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn automation_load_repository_state<R: Runtime>(
    app: AppHandle<R>,
) -> Result<AutomationRepositoryState, String> {
    run_repository_task(app, |repository| repository.load_state()).await
}

#[tauri::command]
pub async fn automation_persist_rules<R: Runtime>(
    app: AppHandle<R>,
    rules: Vec<Value>,
) -> Result<(), String> {
    run_repository_task(app, move |repository| repository.persist_rules(rules)).await
}

#[tauri::command]
pub async fn automation_persist_processed_entries<R: Runtime>(
    app: AppHandle<R>,
    processed_entries: Vec<Value>,
) -> Result<(), String> {
    run_repository_task(app, move |repository| {
        repository.persist_processed_entries(processed_entries)
    })
    .await
}

#[tauri::command]
pub async fn automation_persist_repository_state<R: Runtime>(
    app: AppHandle<R>,
    rules: Vec<Value>,
    processed_entries: Vec<Value>,
) -> Result<(), String> {
    run_repository_task(app, move |repository| {
        repository.persist_state(rules, processed_entries)
    })
    .await
}

#[tauri::command]
pub async fn automation_validate_rule_activation(
    rule: AutomationRule,
    global_config: Value,
    project: Option<Value>,
) -> Result<AutomationRuleValidationResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        Ok(validate_rule_activation_inner(
            &rule,
            &global_config,
            project.as_ref(),
        ))
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
