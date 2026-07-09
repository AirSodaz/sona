use serde_json::Value;
use tauri::{AppHandle, Runtime};

pub fn load_config<R: Runtime>(app: &AppHandle<R>) -> Result<Option<Value>, String> {
    crate::platform::database::sqlite_config_store(app)
        .load_config()
        .map_err(|error| error.to_string())
}

pub fn save_config<R: Runtime>(app: &AppHandle<R>, config: Value) -> Result<(), String> {
    crate::platform::database::sqlite_config_store(app)
        .save_config(&config)
        .map_err(|error| error.to_string())
}

pub fn get_setting<R: Runtime>(app: &AppHandle<R>, key: String) -> Result<Option<Value>, String> {
    crate::platform::database::sqlite_config_store(app)
        .get_setting(&key)
        .map_err(|error| error.to_string())
}

pub fn set_setting<R: Runtime>(
    app: &AppHandle<R>,
    key: String,
    value: Value,
) -> Result<(), String> {
    crate::platform::database::sqlite_config_store(app)
        .set_setting(&key, &value)
        .map_err(|error| error.to_string())
}
