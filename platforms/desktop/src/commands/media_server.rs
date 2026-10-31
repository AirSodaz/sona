use serde_json::Value;
use std::sync::Arc;
use tauri::State;

use crate::integrations::media_server::{
    EmbyClient, JellyfinClient, MediaServerError, MediaServerInfo, MediaServerItemListResult,
    MediaServerLibrary, MediaServerLoginPayload, MediaServerLoginResult, MediaServerProfile,
    MediaServerRegistry, MediaServerSubtitleUploadPayload, MediaServerType, normalize_server_url,
};
use crate::services::DesktopServices;

pub fn sanitize_media_server_error(err: impl std::fmt::Display, api_key: Option<&str>) -> String {
    let mut msg = err.to_string();
    if let Some(key) = api_key {
        let trimmed_key = key.trim();
        if !trimmed_key.is_empty() {
            msg = msg.replace(trimmed_key, "[REDACTED]");
        }
    }

    let mut result = String::with_capacity(msg.len());
    let mut remaining = &msg[..];
    while let Some(pos) = remaining
        .as_bytes()
        .windows(8)
        .position(|w| w.eq_ignore_ascii_case(b"api_key="))
    {
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
pub async fn media_server_test_connection(
    mut profile: MediaServerProfile,
) -> Result<MediaServerInfo, String> {
    profile.server_url = normalize_server_url(&profile.server_url);
    profile.api_key = profile.api_key.trim().to_string();
    if profile.server_url.is_empty() || profile.api_key.is_empty() {
        return Err("Server URL and API key cannot be empty".to_string());
    }

    let api_key = profile.api_key.clone();
    let client: Arc<dyn crate::integrations::media_server::MediaServerClient> =
        match profile.server_type {
            MediaServerType::Emby => {
                let c = EmbyClient::new(profile)
                    .map_err(|e| sanitize_media_server_error(e, Some(&api_key)))?;
                Arc::new(c)
            }
            MediaServerType::Jellyfin => {
                let c = JellyfinClient::new(profile)
                    .map_err(|e| sanitize_media_server_error(e, Some(&api_key)))?;
                Arc::new(c)
            }
        };

    client
        .test_connection()
        .await
        .map_err(|e| sanitize_media_server_error(e, Some(&api_key)))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn media_server_login(
    payload: MediaServerLoginPayload,
) -> Result<MediaServerLoginResult, String> {
    let clean_url = normalize_server_url(&payload.server_url);
    let clean_user = payload.username.trim().to_string();
    let password = payload.password.unwrap_or_default();
    if clean_url.is_empty() || clean_user.is_empty() {
        return Err("Server URL and username cannot be empty".to_string());
    }

    let result = match payload.server_type {
        MediaServerType::Emby => {
            EmbyClient::authenticate_by_name(
                &clean_url,
                &clean_user,
                &password,
                payload.allow_invalid_certs,
            )
            .await
        }
        MediaServerType::Jellyfin => {
            JellyfinClient::authenticate_by_name(
                &clean_url,
                &clean_user,
                &password,
                payload.allow_invalid_certs,
            )
            .await
        }
    };

    result.map_err(|e| match e {
        MediaServerError::Unauthorized => "Invalid username or password".to_string(),
        other => sanitize_media_server_error(other, None),
    })
}

#[tauri::command(rename_all = "camelCase")]
pub fn media_server_list_profiles(
    services: State<'_, DesktopServices>,
    registry: State<'_, MediaServerRegistry>,
) -> Result<Vec<MediaServerProfile>, String> {
    let config = services.config.load()?;
    if let Some(servers) = config
        .as_ref()
        .and_then(|c| c.get("mediaServers").or_else(|| c.get("media_servers")))
        && let Ok(profiles) = serde_json::from_value::<Vec<MediaServerProfile>>(servers.clone())
        && !profiles.is_empty()
    {
        return Ok(profiles);
    }
    Ok(registry.list_profiles())
}

#[tauri::command(rename_all = "camelCase")]
pub fn media_server_save_profile(
    services: State<'_, DesktopServices>,
    registry: State<'_, MediaServerRegistry>,
    mut profile: MediaServerProfile,
) -> Result<(), String> {
    profile.server_url = normalize_server_url(&profile.server_url);
    profile.api_key = profile.api_key.trim().to_string();
    if profile.server_url.is_empty() || profile.api_key.is_empty() {
        return Err("Server URL and API key cannot be empty".to_string());
    }

    let mut config = services
        .config
        .load()?
        .filter(|v| v.is_object())
        .unwrap_or_else(|| serde_json::json!({}));

    let mut profiles: Vec<MediaServerProfile> = config
        .get("mediaServers")
        .or_else(|| config.get("media_servers"))
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    if let Some(existing) = profiles.iter_mut().find(|p| p.id == profile.id) {
        *existing = profile.clone();
    } else {
        profiles.push(profile.clone());
    }

    let active_id = config
        .get("activeMediaServerId")
        .or_else(|| config.get("active_media_server_id"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let should_set_active = active_id.is_none() || profiles.len() == 1;
    let next_active_id = if should_set_active {
        profile.id.clone()
    } else {
        active_id.unwrap()
    };

    if let Some(obj) = config.as_object_mut() {
        obj.insert(
            "mediaServers".to_string(),
            serde_json::to_value(&profiles).map_err(|e| e.to_string())?,
        );
        obj.insert(
            "activeMediaServerId".to_string(),
            serde_json::Value::String(next_active_id.clone()),
        );
    }

    services.config.save(&config)?;

    registry
        .register(profile.clone())
        .map_err(|e| e.to_string())?;

    if should_set_active {
        let _ = registry.set_active(&profile.id);
    }

    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
pub fn media_server_delete_profile(
    services: State<'_, DesktopServices>,
    registry: State<'_, MediaServerRegistry>,
    profile_id: String,
) -> Result<(), String> {
    let mut config = services
        .config
        .load()?
        .filter(|v| v.is_object())
        .unwrap_or_else(|| serde_json::json!({}));

    let mut profiles: Vec<MediaServerProfile> = config
        .get("mediaServers")
        .or_else(|| config.get("media_servers"))
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    profiles.retain(|p| p.id != profile_id);

    let active_id = config
        .get("activeMediaServerId")
        .or_else(|| config.get("active_media_server_id"))
        .and_then(|v| v.as_str());

    let next_active = if active_id == Some(&profile_id) {
        profiles.first().map(|p| p.id.clone())
    } else {
        active_id.map(|s| s.to_string())
    };

    if let Some(obj) = config.as_object_mut() {
        obj.insert(
            "mediaServers".to_string(),
            serde_json::to_value(&profiles).map_err(|e| e.to_string())?,
        );
        match next_active {
            Some(ref id) => {
                obj.insert(
                    "activeMediaServerId".to_string(),
                    serde_json::Value::String(id.clone()),
                );
            }
            None => {
                obj.remove("activeMediaServerId");
                obj.remove("active_media_server_id");
            }
        }
    }

    services.config.save(&config)?;
    registry.unregister(&profile_id);

    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
pub fn media_server_set_active_profile(
    services: State<'_, DesktopServices>,
    registry: State<'_, MediaServerRegistry>,
    profile_id: String,
) -> Result<(), String> {
    registry
        .set_active(&profile_id)
        .map_err(|e| e.to_string())?;

    let mut config = services
        .config
        .load()?
        .filter(|v| v.is_object())
        .unwrap_or_else(|| serde_json::json!({}));

    if let Some(obj) = config.as_object_mut() {
        obj.insert(
            "activeMediaServerId".to_string(),
            serde_json::Value::String(profile_id),
        );
    }

    services.config.save(&config)?;
    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn media_server_get_libraries(
    registry: State<'_, MediaServerRegistry>,
    server_id: Option<String>,
) -> Result<Vec<MediaServerLibrary>, String> {
    let client = registry
        .get(server_id.as_deref())
        .map_err(|e| e.to_string())?;
    let api_key = client.profile().api_key.clone();

    client
        .get_libraries()
        .await
        .map_err(|e| sanitize_media_server_error(e, Some(&api_key)))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn media_server_get_items(
    registry: State<'_, MediaServerRegistry>,
    server_id: Option<String>,
    parent_id: Option<String>,
    search_term: Option<String>,
    limit: Option<u32>,
    start_index: Option<u32>,
) -> Result<MediaServerItemListResult, String> {
    let client = registry
        .get(server_id.as_deref())
        .map_err(|e| e.to_string())?;
    let api_key = client.profile().api_key.clone();

    client
        .get_items(
            parent_id.as_deref(),
            search_term.as_deref(),
            limit.unwrap_or(50),
            start_index.unwrap_or(0),
        )
        .await
        .map_err(|e| sanitize_media_server_error(e, Some(&api_key)))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn media_server_download_audio(
    registry: State<'_, MediaServerRegistry>,
    server_id: Option<String>,
    item_id: String,
    stream_index: Option<i32>,
    filename: Option<String>,
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

    let client = registry
        .get(server_id.as_deref())
        .map_err(|e| e.to_string())?;
    let api_key = client.profile().api_key.clone();

    let dir = std::env::temp_dir().join("sona").join("media_server_audio");
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| format!("Failed to create download directory: {e}"))?;

    let file_name = filename.unwrap_or_else(|| {
        format!(
            "media_server_{}_{}_{}.mp3",
            clean_id,
            stream_index.unwrap_or(0),
            uuid::Uuid::new_v4()
        )
    });
    let target_path = dir.join(file_name);

    client
        .download_audio_stream(clean_id, stream_index, &target_path)
        .await
        .map_err(|e| sanitize_media_server_error(e, Some(&api_key)))?;

    Ok(target_path.to_string_lossy().to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn media_server_upload_subtitle(
    registry: State<'_, MediaServerRegistry>,
    server_id: Option<String>,
    mut payload: MediaServerSubtitleUploadPayload,
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
    let clean_language = payload.language.trim();
    let clean_format = payload.format.trim();
    let clean_content = payload.content.trim();
    if clean_language.is_empty() || clean_format.is_empty() || clean_content.is_empty() {
        return Err(
            "Invalid upload payload: language, format, and content must not be empty".to_string(),
        );
    }
    if !clean_format.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("Invalid subtitle format: must be ASCII alphanumeric".to_string());
    }

    payload.item_id = clean_id.to_string();
    payload.language = clean_language.to_string();
    payload.format = clean_format.to_string();

    let client = registry
        .get(server_id.as_deref())
        .map_err(|e| e.to_string())?;
    let api_key = client.profile().api_key.clone();

    if let Some(idx) = delete_existing_external_index.filter(|&idx| idx >= 0)
        && let Err(e) = client.delete_subtitle(&payload.item_id, idx).await
    {
        log::warn!(
            "Failed to delete existing external subtitle {idx}: {}",
            sanitize_media_server_error(&e, Some(&api_key))
        );
    }

    client
        .upload_subtitle(&payload)
        .await
        .map_err(|e| sanitize_media_server_error(e, Some(&api_key)))?;

    let _ = client.refresh_item_metadata(&payload.item_id).await;
    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
pub fn media_server_get_processing_settings(
    services: State<'_, DesktopServices>,
) -> Result<Option<Value>, String> {
    let config = services.config.load()?;
    let Some(config_val) = config else {
        return Ok(None);
    };

    Ok(config_val
        .get("mediaServerProcessingSettings")
        .or_else(|| config_val.get("media_server_processing_settings"))
        .or_else(|| config_val.get("embyProcessingSettings"))
        .or_else(|| config_val.get("emby_processing_settings"))
        .cloned())
}

#[tauri::command(rename_all = "camelCase")]
pub fn media_server_save_processing_settings(
    services: State<'_, DesktopServices>,
    settings: Value,
) -> Result<(), String> {
    if !settings.is_object() {
        return Err("Settings must be a JSON object".to_string());
    }

    let mut current_config = services
        .config
        .load()?
        .filter(|v| v.is_object())
        .unwrap_or_else(|| serde_json::json!({}));

    if let Some(obj) = current_config.as_object_mut() {
        obj.insert(
            "mediaServerProcessingSettings".to_string(),
            settings.clone(),
        );
        obj.insert("embyProcessingSettings".to_string(), settings);
    }

    services.config.save(&current_config)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_media_server_error() {
        let err = "Failed at http://127.0.0.1:8096/Videos/123/Subtitles?api_key=my_secret_token";
        let sanitized = sanitize_media_server_error(err, Some("my_secret_token"));
        assert!(!sanitized.contains("my_secret_token"));
        assert!(sanitized.contains("api_key=[REDACTED]"));
    }

    #[test]
    fn test_media_server_profile_persistence_and_active_selection() {
        let temp_dir = tempfile::tempdir().unwrap();
        let ctx = std::sync::Arc::new(
            sona_sqlite::SqliteApplicationContext::open(temp_dir.path()).unwrap(),
        );
        let sqlite = crate::platform::database::DesktopSqliteState::new(ctx);
        let emitter = std::sync::Arc::new(crate::platform::event::MockEventEmitter::new());
        let services = DesktopServices::builder()
            .sqlite(sqlite)
            .event_emitter(emitter)
            .build()
            .unwrap();
        let registry = MediaServerRegistry::new();

        let p1 = MediaServerProfile {
            id: "emby-1".to_string(),
            name: "Emby 1".to_string(),
            server_type: MediaServerType::Emby,
            server_url: "http://localhost:8096".to_string(),
            api_key: "key1".to_string(),
            username: Some("user1".to_string()),
            admin_user_id: None,
            allow_invalid_certs: false,
            created_at: 1000,
            updated_at: 1000,
        };

        let p2 = MediaServerProfile {
            id: "jellyfin-2".to_string(),
            name: "Jellyfin 2".to_string(),
            server_type: MediaServerType::Jellyfin,
            server_url: "http://localhost:8096".to_string(),
            api_key: "key2".to_string(),
            username: Some("user2".to_string()),
            admin_user_id: None,
            allow_invalid_certs: false,
            created_at: 1000,
            updated_at: 1000,
        };

        // Register into registry
        registry.register(p1.clone()).unwrap();
        registry.register(p2.clone()).unwrap();

        assert_eq!(registry.get_active_id(), Some("emby-1".to_string()));
        registry.set_active("jellyfin-2").unwrap();
        assert_eq!(registry.get_active_id(), Some("jellyfin-2".to_string()));

        // Persist to config
        let config = serde_json::json!({
            "mediaServers": [p1.clone(), p2.clone()],
            "activeMediaServerId": "jellyfin-2"
        });
        services.config.save(&config).unwrap();

        let loaded = services.config.load().unwrap().unwrap();
        let servers: Vec<MediaServerProfile> =
            serde_json::from_value(loaded["mediaServers"].clone()).unwrap();
        assert_eq!(servers.len(), 2);
        assert_eq!(servers[0].id, "emby-1");
        assert_eq!(servers[1].id, "jellyfin-2");
        assert_eq!(loaded["activeMediaServerId"], "jellyfin-2");

        // Delete profile 1
        registry.unregister("emby-1");
        assert_eq!(registry.get_active_id(), Some("jellyfin-2".to_string()));
        let profiles_in_reg = registry.list_profiles();
        assert_eq!(profiles_in_reg.len(), 1);
        assert_eq!(profiles_in_reg[0].id, "jellyfin-2");
    }
}
