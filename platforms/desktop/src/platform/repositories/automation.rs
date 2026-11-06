use serde_json::Value;
use sona_core::automation::AutomationError;
use sona_core::automation::repository::{
    AutomationProcessedInput, AutomationProfileInput, AutomationRepositoryInput,
    AutomationRuleInput,
};
use sona_core::automation::{AutomationRule, AutomationRuleValidationResult};
use sona_runtime_fs::{UuidGenerator, validate_native_automation_rule_activation};
use std::sync::Arc;

use crate::platform::database::DesktopSqliteState;
use crate::services::db_runner::{run_sqlite_task, spawn_blocking_map};

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

pub async fn load_repository_state(
    sqlite: &DesktopSqliteState,
) -> Result<AutomationRepositoryState, String> {
    run_sqlite_task(sqlite, move |context| {
        let adapter = context.automation_adapter(Arc::new(UuidGenerator));
        adapter.load_state()
    })
    .await
}

pub async fn persist_rules(
    sqlite: &DesktopSqliteState,
    rules: Vec<AutomationRuleInput>,
) -> Result<(), String> {
    run_sqlite_task(sqlite, move |context| {
        let adapter = context.automation_adapter(Arc::new(UuidGenerator));
        adapter.replace_rules(rules)
    })
    .await
}

pub async fn persist_profiles(
    sqlite: &DesktopSqliteState,
    profiles: Vec<AutomationProfileInput>,
) -> Result<(), String> {
    run_sqlite_task(sqlite, move |context| {
        let adapter = context.automation_adapter(Arc::new(UuidGenerator));
        adapter.replace_profiles(profiles)
    })
    .await
}

pub async fn persist_processed_entries(
    sqlite: &DesktopSqliteState,
    processed_entries: Vec<AutomationProcessedInput>,
) -> Result<(), String> {
    run_sqlite_task(sqlite, move |context| {
        let adapter = context.automation_adapter(Arc::new(UuidGenerator));
        adapter.replace_processed_entries(processed_entries)
    })
    .await
}

pub async fn persist_repository_state(
    sqlite: &DesktopSqliteState,
    input: AutomationRepositoryInput,
) -> Result<(), String> {
    run_sqlite_task(sqlite, move |context| {
        let adapter = context.automation_adapter(Arc::new(UuidGenerator));
        adapter.replace_state(input)
    })
    .await
}
