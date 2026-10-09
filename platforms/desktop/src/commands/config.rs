use serde_json::Value;
use tauri::State;

use crate::services::DesktopServices;

#[tauri::command]
pub fn load_app_config(services: State<'_, DesktopServices>) -> Result<Option<Value>, String> {
    services.config.load()
}

#[tauri::command]
pub fn save_app_config(services: State<'_, DesktopServices>, config: Value) -> Result<(), String> {
    sona_core::config::validate_app_config(&config)
        .map_err(|error| format!("Invalid app config: {error}"))?;
    services.config.save(&config)
}

#[tauri::command]
pub fn get_app_setting(
    services: State<'_, DesktopServices>,
    key: String,
) -> Result<Option<Value>, String> {
    services.config.get_setting(&key)
}

#[tauri::command]
pub fn set_app_setting(
    services: State<'_, DesktopServices>,
    key: String,
    value: Value,
) -> Result<(), String> {
    services.config.set_setting(&key, &value)
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
