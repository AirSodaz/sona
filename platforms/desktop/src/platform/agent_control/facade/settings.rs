use crate::services::DesktopServices;

pub fn get_settings(
    services: &DesktopServices,
    key: Option<String>,
) -> Result<serde_json::Value, String> {
    if let Some(k) = key {
        services
            .config
            .get_setting(&k)
            .map(|v| v.unwrap_or(serde_json::Value::Null))
    } else {
        services
            .config
            .load()
            .map(|v| v.unwrap_or(serde_json::Value::Null))
    }
}

pub fn update_setting(
    services: &DesktopServices,
    key: String,
    value: serde_json::Value,
) -> Result<bool, String> {
    services.config.set_setting(&key, &value)?;
    let _ = services.emitter.emit(
        "app-setting-updated",
        serde_json::json!({
            "key": &key,
            "value": &value,
        }),
    );
    Ok(true)
}
