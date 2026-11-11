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
    services.config.save(&config)?;
    if let Some(mode_str) = config.get("localModelReleaseMode").and_then(|v| v.as_str()) {
        services.local_model_pool.set_release_mode(mode_str.into());
    }
    let game_enabled = config
        .get("gameModeEnabled")
        .and_then(|v| v.as_bool())
        .unwrap_or_else(|| services.local_model_pool.is_game_mode_enabled());
    let game_procs = config
        .get("gameModeProcessList")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    services
        .local_model_pool
        .set_game_mode(game_enabled, game_procs);
    Ok(())
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
    services.config.set_setting(&key, &value)?;
    if key == "localModelReleaseMode"
        && let Some(mode_str) = value.as_str()
    {
        services.local_model_pool.set_release_mode(mode_str.into());
    } else if key == "gameModeEnabled"
        && let Some(enabled) = value.as_bool()
    {
        let procs = services
            .config
            .get_setting("gameModeProcessList")
            .ok()
            .flatten()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();
        services.local_model_pool.set_game_mode(enabled, &procs);
    } else if key == "gameModeProcessList"
        && let Some(procs) = value.as_str()
    {
        let enabled = services
            .config
            .get_setting("gameModeEnabled")
            .ok()
            .flatten()
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        services.local_model_pool.set_game_mode(enabled, procs);
    }
    Ok(())
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
