use serde_json::Value;
use sona_core::automation::AutomationError;
use sona_core::automation::repository::{
    AutomationProcessedInput, AutomationProfileInput, AutomationRepositoryInput,
    AutomationRuleInput,
};
use sona_core::automation::{AutomationRule, AutomationRuleValidationResult};
use sona_runtime_fs::{UuidGenerator, validate_native_automation_rule_activation};
use sona_sqlite::SqliteAutomationAdapter;
use std::sync::Arc;
use tauri::{AppHandle, Runtime};

use crate::platform::blocking::{spawn_blocking_map, with_sqlite_context_transport};

pub use sona_sqlite::automation::AutomationRepositoryState;

pub fn validate_rule_activation_inner(
    rule: &AutomationRule,
    global_config: &Value,
    tags: &[Value],
) -> Result<AutomationRuleValidationResult, AutomationError> {
    validate_native_automation_rule_activation(rule, global_config, tags)
}

pub async fn validate_rule_activation(
    rule: AutomationRule,
    global_config: Value,
    tags: Vec<Value>,
) -> Result<AutomationRuleValidationResult, String> {
    spawn_blocking_map(move || validate_rule_activation_inner(&rule, &global_config, &tags)).await
}

async fn run_automation_adapter_task<R, T, F>(app: &AppHandle<R>, task: F) -> Result<T, String>
where
    R: Runtime,
    T: Send + serde::Serialize + 'static,
    F: FnOnce(&SqliteAutomationAdapter) -> Result<T, AutomationError> + Send + 'static,
{
    with_sqlite_context_transport(app, move |context| {
        let adapter = context.automation_adapter(Arc::new(UuidGenerator));
        task(&adapter)
    })
    .await
}

pub async fn load_repository_state<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<AutomationRepositoryState, String> {
    run_automation_adapter_task(app, |adapter| adapter.load_state()).await
}

pub async fn persist_rules<R: Runtime>(
    app: &AppHandle<R>,
    rules: Vec<AutomationRuleInput>,
) -> Result<(), String> {
    run_automation_adapter_task(app, move |adapter| adapter.replace_rules(rules)).await
}

pub async fn persist_profiles<R: Runtime>(
    app: &AppHandle<R>,
    profiles: Vec<AutomationProfileInput>,
) -> Result<(), String> {
    run_automation_adapter_task(app, move |adapter| adapter.replace_profiles(profiles)).await
}

pub async fn persist_processed_entries<R: Runtime>(
    app: &AppHandle<R>,
    processed_entries: Vec<AutomationProcessedInput>,
) -> Result<(), String> {
    run_automation_adapter_task(app, move |adapter| {
        adapter.replace_processed_entries(processed_entries)
    })
    .await
}

pub async fn persist_repository_state<R: Runtime>(
    app: &AppHandle<R>,
    input: AutomationRepositoryInput,
) -> Result<(), String> {
    run_automation_adapter_task(app, move |adapter| adapter.replace_state(input)).await
}
