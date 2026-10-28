use std::path::PathBuf;
use tauri::State;

use crate::integrations::emby::{
    EmbyClient, EmbyConnectionConfig, EmbyItemListResult, EmbyLibrary, EmbyServerInfo, EmbyState,
    SubtitleUploadPayload,
};
use crate::services::DesktopServices;

#[tauri::command(rename_all = "camelCase")]
pub async fn emby_test_connection(
    emby_state: State<'_, EmbyState>,
    config: EmbyConnectionConfig,
) -> Result<EmbyServerInfo, String> {
    let client = EmbyClient::new(config).map_err(|e| e.to_string())?;
    let info = client.test_connection().await.map_err(|e| e.to_string())?;
    emby_state.set_client(client).await;
    Ok(info)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn emby_save_config(
    services: State<'_, DesktopServices>,
    emby_state: State<'_, EmbyState>,
    config: EmbyConnectionConfig,
) -> Result<(), String> {
    if let Ok(client) = EmbyClient::new(config.clone()) {
        emby_state.set_client(client).await;
    }

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
    temp_dir: Option<String>,
) -> Result<String, String> {
    let client = emby_state.get_client().await.map_err(|e| e.to_string())?;
    let dir = match temp_dir {
        Some(d) if !d.trim().is_empty() => PathBuf::from(d),
        _ => std::env::temp_dir().join("sona").join("emby_audio"),
    };
    let filename = format!("emby_{}_{}.mp3", item_id, stream_index.unwrap_or(0));
    let target_path = dir.join(filename);

    client
        .download_audio_stream(&item_id, stream_index, &target_path)
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
    let client = emby_state.get_client().await.map_err(|e| e.to_string())?;

    if let Some(idx) = delete_existing_external_index
        && let Err(e) = client.delete_subtitle(&payload.item_id, idx).await
    {
        log::warn!("Failed to delete existing external subtitle {idx}: {e}");
    }

    client
        .upload_subtitle(&payload)
        .await
        .map_err(|e| e.to_string())?;

    if let Err(e) = client.refresh_item_metadata(&payload.item_id).await {
        log::warn!(
            "Failed to refresh metadata after upload for {}: {e}",
            payload.item_id
        );
    }

    Ok(())
}
