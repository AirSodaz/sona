use crate::platform::automation_runtime::{
    AutomationRuntimePathCollectionResult, AutomationRuntimeReplaceResult,
    AutomationRuntimeRuleConfig, AutomationRuntimeState, collect_rule_path_results,
    create_event_sink, replace_rule_runtimes_with, scan_rule_runtime, start_rule_runtime,
};
use serde_json::Value;
use sona_core::automation::repository::{
    AutomationProcessedInput, AutomationProfileInput, AutomationRepositoryInput,
    AutomationRuleInput,
};
use sona_core::automation::{AutomationRule, AutomationRuleValidationResult};
use tauri::{AppHandle, Runtime, State};

fn validate_automation_input<T: serde::Serialize + ?Sized>(value: &T) -> Result<(), String> {
    sona_ts_bind::validate_typescript_safe_integers(value).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn automation_load_repository_state<R: Runtime>(
    app: AppHandle<R>,
) -> Result<crate::platform::automation_repository::AutomationRepositoryState, String> {
    crate::platform::automation_repository::load_repository_state(&app).await
}

#[tauri::command]
pub async fn automation_persist_rules<R: Runtime>(
    app: AppHandle<R>,
    rules: Vec<AutomationRuleInput>,
) -> Result<(), String> {
    validate_automation_input(&rules)?;
    crate::platform::automation_repository::persist_rules(&app, rules).await
}

#[tauri::command]
pub async fn automation_persist_profiles<R: Runtime>(
    app: AppHandle<R>,
    profiles: Vec<AutomationProfileInput>,
) -> Result<(), String> {
    validate_automation_input(&profiles)?;
    crate::platform::automation_repository::persist_profiles(&app, profiles).await
}

#[tauri::command]
pub async fn automation_persist_processed_entries<R: Runtime>(
    app: AppHandle<R>,
    processed_entries: Vec<AutomationProcessedInput>,
) -> Result<(), String> {
    validate_automation_input(&processed_entries)?;
    crate::platform::automation_repository::persist_processed_entries(&app, processed_entries).await
}

#[tauri::command]
pub async fn automation_persist_repository_state<R: Runtime>(
    app: AppHandle<R>,
    profiles: Vec<AutomationProfileInput>,
    rules: Vec<AutomationRuleInput>,
    processed_entries: Vec<AutomationProcessedInput>,
) -> Result<(), String> {
    let input = AutomationRepositoryInput {
        profiles,
        rules,
        processed_entries,
    };
    validate_automation_input(&input)?;
    crate::platform::automation_repository::persist_repository_state(&app, input).await
}

#[tauri::command]
pub async fn automation_validate_rule_activation(
    rule: AutomationRule,
    global_config: Value,
    tags: Vec<Value>,
) -> Result<AutomationRuleValidationResult, String> {
    crate::platform::automation_repository::validate_rule_activation(rule, global_config, tags)
        .await
}

#[tauri::command]
pub async fn replace_automation_runtime_rules<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AutomationRuntimeState>,
    rules: Vec<AutomationRuntimeRuleConfig>,
) -> Result<Vec<AutomationRuntimeReplaceResult>, String> {
    validate_automation_input(&rules)?;
    let runtime_state = state.inner().clone();
    let result = replace_rule_runtimes_with(runtime_state.clone(), rules, move |rule| {
        start_rule_runtime(app.clone(), runtime_state.clone(), rule)
    })
    .await;
    validate_automation_input(&result)?;
    Ok(result)
}

#[tauri::command]
pub async fn scan_automation_runtime_rule<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AutomationRuntimeState>,
    rule: AutomationRuntimeRuleConfig,
) -> Result<(), String> {
    validate_automation_input(&rule)?;
    scan_rule_runtime(state.inner().clone(), create_event_sink(app), rule).await
}

#[tauri::command]
pub async fn collect_automation_runtime_rule_paths(
    rule: AutomationRuntimeRuleConfig,
    file_paths: Vec<String>,
) -> Result<Vec<AutomationRuntimePathCollectionResult>, String> {
    validate_automation_input(&rule)?;
    let result = collect_rule_path_results(rule, file_paths).await?;
    validate_automation_input(&result)?;
    Ok(result)
}
