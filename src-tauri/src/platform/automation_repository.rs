use serde_json::Value;
use sona_core::automation::{
    AutomationRule, AutomationRuleActivationEnvironment, AutomationRuleValidationResult,
    is_virtual_automation_project, normalize_automation_path, resolve_batch_model_path,
    validate_rule_activation,
};
use sona_sqlite::DatabaseError;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tauri::{AppHandle, Manager, Runtime};

pub fn validate_rule_activation_inner(
    rule: &AutomationRule,
    global_config: &Value,
    project: Option<&Value>,
) -> AutomationRuleValidationResult {
    let watch_directory = rule.watch_directory.trim();
    let export_directory = rule.export_config.directory.trim();

    let watch_directory_exists = !watch_directory.is_empty() && Path::new(watch_directory).exists();
    let export_directory_ready = if should_prepare_export_directory(
        rule,
        project,
        watch_directory,
        export_directory,
        watch_directory_exists,
    ) {
        prepare_export_directory(export_directory)
    } else {
        false
    };
    let batch_model_path_exists = resolve_batch_model_path(global_config)
        .as_deref()
        .map(|path| Path::new(path).exists())
        .unwrap_or(false);

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

fn should_prepare_export_directory(
    rule: &AutomationRule,
    project: Option<&Value>,
    watch_directory: &str,
    export_directory: &str,
    watch_directory_exists: bool,
) -> bool {
    !rule.name.trim().is_empty()
        && (project.is_some() || is_virtual_automation_project(&rule.project_id))
        && !watch_directory.is_empty()
        && !export_directory.is_empty()
        && normalize_automation_path(watch_directory) != normalize_automation_path(export_directory)
        && watch_directory_exists
}

fn prepare_export_directory(export_directory: &str) -> bool {
    match fs::create_dir_all(export_directory) {
        Ok(()) => true,
        Err(error) => {
            log::error!("[Automation] Failed to prepare output directory: {error}");
            false
        }
    }
}

pub async fn run_automation_task<R, T, F>(app: &AppHandle<R>, task: F) -> Result<T, String>
where
    R: Runtime,
    T: Send + 'static,
    F: FnOnce(sona_sqlite::automation::SqliteAutomationRepository) -> Result<T, DatabaseError>
        + Send
        + 'static,
{
    let db = Arc::clone(app.state::<Arc<sona_sqlite::Database>>().inner());
    tauri::async_runtime::spawn_blocking(move || {
        task(sona_sqlite::automation::SqliteAutomationRepository::new(db))
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|e| e.to_string())
}
