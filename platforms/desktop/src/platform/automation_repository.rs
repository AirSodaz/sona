use serde_json::Value;
use sona_core::automation::service::{AutomationRepositoryService, AutomationValidationService};
use sona_core::automation::{AutomationRule, AutomationRuleValidationResult};
use sona_runtime_fs::{NativeAutomationFileSystem, UuidGenerator};
use tauri::{AppHandle, Runtime};

pub use sona_sqlite::automation::AutomationRepositoryState;

pub fn validate_rule_activation_inner(
    rule: &AutomationRule,
    global_config: &Value,
    project: Option<&Value>,
) -> AutomationRuleValidationResult {
    AutomationValidationService::new(&NativeAutomationFileSystem).validate_rule_activation(
        rule,
        global_config,
        project,
    )
}

pub async fn validate_rule_activation(
    rule: AutomationRule,
    global_config: Value,
    project: Option<Value>,
) -> Result<AutomationRuleValidationResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        validate_rule_activation_inner(&rule, &global_config, project.as_ref())
    })
    .await
    .map_err(|error| error.to_string())
}

pub async fn run_automation_task<R, T, F>(app: &AppHandle<R>, task: F) -> Result<T, String>
where
    R: Runtime,
    T: Send + 'static,
    F: FnOnce(sona_sqlite::automation::SqliteAutomationRepository) -> Result<T, String>
        + Send
        + 'static,
{
    let db = crate::platform::database::sqlite_database(app);
    tauri::async_runtime::spawn_blocking(move || {
        task(sona_sqlite::automation::SqliteAutomationRepository::new(db))
    })
    .await
    .map_err(|error| error.to_string())?
}

pub async fn load_repository_state<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<AutomationRepositoryState, String> {
    run_automation_task(app, |repository| {
        AutomationRepositoryService::new(&repository, &UuidGenerator).load_state()
    })
    .await
}

pub async fn persist_rules<R: Runtime>(
    app: &AppHandle<R>,
    rules: Vec<Value>,
) -> Result<(), String> {
    run_automation_task(app, move |repository| {
        AutomationRepositoryService::new(&repository, &UuidGenerator).replace_rules_json(rules)
    })
    .await
}

pub async fn persist_processed_entries<R: Runtime>(
    app: &AppHandle<R>,
    processed_entries: Vec<Value>,
) -> Result<(), String> {
    run_automation_task(app, move |repository| {
        AutomationRepositoryService::new(&repository, &UuidGenerator)
            .replace_processed_entries_json(processed_entries)
    })
    .await
}

pub async fn persist_repository_state<R: Runtime>(
    app: &AppHandle<R>,
    rules: Vec<Value>,
    processed_entries: Vec<Value>,
) -> Result<(), String> {
    run_automation_task(app, move |repository| {
        AutomationRepositoryService::new(&repository, &UuidGenerator)
            .replace_state_json(rules, processed_entries)
    })
    .await
}
