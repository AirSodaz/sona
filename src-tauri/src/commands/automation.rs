use crate::platform::automation_repository::{run_automation_task, validate_rule_activation_inner};
use crate::platform::automation_runtime::{
    AutomationRuntimePathCollectionResult, AutomationRuntimeReplaceResult,
    AutomationRuntimeRuleConfig, AutomationRuntimeState, collect_rule_path_result,
    create_event_sink, replace_rule_runtimes_with, scan_rule_runtime, start_rule_runtime,
};
use serde_json::Value;
use sona_core::automation::{AutomationRule, AutomationRuleValidationResult};
use sona_sqlite::automation::AutomationRepositoryState;
use tauri::{AppHandle, Runtime, State};

#[tauri::command]
pub async fn automation_load_repository_state<R: Runtime>(
    app: AppHandle<R>,
) -> Result<AutomationRepositoryState, String> {
    run_automation_task(&app, |repository| repository.load_state()).await
}

#[tauri::command]
pub async fn automation_persist_rules<R: Runtime>(
    app: AppHandle<R>,
    rules: Vec<Value>,
) -> Result<(), String> {
    run_automation_task(&app, move |repository| repository.persist_rules(rules)).await
}

#[tauri::command]
pub async fn automation_persist_processed_entries<R: Runtime>(
    app: AppHandle<R>,
    processed_entries: Vec<Value>,
) -> Result<(), String> {
    run_automation_task(&app, move |repository| {
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
    run_automation_task(&app, move |repository| {
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

#[tauri::command]
pub async fn replace_automation_runtime_rules<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AutomationRuntimeState>,
    rules: Vec<AutomationRuntimeRuleConfig>,
) -> Result<Vec<AutomationRuntimeReplaceResult>, String> {
    let runtime_state = state.inner().clone();
    Ok(
        replace_rule_runtimes_with(runtime_state.clone(), rules, move |rule| {
            start_rule_runtime(app.clone(), runtime_state.clone(), rule)
        })
        .await,
    )
}

#[tauri::command]
pub async fn scan_automation_runtime_rule<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AutomationRuntimeState>,
    rule: AutomationRuntimeRuleConfig,
) -> Result<(), String> {
    scan_rule_runtime(state.inner().clone(), create_event_sink(app), rule).await
}

#[tauri::command]
pub async fn collect_automation_runtime_rule_paths(
    rule: AutomationRuntimeRuleConfig,
    file_paths: Vec<String>,
) -> Result<Vec<AutomationRuntimePathCollectionResult>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        Ok(file_paths
            .into_iter()
            .map(|file_path| collect_rule_path_result(&rule, &file_path))
            .collect())
    })
    .await
    .map_err(|error| error.to_string())?
}
