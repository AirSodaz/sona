use tauri::State;

use crate::integrations::emby::{
    EmbyClient, EmbyConnectionConfig, EmbyError, EmbyItemListResult, EmbyLibrary, EmbyServerInfo,
    EmbyState, SubtitleUploadPayload,
};
use crate::services::DesktopServices;

fn sanitize_emby_error(err: impl std::fmt::Display, api_key: Option<&str>) -> String {
    let mut msg = err.to_string();
    if let Some(key) = api_key {
        let trimmed_key = key.trim();
        if !trimmed_key.is_empty() {
            msg = msg.replace(trimmed_key, "[REDACTED]");
        }
    }

    let mut result = String::with_capacity(msg.len());
    let mut remaining = &msg[..];
    while let Some(pos) = remaining.to_ascii_lowercase().find("api_key=") {
        result.push_str(&remaining[..pos + 8]);
        let after_key = &remaining[pos + 8..];
        let end_pos = after_key
            .find(['&', ' ', '"', '\'', '\n', '\r'])
            .unwrap_or(after_key.len());
        result.push_str("[REDACTED]");
        remaining = &after_key[end_pos..];
    }
    result.push_str(remaining);
    result
}

#[tauri::command(rename_all = "camelCase")]
pub async fn emby_test_connection(config: EmbyConnectionConfig) -> Result<EmbyServerInfo, String> {
    let api_key = config.api_key.clone();
    let client = EmbyClient::new(config).map_err(|e| sanitize_emby_error(e, Some(&api_key)))?;
    let info = client
        .test_connection()
        .await
        .map_err(|e| sanitize_emby_error(e, Some(&api_key)))?;
    Ok(info)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn emby_save_config(
    services: State<'_, DesktopServices>,
    emby_state: State<'_, EmbyState>,
    config: EmbyConnectionConfig,
) -> Result<(), String> {
    if config.server_url.trim().is_empty() || config.api_key.trim().is_empty() {
        return Err("Server URL and API key cannot be empty".to_string());
    }

    let mut config = config;
    if config
        .admin_user_id
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        config.admin_user_id = None;
        if let Ok(temp_client) = EmbyClient::new(config.clone()) {
            config.admin_user_id = temp_client.discover_admin_user_id().await;
        }
    }

    let client = EmbyClient::new(config.clone())
        .map_err(|e| sanitize_emby_error(e, Some(&config.api_key)))?;
    let mut current_config = services
        .config
        .load()
        .map_err(|e| sanitize_emby_error(e, Some(&config.api_key)))?
        .filter(|v| v.is_object())
        .unwrap_or_else(|| serde_json::json!({}));
    if let Some(obj) = current_config.as_object_mut() {
        obj.insert(
            "embyServerUrl".to_string(),
            serde_json::Value::String(config.server_url.clone()),
        );
        obj.insert(
            "embyApiKey".to_string(),
            serde_json::Value::String(config.api_key.clone()),
        );
        obj.insert(
            "embyAllowInvalidCerts".to_string(),
            serde_json::Value::Bool(config.allow_invalid_certs),
        );
        if let Some(ref admin_id) = config.admin_user_id {
            obj.insert(
                "embyAdminUserId".to_string(),
                serde_json::Value::String(admin_id.clone()),
            );
        } else {
            obj.remove("embyAdminUserId");
        }
    }
    services
        .config
        .save(&current_config)
        .map_err(|e| sanitize_emby_error(e, Some(&config.api_key)))?;
    emby_state.set_client(Some(client)).await;
    let _ = services.emitter.emit(
        "app-setting-updated",
        serde_json::json!({
            "key": "sona-config",
            "value": &current_config,
        }),
    );
    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
pub fn emby_get_config(
    services: State<'_, DesktopServices>,
) -> Result<Option<EmbyConnectionConfig>, String> {
    let config = services.config.load()?;
    let Some(config_val) = config else {
        return Ok(None);
    };

    let server_url = config_val
        .get("embyServerUrl")
        .or_else(|| config_val.get("emby_server_url"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let api_key = config_val
        .get("embyApiKey")
        .or_else(|| config_val.get("emby_api_key"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let allow_invalid_certs = config_val
        .get("embyAllowInvalidCerts")
        .or_else(|| config_val.get("emby_allow_invalid_certs"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let admin_user_id = config_val
        .get("embyAdminUserId")
        .or_else(|| config_val.get("emby_admin_user_id"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    if let (Some(server_url), Some(api_key)) = (server_url, api_key)
        && !server_url.trim().is_empty()
        && !api_key.trim().is_empty()
    {
        return Ok(Some(EmbyConnectionConfig {
            server_url,
            api_key,
            allow_invalid_certs,
            admin_user_id,
        }));
    }

    Ok(None)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn emby_get_libraries(
    emby_state: State<'_, EmbyState>,
) -> Result<Vec<EmbyLibrary>, String> {
    let client = emby_state
        .get_client()
        .await
        .map_err(|e| sanitize_emby_error(e, None))?;
    let api_key = client.config().api_key.clone();
    client
        .get_libraries()
        .await
        .map_err(|e| sanitize_emby_error(e, Some(&api_key)))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn emby_get_items(
    emby_state: State<'_, EmbyState>,
    parent_id: Option<String>,
    search_term: Option<String>,
    start_index: u32,
    limit: u32,
) -> Result<EmbyItemListResult, String> {
    let client = emby_state
        .get_client()
        .await
        .map_err(|e| sanitize_emby_error(e, None))?;
    let api_key = client.config().api_key.clone();
    client
        .get_items(
            parent_id.as_deref(),
            search_term.as_deref(),
            start_index,
            limit,
        )
        .await
        .map_err(|e| sanitize_emby_error(e, Some(&api_key)))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn emby_download_audio(
    emby_state: State<'_, EmbyState>,
    item_id: String,
    stream_index: Option<i32>,
) -> Result<String, String> {
    let clean_id = item_id.trim();
    if clean_id.is_empty()
        || !clean_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("Invalid item ID format".to_string());
    }

    if let Some(idx) = stream_index
        && idx < 0
    {
        return Err("Stream index cannot be negative".to_string());
    }

    let client = emby_state
        .get_client()
        .await
        .map_err(|e| sanitize_emby_error(e, None))?;
    let api_key = client.config().api_key.clone();
    let dir = std::env::temp_dir().join("sona").join("emby_audio");
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| format!("Failed to create download directory: {e}"))?;

    let filename = format!(
        "emby_{}_{}_{}.mp3",
        clean_id,
        stream_index.unwrap_or(0),
        uuid::Uuid::new_v4()
    );
    let target_path = dir.join(filename);

    client
        .download_audio_stream(clean_id, stream_index, &target_path)
        .await
        .map_err(|e| sanitize_emby_error(e, Some(&api_key)))?;

    Ok(target_path.to_string_lossy().to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn emby_upload_subtitle(
    emby_state: State<'_, EmbyState>,
    payload: SubtitleUploadPayload,
    delete_existing_external_index: Option<i32>,
) -> Result<(), String> {
    let clean_id = payload.item_id.trim();
    if clean_id.is_empty()
        || !clean_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("Invalid item ID format".to_string());
    }
    if payload.language.trim().is_empty()
        || payload.format.trim().is_empty()
        || payload.content.trim().is_empty()
    {
        return Err(
            "Invalid upload payload: language, format, and content must not be empty".to_string(),
        );
    }

    let client = emby_state
        .get_client()
        .await
        .map_err(|e| sanitize_emby_error(e, None))?;
    let api_key = client.config().api_key.clone();

    let mut deleted_any = false;
    if let Some(idx) = delete_existing_external_index {
        if idx < 0 {
            return Err("Invalid external subtitle index: cannot be negative".to_string());
        }
        match client.delete_subtitle(&payload.item_id, idx).await {
            Ok(_) => {
                deleted_any = true;
            }
            Err(e) => {
                log::warn!(
                    "Failed to delete existing external subtitle {idx}: {}",
                    sanitize_emby_error(&e, Some(&api_key))
                );
            }
        }
    }

    let upload_result = client.upload_subtitle(&payload).await;
    match upload_result {
        Ok(_) => {}
        Err(EmbyError::DuplicateSubtitle) if !deleted_any => {
            log::info!(
                "[Emby] Duplicate subtitle detected for item {}, attempting auto-recovery",
                payload.item_id
            );
            let streams = client
                .get_item_streams(&payload.item_id)
                .await
                .map_err(|e| sanitize_emby_error(e, Some(&api_key)))?;

            let is_target_zh =
                crate::integrations::emby::is_chinese_language(Some(&payload.language), None);
            let matching_external = streams.iter().find(|s| {
                if !s.stream_type.eq_ignore_ascii_case("Subtitle") || !s.is_external {
                    return false;
                }
                if is_target_zh {
                    crate::integrations::emby::is_chinese_language(
                        s.language.as_deref(),
                        s.display_title.as_deref(),
                    )
                } else {
                    crate::integrations::emby::language_matches(
                        s.language.as_deref(),
                        &payload.language,
                    )
                }
            });

            if let Some(stream) = matching_external {
                log::info!(
                    "[Emby] Auto-recovering: deleting existing external subtitle track index {}",
                    stream.index
                );
                client
                    .delete_subtitle(&payload.item_id, stream.index)
                    .await
                    .map_err(|e| sanitize_emby_error(e, Some(&api_key)))?;

                client
                    .upload_subtitle(&payload)
                    .await
                    .map_err(|e| sanitize_emby_error(e, Some(&api_key)))?;
            } else {
                return Err(sanitize_emby_error(
                    EmbyError::DuplicateSubtitle,
                    Some(&api_key),
                ));
            }
        }
        Err(e) => return Err(sanitize_emby_error(e, Some(&api_key))),
    }

    if let Err(e) = client.refresh_item_metadata(&payload.item_id).await {
        log::warn!(
            "Failed to refresh metadata after upload for {}: {}",
            payload.item_id,
            sanitize_emby_error(&e, Some(&api_key))
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_emby_error_redacts_api_key_and_query() {
        let err = "Failed to connect to http://127.0.0.1:8096/emby/System/Info?api_key=secret_12345&foo=bar";
        let sanitized = sanitize_emby_error(err, Some("secret_12345"));
        assert!(!sanitized.contains("secret_12345"));
        assert!(sanitized.contains("api_key=[REDACTED]"));
        assert!(sanitized.contains("&foo=bar"));

        let err_no_key_provided = "Error at http://emby.local?Api_Key=supersecret";
        let sanitized_no_key = sanitize_emby_error(err_no_key_provided, None);
        assert!(!sanitized_no_key.contains("supersecret"));
        assert!(sanitized_no_key.contains("Api_Key=[REDACTED]"));
    }
}
