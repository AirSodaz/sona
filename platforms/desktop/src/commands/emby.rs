use tauri::State;

use crate::integrations::emby::{
    EmbyClient, EmbyConnectionConfig, EmbyError, EmbyItemListResult, EmbyLibrary, EmbyServerInfo,
    EmbyState, SubtitleUploadPayload,
};
use crate::services::DesktopServices;

#[tauri::command(rename_all = "camelCase")]
pub async fn emby_test_connection(config: EmbyConnectionConfig) -> Result<EmbyServerInfo, String> {
    let client = EmbyClient::new(config).map_err(|e| e.to_string())?;
    let info = client.test_connection().await.map_err(|e| e.to_string())?;
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

    let client = EmbyClient::new(config.clone()).map_err(|e| e.to_string())?;
    let mut current_config = services
        .config
        .load()?
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
    }

    services.config.save(&current_config)?;
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

    if let (Some(server_url), Some(api_key)) = (server_url, api_key)
        && !server_url.trim().is_empty()
        && !api_key.trim().is_empty()
    {
        return Ok(Some(EmbyConnectionConfig {
            server_url,
            api_key,
            allow_invalid_certs,
        }));
    }

    Ok(None)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn emby_get_libraries(
    emby_state: State<'_, EmbyState>,
) -> Result<Vec<EmbyLibrary>, String> {
    let client = emby_state.get_client().await.map_err(|e| e.to_string())?;
    client.get_libraries().await.map_err(|e| e.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn emby_get_items(
    emby_state: State<'_, EmbyState>,
    parent_id: Option<String>,
    search_term: Option<String>,
    start_index: u32,
    limit: u32,
) -> Result<EmbyItemListResult, String> {
    let client = emby_state.get_client().await.map_err(|e| e.to_string())?;
    client
        .get_items(
            parent_id.as_deref(),
            search_term.as_deref(),
            start_index,
            limit,
        )
        .await
        .map_err(|e| e.to_string())
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

    let client = emby_state.get_client().await.map_err(|e| e.to_string())?;
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
        .map_err(|e| e.to_string())?;

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

    let client = emby_state.get_client().await.map_err(|e| e.to_string())?;

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
                log::warn!("Failed to delete existing external subtitle {idx}: {e}");
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
                .map_err(|e| e.to_string())?;

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
                    .map_err(|e| e.to_string())?;

                client
                    .upload_subtitle(&payload)
                    .await
                    .map_err(|e| e.to_string())?;
            } else {
                return Err(EmbyError::DuplicateSubtitle.to_string());
            }
        }
        Err(e) => return Err(e.to_string()),
    }

    if let Err(e) = client.refresh_item_metadata(&payload.item_id).await {
        log::warn!(
            "Failed to refresh metadata after upload for {}: {e}",
            payload.item_id
        );
    }

    Ok(())
}
