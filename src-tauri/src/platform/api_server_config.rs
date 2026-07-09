use crate::platform::paths::{PathKind, PathProvider, TauriPathProvider};
use sona_core::runtime::serve::{
    ServeStartupSettings, online_asr_config_from_app_config, serve_startup_settings_from_app_config,
};
use sona_sqlite::config_store::{
    load_app_config_payload_from_app_local_data_dir,
    load_serve_startup_settings_from_app_local_data_dir,
};
use std::collections::HashMap;

fn load_sqlite_app_config_payload(provider: &dyn PathProvider) -> Option<serde_json::Value> {
    let app_local_data_dir = provider.resolve_path(PathKind::AppLocalData).ok()?;
    load_app_config_payload_from_app_local_data_dir(&app_local_data_dir)
        .map_err(|error| {
            log::warn!("[API Server] Failed to load SQLite app config: {error}");
            error
        })
        .ok()
        .flatten()
}

fn load_sqlite_serve_startup_settings(provider: &dyn PathProvider) -> Option<ServeStartupSettings> {
    let app_local_data_dir = provider.resolve_path(PathKind::AppLocalData).ok()?;
    load_serve_startup_settings_from_app_local_data_dir(&app_local_data_dir)
        .map_err(|error| {
            log::warn!("[API Server] Failed to load SQLite startup settings: {error}");
            error
        })
        .ok()
        .flatten()
}

fn load_legacy_settings_config(provider: &dyn PathProvider) -> Option<serde_json::Value> {
    let app_data_dir = provider.resolve_path(PathKind::AppData).ok()?;
    sona_runtime_fs::load_legacy_settings_app_config(&app_data_dir)
        .map_err(|error| {
            log::warn!("[API Server] Failed to load legacy settings: {error}");
            error
        })
        .ok()
        .flatten()
}

fn load_app_config_for_server(provider: &dyn PathProvider) -> Option<serde_json::Value> {
    load_sqlite_app_config_payload(provider).or_else(|| load_legacy_settings_config(provider))
}

pub fn load_online_asr_config(provider: &dyn PathProvider) -> HashMap<String, serde_json::Value> {
    load_app_config_for_server(provider)
        .map(|config| online_asr_config_from_app_config(&config))
        .unwrap_or_default()
}

pub fn load_online_asr_config_for_app<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> HashMap<String, serde_json::Value> {
    let provider = TauriPathProvider::from_app(app);
    load_online_asr_config(&provider)
}

pub fn load_api_server_startup_settings(provider: &dyn PathProvider) -> ServeStartupSettings {
    if let Some(settings) = load_sqlite_serve_startup_settings(provider) {
        return settings;
    }
    load_app_config_for_server(provider)
        .map(|config| serve_startup_settings_from_app_config(&config))
        .unwrap_or_default()
}

pub fn load_api_server_startup_settings_for_app<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> ServeStartupSettings {
    let provider = TauriPathProvider::from_app(app);
    load_api_server_startup_settings(&provider)
}
