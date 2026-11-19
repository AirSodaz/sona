use tauri::State;

use crate::integrations::emby::{
    EmbyClient, EmbyConnectionConfig, EmbyError, EmbyItemListResult, EmbyLibrary, EmbyLoginPayload,
    EmbyServerInfo, EmbyState, SubtitleUploadPayload,
};
use crate::integrations::media_server::{MediaServerProfile, MediaServerRegistry, MediaServerType};
use crate::services::DesktopServices;

fn sanitize_emby_error(err: impl std::fmt::Display, api_key: Option<&str>) -> String {
    crate::commands::media_server::sanitize_media_server_error(err, api_key)
}

fn sync_legacy_emby_to_registry(
    registry: &MediaServerRegistry,
    config: &EmbyConnectionConfig,
    server_name: Option<&str>,
    persisted_config: Option<&mut serde_json::Value>,
) {
    let active_emby = registry
        .get_active_id()
        .and_then(|id| registry.get_profile(&id))
        .filter(|p| p.server_type == MediaServerType::Emby);

    let mut persisted_profiles: Option<Vec<MediaServerProfile>> = persisted_config
        .as_deref()
        .and_then(|c| c.get("mediaServers").or_else(|| c.get("media_servers")))
        .and_then(|v| serde_json::from_value::<Vec<MediaServerProfile>>(v.clone()).ok())
        .filter(|list| !list.is_empty());

    let existing_default = registry.get_profile("default-emby").or_else(|| {
        persisted_profiles
            .as_ref()
            .and_then(|list| list.iter().find(|p| p.id == "default-emby").cloned())
    });

    let (profile_id, profile_name, created_at) = match active_emby {
        Some(existing) => (existing.id, existing.name, existing.created_at),
        None => {
            let now = chrono::Utc::now().timestamp_millis();
            let fallback_created = existing_default
                .as_ref()
                .map(|p| p.created_at)
                .unwrap_or(now);
            let name = server_name
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .or_else(|| existing_default.as_ref().map(|p| p.name.clone()))
                .unwrap_or_else(|| "Emby Server".to_string());
            ("default-emby".to_string(), name, fallback_created)
        }
    };

    let profile = MediaServerProfile {
        id: profile_id.clone(),
        name: profile_name,
        server_type: MediaServerType::Emby,
        server_url: config.server_url.clone(),
        api_key: config.api_key.clone(),
        username: config.username.clone(),
        admin_user_id: config.admin_user_id.clone(),
        allow_invalid_certs: config.allow_invalid_certs,
        created_at,
        updated_at: chrono::Utc::now().timestamp_millis(),
    };

    if let Err(e) = registry.register(profile.clone()) {
        log::warn!(
            "[Emby] Failed to sync legacy Emby profile to MediaServerRegistry: {}",
            sanitize_emby_error(e, Some(&config.api_key))
        );
        return;
    }
    let _ = registry.set_active(&profile_id);

    if let (Some(cfg_val), Some(profiles)) = (persisted_config, persisted_profiles.as_mut()) {
        if let Some(existing) = profiles.iter_mut().find(|p| p.id == profile.id) {
            *existing = profile;
        } else {
            profiles.push(profile);
        }
        if let Some(obj) = cfg_val.as_object_mut() {
            if let Ok(serialized) = serde_json::to_value(profiles) {
                obj.insert("mediaServers".to_string(), serialized);
            }
            obj.insert(
                "activeMediaServerId".to_string(),
                serde_json::Value::String(profile_id),
            );
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn emby_test_connection(
    mut config: EmbyConnectionConfig,
) -> Result<EmbyServerInfo, String> {
    config.server_url = crate::integrations::emby::normalize_server_url(&config.server_url);
    config.api_key = config.api_key.trim().to_string();
    if config.server_url.is_empty() || config.api_key.is_empty() {
        return Err("Server URL and API key cannot be empty".to_string());
    }
    let api_key = config.api_key.clone();
    let client = EmbyClient::new(config).map_err(|e| sanitize_emby_error(e, Some(&api_key)))?;
    let info = client
        .test_connection()
        .await
        .map_err(|e| sanitize_emby_error(e, Some(&api_key)))?;
    Ok(info)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn emby_login(
    services: State<'_, DesktopServices>,
    emby_state: State<'_, EmbyState>,
    registry: State<'_, MediaServerRegistry>,
    payload: EmbyLoginPayload,
) -> Result<EmbyServerInfo, String> {
    let clean_url = crate::integrations::emby::normalize_server_url(&payload.server_url);
    let clean_user = payload.username.trim().to_string();
    let password = payload.password.unwrap_or_default();
    if clean_url.is_empty() || clean_user.is_empty() {
        return Err("Server URL and username cannot be empty".to_string());
    }

    let login_res = EmbyClient::authenticate_by_name(
        &clean_url,
        &clean_user,
        &password,
        payload.allow_invalid_certs,
    )
    .await
    .map_err(|e| match e {
        EmbyError::Unauthorized => "Invalid username or password".to_string(),
        other => sanitize_emby_error(other, None),
    })?;

    if !login_res.is_admin {
        return Err(
            "The account is not an administrator. Subtitle management requires administrator privileges."
                .to_string(),
        );
    }

    let config = EmbyConnectionConfig {
        server_url: clean_url.clone(),
        api_key: login_res.access_token.clone(),
        username: Some(login_res.user_name.clone()),
        allow_invalid_certs: payload.allow_invalid_certs,
        admin_user_id: Some(login_res.user_id.clone()),
    };

    let client = EmbyClient::new(config.clone())
        .map_err(|e| sanitize_emby_error(e, Some(&login_res.access_token)))?;

    let info = client
        .test_connection()
        .await
        .map_err(|e| sanitize_emby_error(e, Some(&login_res.access_token)))?;

    let mut current_config = services
        .config
        .load()
        .map_err(|e| sanitize_emby_error(e, Some(&login_res.access_token)))?
        .filter(|v| v.is_object())
        .unwrap_or_else(|| serde_json::json!({}));

    if let Some(obj) = current_config.as_object_mut() {
        obj.insert(
            "embyServerUrl".to_string(),
            serde_json::Value::String(clean_url),
        );
        obj.insert(
            "embyApiKey".to_string(),
            serde_json::Value::String(login_res.access_token.clone()),
        );
        obj.remove("embyToken");
        obj.remove("emby_token");
        obj.remove("emby_api_key");
        obj.insert(
            "embyUsername".to_string(),
            serde_json::Value::String(login_res.user_name),
        );
        obj.insert(
            "embyAdminUserId".to_string(),
            serde_json::Value::String(login_res.user_id),
        );
        obj.insert(
            "embyAllowInvalidCerts".to_string(),
            serde_json::Value::Bool(payload.allow_invalid_certs),
        );
    }

    sync_legacy_emby_to_registry(
        &registry,
        &config,
        Some(&info.server_name),
        Some(&mut current_config),
    );

    services
        .config
        .save(&current_config)
        .map_err(|e| sanitize_emby_error(e, Some(&login_res.access_token)))?;

    emby_state.set_client(Some(client)).await;
    let _ = services.emitter.emit(
        "app-setting-updated",
        serde_json::json!({
            "key": "sona-config",
            "value": &current_config,
        }),
    );

    Ok(info)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn emby_save_config(
    services: State<'_, DesktopServices>,
    emby_state: State<'_, EmbyState>,
    registry: State<'_, MediaServerRegistry>,
    config: EmbyConnectionConfig,
) -> Result<(), String> {
    let mut config = config;
    config.server_url = crate::integrations::emby::normalize_server_url(&config.server_url);
    config.api_key = config.api_key.trim().to_string();
    if config.server_url.is_empty() || config.api_key.is_empty() {
        return Err("Server URL and API key cannot be empty".to_string());
    }
    config.username = config
        .username
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    config.admin_user_id = config
        .admin_user_id
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if config.admin_user_id.is_none()
        && let Ok(temp_client) = EmbyClient::new(config.clone())
    {
        config.admin_user_id = temp_client.discover_admin_user_id().await;
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
        obj.remove("embyToken");
        obj.remove("emby_token");
        obj.remove("emby_api_key");
        if let Some(user) = &config.username {
            obj.insert(
                "embyUsername".to_string(),
                serde_json::Value::String(user.clone()),
            );
        } else {
            obj.remove("embyUsername");
            obj.remove("emby_username");
        }
        obj.insert(
            "embyAllowInvalidCerts".to_string(),
            serde_json::Value::Bool(config.allow_invalid_certs),
        );
        if let Some(admin_id) = &config.admin_user_id {
            obj.insert(
                "embyAdminUserId".to_string(),
                serde_json::Value::String(admin_id.clone()),
            );
        } else {
            obj.remove("embyAdminUserId");
            obj.remove("emby_admin_user_id");
        }
    }
    sync_legacy_emby_to_registry(&registry, &config, None, Some(&mut current_config));
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
        .get("embyToken")
        .or_else(|| config_val.get("emby_token"))
        .or_else(|| config_val.get("embyApiKey"))
        .or_else(|| config_val.get("emby_api_key"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let username = config_val
        .get("embyUsername")
        .or_else(|| config_val.get("emby_username"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
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

    if let (Some(server_url), Some(api_key)) = (server_url, api_key) {
        let clean_url = server_url.trim();
        let clean_key = api_key.trim();
        if !clean_url.is_empty() && !clean_key.is_empty() {
            return Ok(Some(EmbyConnectionConfig {
                server_url: clean_url.to_string(),
                api_key: clean_key.to_string(),
                username,
                allow_invalid_certs,
                admin_user_id,
            }));
        }
    }

    Ok(None)
}
#[tauri::command(rename_all = "camelCase")]
pub fn emby_get_processing_settings(
    services: State<'_, DesktopServices>,
) -> Result<Option<serde_json::Value>, String> {
    let config = services.config.load()?;
    let Some(config_val) = config else {
        return Ok(None);
    };

    Ok(config_val
        .get("embyProcessingSettings")
        .or_else(|| config_val.get("emby_processing_settings"))
        .or_else(|| config_val.get("embyToolOverrides"))
        .or_else(|| config_val.get("emby_tool_overrides"))
        .cloned())
}

#[tauri::command(rename_all = "camelCase")]
pub fn emby_save_processing_settings(
    services: State<'_, DesktopServices>,
    settings: serde_json::Value,
) -> Result<(), String> {
    let mut current_config = services
        .config
        .load()?
        .filter(|v| v.is_object())
        .unwrap_or_else(|| serde_json::json!({}));
    if let Some(obj) = current_config.as_object_mut() {
        obj.insert("embyProcessingSettings".to_string(), settings.clone());
    }
    services.config.save(&current_config)?;
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
    let clean_parent_id = parent_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let clean_search_term = search_term
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    client
        .get_items(clean_parent_id, clean_search_term, start_index, limit)
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
    mut payload: SubtitleUploadPayload,
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
            let mut matching_external: Vec<_> = streams
                .iter()
                .filter(|s| {
                    if !s.stream_type.eq_ignore_ascii_case("Subtitle")
                        || !s.is_external
                        || s.is_forced != payload.is_forced
                    {
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
                })
                .collect();

            if matching_external.is_empty() {
                return Err(sanitize_emby_error(
                    EmbyError::DuplicateSubtitle,
                    Some(&api_key),
                ));
            }

            matching_external.sort_by(|a, b| b.index.cmp(&a.index));
            for stream in matching_external {
                log::info!(
                    "[Emby] Auto-recovering: deleting existing external subtitle track index {}",
                    stream.index
                );
                client
                    .delete_subtitle(&payload.item_id, stream.index)
                    .await
                    .map_err(|e| sanitize_emby_error(e, Some(&api_key)))?;
            }

            client
                .upload_subtitle(&payload)
                .await
                .map_err(|e| sanitize_emby_error(e, Some(&api_key)))?;
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

    #[test]
    fn test_emby_processing_settings_save_and_get() {
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

        // 1. Initial state should be None
        let initial = services.config.load().unwrap();
        let get_initial = initial
            .as_ref()
            .and_then(|c| c.get("embyProcessingSettings"))
            .cloned();
        assert_eq!(get_initial, None);

        // 2. Save processing settings
        let settings_json = serde_json::json!({
            "asrLanguage": "ja",
            "defaultSubtitleMode": "pure_translation",
            "overwriteExistingExternal": false
        });

        let mut current_config = services
            .config
            .load()
            .unwrap()
            .filter(|v| v.is_object())
            .unwrap_or_else(|| serde_json::json!({}));
        current_config
            .as_object_mut()
            .unwrap()
            .insert("embyProcessingSettings".to_string(), settings_json.clone());
        services.config.save(&current_config).unwrap();

        // 3. Load and verify
        let loaded = services.config.load().unwrap().unwrap();
        let loaded_settings = loaded.get("embyProcessingSettings").unwrap();
        assert_eq!(loaded_settings, &settings_json);
        assert_eq!(loaded_settings["asrLanguage"], "ja");
        assert_eq!(loaded_settings["defaultSubtitleMode"], "pure_translation");
        assert_eq!(loaded_settings["overwriteExistingExternal"], false);
    }

    #[test]
    fn test_sync_legacy_emby_to_registry_creates_and_updates_default_emby() {
        let registry = MediaServerRegistry::new();
        let config = EmbyConnectionConfig {
            server_url: "http://localhost:8096".to_string(),
            api_key: "key-1".to_string(),
            username: Some("admin".to_string()),
            allow_invalid_certs: false,
            admin_user_id: Some("uid-1".to_string()),
        };

        sync_legacy_emby_to_registry(&registry, &config, Some("My Emby"), None);
        assert_eq!(registry.get_active_id(), Some("default-emby".to_string()));
        let profile = registry.get_profile("default-emby").unwrap();
        assert_eq!(profile.name, "My Emby");
        assert_eq!(profile.api_key, "key-1");

        let updated = EmbyConnectionConfig {
            api_key: "key-2".to_string(),
            ..config
        };
        sync_legacy_emby_to_registry(&registry, &updated, None, None);
        let profile_after = registry.get_profile("default-emby").unwrap();
        assert_eq!(profile_after.api_key, "key-2");
        assert_eq!(profile_after.name, "My Emby");
    }

    #[test]
    fn test_sync_legacy_emby_persists_media_servers_without_overwriting_jellyfin() {
        let registry = MediaServerRegistry::new();
        let jellyfin = MediaServerProfile {
            id: "jf-1".to_string(),
            name: "My Jellyfin".to_string(),
            server_type: MediaServerType::Jellyfin,
            server_url: "http://jellyfin.local:8096".to_string(),
            api_key: "jf-key".to_string(),
            username: None,
            admin_user_id: None,
            allow_invalid_certs: false,
            created_at: 1000,
            updated_at: 1000,
        };
        registry.register(jellyfin.clone()).unwrap();
        registry.set_active("jf-1").unwrap();

        let mut persisted_config = serde_json::json!({
            "mediaServers": [jellyfin],
            "activeMediaServerId": "jf-1"
        });
        let emby_cfg = EmbyConnectionConfig {
            server_url: "http://emby.local:8096".to_string(),
            api_key: "emby-key".to_string(),
            username: Some("admin".to_string()),
            allow_invalid_certs: false,
            admin_user_id: None,
        };

        sync_legacy_emby_to_registry(
            &registry,
            &emby_cfg,
            Some("My Emby"),
            Some(&mut persisted_config),
        );

        assert_eq!(registry.get_active_id(), Some("default-emby".to_string()));
        assert_eq!(persisted_config["activeMediaServerId"], "default-emby");
        let servers: Vec<MediaServerProfile> =
            serde_json::from_value(persisted_config["mediaServers"].clone()).unwrap();
        assert_eq!(servers.len(), 2);
        assert_eq!(servers[0].id, "jf-1");
        assert_eq!(servers[0].server_type, MediaServerType::Jellyfin);
        assert_eq!(servers[1].id, "default-emby");
        assert_eq!(servers[1].server_type, MediaServerType::Emby);
        assert_eq!(servers[1].name, "My Emby");

        // Subsequent save with None server_name preserves custom name on default-emby
        let updated = EmbyConnectionConfig {
            api_key: "emby-key-2".to_string(),
            ..emby_cfg
        };
        registry.set_active("jf-1").unwrap();
        sync_legacy_emby_to_registry(&registry, &updated, None, Some(&mut persisted_config));
        let servers_after: Vec<MediaServerProfile> =
            serde_json::from_value(persisted_config["mediaServers"].clone()).unwrap();
        let default_emby = servers_after.iter().find(|p| p.id == "default-emby").unwrap();
        assert_eq!(default_emby.name, "My Emby");
        assert_eq!(default_emby.api_key, "emby-key-2");
    }
}
