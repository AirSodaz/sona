use serde_json::Value;
use tauri::{AppHandle, Runtime};

#[tauri::command]
pub fn load_app_config<R: Runtime>(app: AppHandle<R>) -> Result<Option<Value>, String> {
    crate::platform::app_config::load_config(&app)
}

#[tauri::command]
pub fn save_app_config<R: Runtime>(app: AppHandle<R>, config: Value) -> Result<(), String> {
    sona_core::config::validate_app_config(&config)
        .map_err(|error| format!("Invalid app config: {error}"))?;
    crate::platform::app_config::save_config(&app, config)
}

#[tauri::command]
pub fn get_app_setting<R: Runtime>(
    app: AppHandle<R>,
    key: String,
) -> Result<Option<Value>, String> {
    crate::platform::app_config::get_setting(&app, key)
}

#[tauri::command]
pub fn set_app_setting<R: Runtime>(
    app: AppHandle<R>,
    key: String,
    value: Value,
) -> Result<(), String> {
    crate::platform::app_config::set_setting(&app, key, value)
}

#[tauri::command(rename_all = "camelCase")]
pub fn migrate_app_config(
    saved_config: Option<Value>,
    default_rule_set_name: String,
) -> sona_core::config::MigrationResult {
    sona_core::config::migrate_app_config(saved_config, default_rule_set_name)
}

#[tauri::command(rename_all = "camelCase")]
pub fn resolve_effective_config(global_config: Value, project: Option<Value>) -> Value {
    sona_core::config::resolve_effective_config(global_config, project)
}
