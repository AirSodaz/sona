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
    sona_core::runtime::logging::sanitize_credentials_text(&err.to_string(), api_key)
}

fn emit_config_updated(services: &DesktopServices, config: &Value) {
    let _ = services.emitter.emit(
        "app-setting-updated",
        serde_json::json!({
            "key": "sona-config",
            "value": config,
        }),
    );
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

pub(crate) fn hydrate_media_server_registry(registry: &MediaServerRegistry, config_val: &Value) {
    let media_servers_val = config_val
        .get("mediaServers")
        .or_else(|| config_val.get("media_servers"));
    let has_explicit_media_servers = media_servers_val.is_some_and(|v| v.is_array());
    let profiles: Vec<MediaServerProfile> = media_servers_val
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let active_id = config_val
        .get("activeMediaServerId")
        .or_else(|| config_val.get("active_media_server_id"))
        .and_then(|v| v.as_str());

    if !profiles.is_empty() {
        for profile in profiles {
            if let Err(e) = registry.register(profile) {
                log::warn!("[MediaServer] Failed to register profile on startup: {e}");
            }
        }
        if let Some(id) = active_id {
            let _ = registry.set_active(id);
        }
    } else if !has_explicit_media_servers {
        let server_url = config_val
            .get("embyServerUrl")
            .or_else(|| config_val.get("emby_server_url"))
            .and_then(|v| v.as_str());
        let api_key = config_val
            .get("embyToken")
            .or_else(|| config_val.get("emby_token"))
            .or_else(|| config_val.get("embyApiKey"))
            .or_else(|| config_val.get("emby_api_key"))
            .and_then(|v| v.as_str());
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

        if let (Some(url), Some(key)) = (server_url, api_key)
            && !url.trim().is_empty()
            && !key.trim().is_empty()
        {
            let now = chrono::Utc::now().timestamp_millis();
            let profile = MediaServerProfile {
                id: "default-emby".to_string(),
                name: "Emby Server".to_string(),
                server_type: MediaServerType::Emby,
                server_url: url.trim().to_string(),
                api_key: key.trim().to_string(),
                username,
                admin_user_id,
                allow_invalid_certs,
                created_at: now,
                updated_at: now,
            };
            if let Err(e) = registry.register(profile) {
                log::warn!("[MediaServer] Failed to register legacy default-emby on startup: {e}");
            } else {
                let _ = registry.set_active("default-emby");
            }
        }
    }
}

pub(crate) fn list_profiles_from_state(
    services: &DesktopServices,
    registry: &MediaServerRegistry,
) -> Result<Vec<MediaServerProfile>, String> {
    let config = services.config.load()?;
    if let Some(servers) = config
        .as_ref()
        .and_then(|c| c.get("mediaServers").or_else(|| c.get("media_servers")))
        && let Ok(profiles) = serde_json::from_value::<Vec<MediaServerProfile>>(servers.clone())
        && (!profiles.is_empty() || registry.list_profiles().is_empty())
    {
        return Ok(profiles);
    }
    Ok(registry.list_profiles())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LegacyEmbySyncAction {
    NoChange,
    Update(crate::integrations::emby::EmbyConnectionConfig),
    Clear,
}

fn clear_legacy_emby_keys(obj: &mut serde_json::Map<String, Value>) {
    for key in [
        "embyServerUrl",
        "emby_server_url",
        "embyApiKey",
        "emby_api_key",
        "embyToken",
        "emby_token",
        "embyUsername",
        "emby_username",
        "embyAdminUserId",
        "emby_admin_user_id",
        "embyAllowInvalidCerts",
        "emby_allow_invalid_certs",
    ] {
        obj.remove(key);
    }
}

fn write_legacy_emby_keys(
    obj: &mut serde_json::Map<String, Value>,
    cfg: &crate::integrations::emby::EmbyConnectionConfig,
) {
    obj.insert(
        "embyServerUrl".to_string(),
        Value::String(cfg.server_url.clone()),
    );
    obj.insert("embyApiKey".to_string(), Value::String(cfg.api_key.clone()));
    obj.remove("embyToken");
    obj.remove("emby_token");
    obj.remove("emby_api_key");
    obj.remove("emby_server_url");
    if let Some(user) = &cfg.username {
        obj.insert("embyUsername".to_string(), Value::String(user.clone()));
    } else {
        obj.remove("embyUsername");
    }
    obj.remove("emby_username");
    obj.insert(
        "embyAllowInvalidCerts".to_string(),
        Value::Bool(cfg.allow_invalid_certs),
    );
    obj.remove("emby_allow_invalid_certs");
    if let Some(admin_id) = &cfg.admin_user_id {
        obj.insert(
            "embyAdminUserId".to_string(),
            Value::String(admin_id.clone()),
        );
    } else {
        obj.remove("embyAdminUserId");
    }
    obj.remove("emby_admin_user_id");
}

fn validate_download_filename(filename: &str) -> Result<String, String> {
    let trimmed = filename.trim();
    if trimmed.is_empty() || trimmed.contains('/') || trimmed.contains('\\') {
        return Err("Invalid download filename".to_string());
    }
    let mut components = std::path::Path::new(trimmed).components();
    match (components.next(), components.next()) {
        (Some(std::path::Component::Normal(_)), None) => Ok(trimmed.to_string()),
        _ => Err("Invalid download filename".to_string()),
    }
}

pub(crate) fn save_profile_to_state(
    services: &DesktopServices,
    registry: &MediaServerRegistry,
    mut profile: MediaServerProfile,
) -> Result<LegacyEmbySyncAction, String> {
    profile.server_url = normalize_server_url(&profile.server_url);
    profile.api_key = profile.api_key.trim().to_string();
    if profile.server_url.is_empty() || profile.api_key.is_empty() {
        return Err("Server URL and API key cannot be empty".to_string());
    }
    let parsed_url =
        reqwest::Url::parse(&profile.server_url).map_err(|e| format!("Invalid server URL: {e}"))?;
    if !matches!(parsed_url.scheme(), "http" | "https")
        || parsed_url.host_str().is_none_or(|h| h.is_empty())
    {
        return Err("Invalid server URL: must include a valid http/https host".to_string());
    }

    match profile.server_type {
        MediaServerType::Emby => {
            EmbyClient::new(profile.clone())
                .map(|_| ())
                .map_err(|e| sanitize_media_server_error(e, Some(&profile.api_key)))?;
        }
        MediaServerType::Jellyfin => {
            JellyfinClient::new(profile.clone())
                .map(|_| ())
                .map_err(|e| sanitize_media_server_error(e, Some(&profile.api_key)))?;
        }
    }

    let mut config = services
        .config
        .load()?
        .filter(|v| v.is_object())
        .unwrap_or_else(|| serde_json::json!({}));

    let media_servers_val = config
        .get("mediaServers")
        .or_else(|| config.get("media_servers"));
    let has_persisted_servers = media_servers_val.is_some_and(|v| v.is_array());
    let mut profiles: Vec<MediaServerProfile> = media_servers_val
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    if !has_persisted_servers {
        for reg_profile in registry.list_profiles() {
            if !profiles.iter().any(|p| p.id == reg_profile.id) {
                profiles.push(reg_profile);
            }
        }
    }

    let previous_profile = profiles
        .iter()
        .find(|p| p.id == profile.id)
        .cloned()
        .or_else(|| registry.get_profile(&profile.id));

    if let Some(existing) = profiles.iter_mut().find(|p| p.id == profile.id) {
        *existing = profile.clone();
    } else {
        profiles.push(profile.clone());
    }

    let active_id = config
        .get("activeMediaServerId")
        .or_else(|| config.get("active_media_server_id"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| registry.get_active_id())
        .filter(|id| profiles.iter().any(|p| p.id == *id));

    let should_set_active = active_id.is_none() || profiles.len() == 1;
    let next_active_id = if should_set_active {
        profile.id.clone()
    } else {
        active_id.unwrap()
    };

    let legacy_emby_url = config
        .get("embyServerUrl")
        .or_else(|| config.get("emby_server_url"))
        .and_then(|v| v.as_str())
        .map(normalize_server_url)
        .filter(|s| !s.is_empty());

    let to_legacy_cfg = || crate::integrations::emby::EmbyConnectionConfig {
        server_url: profile.server_url.clone(),
        api_key: profile.api_key.clone(),
        username: profile.username.clone(),
        allow_invalid_certs: profile.allow_invalid_certs,
        admin_user_id: profile.admin_user_id.clone(),
    };

    let legacy_action = if profile.id == "default-emby" {
        if profile.server_type == MediaServerType::Emby {
            LegacyEmbySyncAction::Update(to_legacy_cfg())
        } else {
            LegacyEmbySyncAction::Clear
        }
    } else if let Some(ref legacy_url) = legacy_emby_url {
        let prev_matched_legacy = previous_profile.as_ref().is_some_and(|prev| {
            prev.server_type == MediaServerType::Emby
                && normalize_server_url(&prev.server_url).eq_ignore_ascii_case(legacy_url)
        });
        let other_matches_legacy = profiles.iter().any(|p| {
            p.id != profile.id
                && p.server_type == MediaServerType::Emby
                && normalize_server_url(&p.server_url).eq_ignore_ascii_case(legacy_url)
        });
        let curr_matches_legacy = profile.server_type == MediaServerType::Emby
            && normalize_server_url(&profile.server_url).eq_ignore_ascii_case(legacy_url);

        if profile.server_type == MediaServerType::Emby
            && (curr_matches_legacy || (prev_matched_legacy && !other_matches_legacy))
        {
            LegacyEmbySyncAction::Update(to_legacy_cfg())
        } else if prev_matched_legacy && !other_matches_legacy {
            LegacyEmbySyncAction::Clear
        } else {
            LegacyEmbySyncAction::NoChange
        }
    } else {
        LegacyEmbySyncAction::NoChange
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
        match &legacy_action {
            LegacyEmbySyncAction::Update(cfg) => write_legacy_emby_keys(obj, cfg),
            LegacyEmbySyncAction::Clear => clear_legacy_emby_keys(obj),
            LegacyEmbySyncAction::NoChange => {}
        }
    }

    services.config.save(&config)?;

    registry
        .register(profile.clone())
        .map_err(|e| sanitize_media_server_error(e, Some(&profile.api_key)))?;

    if should_set_active {
        let _ = registry.set_active(&next_active_id);
    }

    emit_config_updated(services, &config);
    Ok(legacy_action)
}

pub(crate) fn delete_profile_from_state(
    services: &DesktopServices,
    registry: &MediaServerRegistry,
    profile_id: &str,
) -> Result<bool, String> {
    let mut config = services
        .config
        .load()?
        .filter(|v| v.is_object())
        .unwrap_or_else(|| serde_json::json!({}));

    let media_servers_val = config
        .get("mediaServers")
        .or_else(|| config.get("media_servers"));
    let has_persisted_servers = media_servers_val.is_some_and(|v| v.is_array());
    let mut profiles: Vec<MediaServerProfile> = media_servers_val
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    if !has_persisted_servers {
        for reg_profile in registry.list_profiles() {
            if !profiles.iter().any(|p| p.id == reg_profile.id) {
                profiles.push(reg_profile);
            }
        }
    }

    let deleted_profile = profiles
        .iter()
        .find(|p| p.id == profile_id)
        .cloned()
        .or_else(|| registry.get_profile(profile_id));

    profiles.retain(|p| p.id != profile_id);

    let active_id = config
        .get("activeMediaServerId")
        .or_else(|| config.get("active_media_server_id"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| registry.get_active_id());

    let next_active = match active_id {
        Some(ref id) if id != profile_id && profiles.iter().any(|p| p.id == *id) => {
            Some(id.clone())
        }
        _ => profiles.first().map(|p| p.id.clone()),
    };

    let legacy_emby_url = config
        .get("embyServerUrl")
        .or_else(|| config.get("emby_server_url"))
        .and_then(|v| v.as_str())
        .map(normalize_server_url)
        .filter(|s| !s.is_empty());

    let deleted_matches_legacy_url = deleted_profile.as_ref().is_some_and(|p| {
        p.server_type == MediaServerType::Emby
            && legacy_emby_url.as_ref().is_some_and(|legacy_url| {
                let deleted_url = normalize_server_url(&p.server_url);
                !deleted_url.is_empty()
                    && deleted_url.eq_ignore_ascii_case(legacy_url)
                    && !profiles.iter().any(|rem| {
                        rem.server_type == MediaServerType::Emby
                            && normalize_server_url(&rem.server_url)
                                .eq_ignore_ascii_case(legacy_url)
                    })
            })
    });

    let should_clear_legacy_emby =
        profile_id == "default-emby" || deleted_matches_legacy_url || profiles.is_empty();

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
        if should_clear_legacy_emby {
            clear_legacy_emby_keys(obj);
        }
    }

    services.config.save(&config)?;
    registry.unregister(profile_id);
    if let Some(ref id) = next_active {
        let _ = registry.set_active(id);
    }

    emit_config_updated(services, &config);
    Ok(should_clear_legacy_emby)
}

#[tauri::command(rename_all = "camelCase")]
pub fn media_server_list_profiles(
    services: State<'_, DesktopServices>,
    registry: State<'_, MediaServerRegistry>,
) -> Result<Vec<MediaServerProfile>, String> {
    list_profiles_from_state(&services, &registry)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn media_server_save_profile(
    services: State<'_, DesktopServices>,
    registry: State<'_, MediaServerRegistry>,
    emby_state: State<'_, crate::integrations::emby::EmbyState>,
    profile: MediaServerProfile,
) -> Result<(), String> {
    let legacy_action = save_profile_to_state(&services, &registry, profile)?;
    match legacy_action {
        LegacyEmbySyncAction::Update(cfg) => {
            if let Ok(client) = crate::integrations::emby::EmbyClient::new(cfg) {
                emby_state.set_client(Some(client)).await;
            }
        }
        LegacyEmbySyncAction::Clear => {
            emby_state.set_client(None).await;
        }
        LegacyEmbySyncAction::NoChange => {}
    }
    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn media_server_delete_profile(
    services: State<'_, DesktopServices>,
    registry: State<'_, MediaServerRegistry>,
    emby_state: State<'_, crate::integrations::emby::EmbyState>,
    profile_id: String,
) -> Result<(), String> {
    let should_clear_legacy_emby = delete_profile_from_state(&services, &registry, &profile_id)?;
    if should_clear_legacy_emby {
        emby_state.set_client(None).await;
    }
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
    emit_config_updated(&services, &config);
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

    let file_name = match filename {
        Some(raw) => validate_download_filename(&raw)?,
        None => format!(
            "media_server_{}_{}_{}.mp3",
            clean_id,
            stream_index.unwrap_or(0),
            uuid::Uuid::new_v4()
        ),
    };
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
    allow_overwrite: Option<bool>,
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
    let allow_overwrite = allow_overwrite.unwrap_or(true);

    execute_subtitle_upload(
        client.as_ref(),
        &payload,
        delete_existing_external_index,
        allow_overwrite,
        &api_key,
    )
    .await?;

    let _ = client.refresh_item_metadata(&payload.item_id).await;
    Ok(())
}

async fn execute_subtitle_upload(
    client: &dyn crate::integrations::media_server::MediaServerClient,
    payload: &MediaServerSubtitleUploadPayload,
    delete_existing_external_index: Option<i32>,
    allow_overwrite: bool,
    api_key: &str,
) -> Result<(), String> {
    use crate::integrations::media_server::{
        SubtitleTrackCandidate, unique_duplicate_subtitle_index,
    };

    if let Some(idx) = delete_existing_external_index
        && idx < 0
    {
        return Err("Invalid external subtitle index: cannot be negative".to_string());
    }

    let mut deleted_any = false;
    let mut predelete_auto_error: Option<String> = None;
    if allow_overwrite {
        if let Some(idx) = delete_existing_external_index {
            match client.delete_subtitle(&payload.item_id, idx).await {
                Ok(_) => {
                    deleted_any = true;
                }
                Err(e) => {
                    log::warn!(
                        "Failed to delete existing external subtitle {idx}: {}",
                        sanitize_media_server_error(&e, Some(api_key))
                    );
                }
            }
        } else if !payload.language.eq_ignore_ascii_case("und") {
            match client.get_item_streams(&payload.item_id).await {
                Ok(streams) => {
                    if let Some(idx) = unique_duplicate_subtitle_index(
                        streams.iter().map(|s| SubtitleTrackCandidate {
                            index: s.index,
                            stream_type: &s.stream_type,
                            language: s.language.as_deref(),
                            display_title: s.display_title.as_deref(),
                            is_external: s.is_external,
                            is_forced: s.is_forced,
                        }),
                        &payload.language,
                        payload.is_forced,
                    ) {
                        match client.delete_subtitle(&payload.item_id, idx).await {
                            Ok(_) => {
                                deleted_any = true;
                            }
                            Err(e) => {
                                let sanitized = sanitize_media_server_error(&e, Some(api_key));
                                log::warn!(
                                    "Failed to delete existing external subtitle {idx}: {sanitized}"
                                );
                                predelete_auto_error = Some(sanitized);
                            }
                        }
                    }
                }
                Err(e) => {
                    log::warn!(
                        "Failed to inspect existing streams before subtitle upload for {}: {}",
                        payload.item_id,
                        sanitize_media_server_error(&e, Some(api_key))
                    );
                }
            }
        }
    }

    upload_subtitle_with_duplicate_recovery(
        client,
        payload,
        api_key,
        deleted_any,
        allow_overwrite,
        predelete_auto_error,
    )
    .await
}

async fn upload_subtitle_with_duplicate_recovery(
    client: &dyn crate::integrations::media_server::MediaServerClient,
    payload: &MediaServerSubtitleUploadPayload,
    api_key: &str,
    deleted_any: bool,
    allow_overwrite: bool,
    predelete_auto_error: Option<String>,
) -> Result<(), String> {
    use crate::integrations::media_server::{
        SubtitleTrackCandidate, unique_duplicate_subtitle_index,
    };

    match client.upload_subtitle(payload).await {
        Ok(()) => Ok(()),
        Err(MediaServerError::DuplicateSubtitle) if allow_overwrite && !deleted_any => {
            if let Some(err_msg) = predelete_auto_error {
                return Err(err_msg);
            }
            log::info!(
                "[MediaServer] Duplicate subtitle detected for item {}, attempting auto-recovery",
                payload.item_id
            );
            let streams = client
                .get_item_streams(&payload.item_id)
                .await
                .map_err(|e| sanitize_media_server_error(e, Some(api_key)))?;

            let Some(stream_index) = unique_duplicate_subtitle_index(
                streams.iter().map(|s| SubtitleTrackCandidate {
                    index: s.index,
                    stream_type: &s.stream_type,
                    language: s.language.as_deref(),
                    display_title: s.display_title.as_deref(),
                    is_external: s.is_external,
                    is_forced: s.is_forced,
                }),
                &payload.language,
                payload.is_forced,
            ) else {
                return Err(sanitize_media_server_error(
                    MediaServerError::DuplicateSubtitle,
                    Some(api_key),
                ));
            };

            log::info!(
                "[MediaServer] Auto-recovering: deleting existing external subtitle track index {}",
                stream_index
            );
            client
                .delete_subtitle(&payload.item_id, stream_index)
                .await
                .map_err(|e| sanitize_media_server_error(e, Some(api_key)))?;

            client
                .upload_subtitle(payload)
                .await
                .map_err(|e| sanitize_media_server_error(e, Some(api_key)))
        }
        Err(e) => Err(sanitize_media_server_error(e, Some(api_key))),
    }
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
    emit_config_updated(&services, &current_config);
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

    #[derive(Debug)]
    struct MockDuplicateRecoverClient {
        profile: MediaServerProfile,
        upload_attempts: std::sync::atomic::AtomicUsize,
        deleted_indices: std::sync::Mutex<Vec<i32>>,
        streams: Vec<crate::integrations::media_server::MediaServerStreamInfo>,
    }

    #[async_trait::async_trait]
    impl crate::integrations::media_server::MediaServerClient for MockDuplicateRecoverClient {
        fn profile(&self) -> &MediaServerProfile {
            &self.profile
        }
        async fn test_connection(
            &self,
        ) -> Result<crate::integrations::media_server::MediaServerInfo, MediaServerError> {
            unreachable!()
        }
        async fn get_libraries(&self) -> Result<Vec<MediaServerLibrary>, MediaServerError> {
            unreachable!()
        }
        async fn get_items(
            &self,
            _: Option<&str>,
            _: Option<&str>,
            _: u32,
            _: u32,
        ) -> Result<MediaServerItemListResult, MediaServerError> {
            unreachable!()
        }
        async fn get_item_streams(
            &self,
            _item_id: &str,
        ) -> Result<Vec<crate::integrations::media_server::MediaServerStreamInfo>, MediaServerError>
        {
            Ok(self.streams.clone())
        }
        async fn download_audio_stream(
            &self,
            _: &str,
            _: Option<i32>,
            _: &std::path::Path,
        ) -> Result<u64, MediaServerError> {
            unreachable!()
        }
        async fn upload_subtitle(
            &self,
            _payload: &MediaServerSubtitleUploadPayload,
        ) -> Result<(), MediaServerError> {
            let attempt = self
                .upload_attempts
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if attempt == 0 {
                Err(MediaServerError::DuplicateSubtitle)
            } else {
                Ok(())
            }
        }
        async fn delete_subtitle(
            &self,
            _item_id: &str,
            subtitle_index: i32,
        ) -> Result<(), MediaServerError> {
            self.deleted_indices.lock().unwrap().push(subtitle_index);
            Ok(())
        }
        async fn refresh_item_metadata(&self, _item_id: &str) -> Result<(), MediaServerError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_upload_subtitle_duplicate_auto_recovery() {
        use crate::integrations::media_server::MediaServerStreamInfo;
        let client = MockDuplicateRecoverClient {
            profile: MediaServerProfile {
                id: "emby-1".to_string(),
                name: "Emby".to_string(),
                server_type: MediaServerType::Emby,
                server_url: "http://localhost:8096".to_string(),
                api_key: "key".to_string(),
                username: None,
                admin_user_id: None,
                allow_invalid_certs: false,
                created_at: 1000,
                updated_at: 1000,
            },
            upload_attempts: std::sync::atomic::AtomicUsize::new(0),
            deleted_indices: std::sync::Mutex::new(Vec::new()),
            streams: vec![
                MediaServerStreamInfo {
                    index: 2,
                    stream_type: "Subtitle".to_string(),
                    language: Some("en".to_string()),
                    display_title: Some("English".to_string()),
                    codec: Some("srt".to_string()),
                    is_external: true,
                    is_forced: false,
                    is_default: false,
                },
                MediaServerStreamInfo {
                    index: 5,
                    stream_type: "Subtitle".to_string(),
                    language: Some("chs".to_string()),
                    display_title: Some("简体中文".to_string()),
                    codec: Some("srt".to_string()),
                    is_external: true,
                    is_forced: false,
                    is_default: false,
                },
            ],
        };
        let payload = MediaServerSubtitleUploadPayload {
            item_id: "movie-1".to_string(),
            language: "zh-CN".to_string(),
            format: "srt".to_string(),
            is_forced: false,
            content: "1\n00:00:00,000 --> 00:00:01,000\n你好\n".to_string(),
        };

        upload_subtitle_with_duplicate_recovery(&client, &payload, "key", false, true, None)
            .await
            .expect("duplicate subtitle should auto-recover by deleting matching external track");

        assert_eq!(
            client
                .upload_attempts
                .load(std::sync::atomic::Ordering::SeqCst),
            2
        );
        assert_eq!(*client.deleted_indices.lock().unwrap(), vec![5]);
    }

    #[tokio::test]
    async fn test_upload_subtitle_duplicate_recovery_preserves_ambiguous_chinese_tracks() {
        use crate::integrations::media_server::MediaServerStreamInfo;

        let client = MockDuplicateRecoverClient {
            profile: MediaServerProfile {
                id: "emby-1".to_string(),
                name: "Emby".to_string(),
                server_type: MediaServerType::Emby,
                server_url: "http://localhost:8096".to_string(),
                api_key: "key".to_string(),
                username: None,
                admin_user_id: None,
                allow_invalid_certs: false,
                created_at: 1000,
                updated_at: 1000,
            },
            upload_attempts: std::sync::atomic::AtomicUsize::new(0),
            deleted_indices: std::sync::Mutex::new(Vec::new()),
            streams: vec![
                MediaServerStreamInfo {
                    index: 5,
                    stream_type: "Subtitle".to_string(),
                    language: Some("chs".to_string()),
                    display_title: Some("简体中文".to_string()),
                    codec: Some("srt".to_string()),
                    is_external: true,
                    is_forced: false,
                    is_default: false,
                },
                MediaServerStreamInfo {
                    index: 6,
                    stream_type: "Subtitle".to_string(),
                    language: Some("cht".to_string()),
                    display_title: Some("繁體中文".to_string()),
                    codec: Some("srt".to_string()),
                    is_external: true,
                    is_forced: false,
                    is_default: false,
                },
            ],
        };
        let payload = MediaServerSubtitleUploadPayload {
            item_id: "movie-1".to_string(),
            language: "zh-CN".to_string(),
            format: "srt".to_string(),
            is_forced: false,
            content: "1\n00:00:00,000 --> 00:00:01,000\n你好\n".to_string(),
        };

        let result =
            upload_subtitle_with_duplicate_recovery(&client, &payload, "key", false, true, None)
                .await;

        assert!(result.is_err());
        assert_eq!(
            client
                .upload_attempts
                .load(std::sync::atomic::Ordering::SeqCst),
            1
        );
        assert!(client.deleted_indices.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_upload_subtitle_duplicate_recovery_skipped_when_overwrite_disabled() {
        use crate::integrations::media_server::MediaServerStreamInfo;

        let client = MockDuplicateRecoverClient {
            profile: MediaServerProfile {
                id: "emby-1".to_string(),
                name: "Emby".to_string(),
                server_type: MediaServerType::Emby,
                server_url: "http://localhost:8096".to_string(),
                api_key: "key".to_string(),
                username: None,
                admin_user_id: None,
                allow_invalid_certs: false,
                created_at: 1000,
                updated_at: 1000,
            },
            upload_attempts: std::sync::atomic::AtomicUsize::new(0),
            deleted_indices: std::sync::Mutex::new(Vec::new()),
            streams: vec![MediaServerStreamInfo {
                index: 5,
                stream_type: "Subtitle".to_string(),
                language: Some("chs".to_string()),
                display_title: Some("简体中文".to_string()),
                codec: Some("srt".to_string()),
                is_external: true,
                is_forced: false,
                is_default: false,
            }],
        };
        let payload = MediaServerSubtitleUploadPayload {
            item_id: "movie-1".to_string(),
            language: "zh-CN".to_string(),
            format: "srt".to_string(),
            is_forced: false,
            content: "1\n00:00:00,000 --> 00:00:01,000\n你好\n".to_string(),
        };

        let result =
            upload_subtitle_with_duplicate_recovery(&client, &payload, "key", false, false, None)
                .await;

        assert!(result.is_err());
        assert_eq!(
            client
                .upload_attempts
                .load(std::sync::atomic::Ordering::SeqCst),
            1
        );
        assert!(client.deleted_indices.lock().unwrap().is_empty());
    }

    fn build_test_services(temp_dir: &tempfile::TempDir) -> DesktopServices {
        let ctx = std::sync::Arc::new(
            sona_sqlite::SqliteApplicationContext::open(temp_dir.path()).unwrap(),
        );
        let sqlite = crate::platform::database::DesktopSqliteState::new(ctx);
        let emitter = std::sync::Arc::new(crate::platform::event::MockEventEmitter::new());
        DesktopServices::builder()
            .sqlite(sqlite)
            .event_emitter(emitter)
            .build()
            .unwrap()
    }

    #[test]
    fn test_save_profile_preserves_hydrated_legacy_default_emby() {
        let temp_dir = tempfile::tempdir().unwrap();
        let services = build_test_services(&temp_dir);
        let legacy_config = serde_json::json!({
            "embyServerUrl": "http://legacy-emby.local:8096",
            "embyApiKey": "legacy-key",
            "embyUsername": "admin"
        });
        services.config.save(&legacy_config).unwrap();

        let registry = MediaServerRegistry::new();
        hydrate_media_server_registry(&registry, &legacy_config);
        assert_eq!(registry.get_active_id(), Some("default-emby".to_string()));

        let jellyfin = MediaServerProfile {
            id: "jf-1".to_string(),
            name: "My Jellyfin".to_string(),
            server_type: MediaServerType::Jellyfin,
            server_url: "http://jellyfin.local:8096".to_string(),
            api_key: "jf-key".to_string(),
            username: None,
            admin_user_id: None,
            allow_invalid_certs: false,
            created_at: 2000,
            updated_at: 2000,
        };
        save_profile_to_state(&services, &registry, jellyfin).unwrap();

        let listed = list_profiles_from_state(&services, &registry).unwrap();
        assert_eq!(listed.len(), 2);
        assert!(listed.iter().any(|p| p.id == "default-emby"));
        assert!(listed.iter().any(|p| p.id == "jf-1"));

        let saved_cfg = services.config.load().unwrap().unwrap();
        let persisted_servers: Vec<MediaServerProfile> =
            serde_json::from_value(saved_cfg["mediaServers"].clone()).unwrap();
        assert_eq!(persisted_servers.len(), 2);
        assert_eq!(saved_cfg["activeMediaServerId"], "default-emby");
        assert_eq!(registry.get_active_id(), Some("default-emby".to_string()));
    }

    #[test]
    fn test_delete_default_emby_clears_legacy_config_and_prevents_resurrection() {
        let temp_dir = tempfile::tempdir().unwrap();
        let services = build_test_services(&temp_dir);
        let legacy_config = serde_json::json!({
            "embyServerUrl": "http://legacy-emby.local:8096",
            "embyApiKey": "legacy-key",
            "embyToken": "legacy-token",
            "embyUsername": "admin",
            "embyAdminUserId": "uid-1",
            "embyAllowInvalidCerts": true
        });
        services.config.save(&legacy_config).unwrap();

        let registry = MediaServerRegistry::new();
        hydrate_media_server_registry(&registry, &legacy_config);
        assert_eq!(registry.list_profiles().len(), 1);

        let cleared_legacy =
            delete_profile_from_state(&services, &registry, "default-emby").unwrap();
        assert!(cleared_legacy);
        assert!(registry.list_profiles().is_empty());

        let saved_cfg = services.config.load().unwrap().unwrap();
        assert_eq!(saved_cfg["mediaServers"], serde_json::json!([]));
        assert!(saved_cfg.get("activeMediaServerId").is_none());
        assert!(saved_cfg.get("embyServerUrl").is_none());
        assert!(saved_cfg.get("embyApiKey").is_none());
        assert!(saved_cfg.get("embyToken").is_none());
        assert!(saved_cfg.get("embyUsername").is_none());
        assert!(saved_cfg.get("embyAdminUserId").is_none());
        assert!(saved_cfg.get("embyAllowInvalidCerts").is_none());

        let restarted_registry = MediaServerRegistry::new();
        hydrate_media_server_registry(&restarted_registry, &saved_cfg);
        assert!(restarted_registry.list_profiles().is_empty());
        assert!(
            list_profiles_from_state(&services, &restarted_registry)
                .unwrap()
                .is_empty()
        );
    }

    #[derive(Debug)]
    struct MockSucceedingUploadClient {
        profile: MediaServerProfile,
        upload_attempts: std::sync::atomic::AtomicUsize,
        deleted_indices: std::sync::Mutex<Vec<i32>>,
        streams: Vec<crate::integrations::media_server::MediaServerStreamInfo>,
    }

    #[async_trait::async_trait]
    impl crate::integrations::media_server::MediaServerClient for MockSucceedingUploadClient {
        fn profile(&self) -> &MediaServerProfile {
            &self.profile
        }
        async fn test_connection(
            &self,
        ) -> Result<crate::integrations::media_server::MediaServerInfo, MediaServerError> {
            unreachable!()
        }
        async fn get_libraries(&self) -> Result<Vec<MediaServerLibrary>, MediaServerError> {
            unreachable!()
        }
        async fn get_items(
            &self,
            _: Option<&str>,
            _: Option<&str>,
            _: u32,
            _: u32,
        ) -> Result<MediaServerItemListResult, MediaServerError> {
            unreachable!()
        }
        async fn get_item_streams(
            &self,
            _item_id: &str,
        ) -> Result<Vec<crate::integrations::media_server::MediaServerStreamInfo>, MediaServerError>
        {
            Ok(self.streams.clone())
        }
        async fn download_audio_stream(
            &self,
            _: &str,
            _: Option<i32>,
            _: &std::path::Path,
        ) -> Result<u64, MediaServerError> {
            unreachable!()
        }
        async fn upload_subtitle(
            &self,
            _payload: &MediaServerSubtitleUploadPayload,
        ) -> Result<(), MediaServerError> {
            self.upload_attempts
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
        async fn delete_subtitle(
            &self,
            _item_id: &str,
            subtitle_index: i32,
        ) -> Result<(), MediaServerError> {
            self.deleted_indices.lock().unwrap().push(subtitle_index);
            Ok(())
        }
        async fn refresh_item_metadata(&self, _item_id: &str) -> Result<(), MediaServerError> {
            Ok(())
        }
    }

    fn make_jellyfin_profile() -> MediaServerProfile {
        MediaServerProfile {
            id: "jf-1".to_string(),
            name: "Jellyfin".to_string(),
            server_type: MediaServerType::Jellyfin,
            server_url: "http://localhost:8096".to_string(),
            api_key: "key".to_string(),
            username: None,
            admin_user_id: None,
            allow_invalid_certs: false,
            created_at: 1000,
            updated_at: 1000,
        }
    }

    #[tokio::test]
    async fn test_execute_subtitle_upload_predeletes_unique_external_track_without_index() {
        use crate::integrations::media_server::MediaServerStreamInfo;

        let client = MockSucceedingUploadClient {
            profile: make_jellyfin_profile(),
            upload_attempts: std::sync::atomic::AtomicUsize::new(0),
            deleted_indices: std::sync::Mutex::new(Vec::new()),
            streams: vec![
                MediaServerStreamInfo {
                    index: 2,
                    stream_type: "Subtitle".to_string(),
                    language: Some("en".to_string()),
                    display_title: Some("English".to_string()),
                    codec: Some("srt".to_string()),
                    is_external: true,
                    is_forced: false,
                    is_default: false,
                },
                MediaServerStreamInfo {
                    index: 3,
                    stream_type: "Subtitle".to_string(),
                    language: Some("zh".to_string()),
                    display_title: Some("Chinese".to_string()),
                    codec: Some("srt".to_string()),
                    is_external: true,
                    is_forced: false,
                    is_default: false,
                },
            ],
        };
        let payload = MediaServerSubtitleUploadPayload {
            item_id: "movie-1".to_string(),
            language: "en".to_string(),
            format: "srt".to_string(),
            is_forced: false,
            content: "1\n00:00:00,000 --> 00:00:01,000\nHello\n".to_string(),
        };

        execute_subtitle_upload(&client, &payload, None, true, "key")
            .await
            .unwrap();

        assert_eq!(*client.deleted_indices.lock().unwrap(), vec![2]);
        assert_eq!(
            client
                .upload_attempts
                .load(std::sync::atomic::Ordering::SeqCst),
            1
        );
    }

    #[tokio::test]
    async fn test_execute_subtitle_upload_skips_predelete_for_ambiguous_und_or_no_overwrite() {
        use crate::integrations::media_server::MediaServerStreamInfo;

        let make_client = || MockSucceedingUploadClient {
            profile: make_jellyfin_profile(),
            upload_attempts: std::sync::atomic::AtomicUsize::new(0),
            deleted_indices: std::sync::Mutex::new(Vec::new()),
            streams: vec![
                MediaServerStreamInfo {
                    index: 5,
                    stream_type: "Subtitle".to_string(),
                    language: Some("chs".to_string()),
                    display_title: Some("简体中文".to_string()),
                    codec: Some("srt".to_string()),
                    is_external: true,
                    is_forced: false,
                    is_default: false,
                },
                MediaServerStreamInfo {
                    index: 6,
                    stream_type: "Subtitle".to_string(),
                    language: Some("cht".to_string()),
                    display_title: Some("繁體中文".to_string()),
                    codec: Some("srt".to_string()),
                    is_external: true,
                    is_forced: false,
                    is_default: false,
                },
                MediaServerStreamInfo {
                    index: 7,
                    stream_type: "Subtitle".to_string(),
                    language: Some("en".to_string()),
                    display_title: Some("English".to_string()),
                    codec: Some("srt".to_string()),
                    is_external: true,
                    is_forced: false,
                    is_default: false,
                },
            ],
        };

        // Ambiguous Chinese tracks (chs + cht)
        let ambiguous_client = make_client();
        let zh_payload = MediaServerSubtitleUploadPayload {
            item_id: "movie-1".to_string(),
            language: "zh-CN".to_string(),
            format: "srt".to_string(),
            is_forced: false,
            content: "1\n00:00:00,000 --> 00:00:01,000\n你好\n".to_string(),
        };
        execute_subtitle_upload(&ambiguous_client, &zh_payload, None, true, "key")
            .await
            .unwrap();
        assert!(ambiguous_client.deleted_indices.lock().unwrap().is_empty());
        assert_eq!(
            ambiguous_client
                .upload_attempts
                .load(std::sync::atomic::Ordering::SeqCst),
            1
        );

        // language == "und"
        let und_client = make_client();
        let und_payload = MediaServerSubtitleUploadPayload {
            item_id: "movie-1".to_string(),
            language: "und".to_string(),
            format: "srt".to_string(),
            is_forced: false,
            content: "1\n00:00:00,000 --> 00:00:01,000\nHello\n".to_string(),
        };
        execute_subtitle_upload(&und_client, &und_payload, None, true, "key")
            .await
            .unwrap();
        assert!(und_client.deleted_indices.lock().unwrap().is_empty());
        assert_eq!(
            und_client
                .upload_attempts
                .load(std::sync::atomic::Ordering::SeqCst),
            1
        );

        // allow_overwrite == false
        let no_overwrite_client = make_client();
        let en_payload = MediaServerSubtitleUploadPayload {
            item_id: "movie-1".to_string(),
            language: "en".to_string(),
            format: "srt".to_string(),
            is_forced: false,
            content: "1\n00:00:00,000 --> 00:00:01,000\nHello\n".to_string(),
        };
        execute_subtitle_upload(&no_overwrite_client, &en_payload, None, false, "key")
            .await
            .unwrap();
        assert!(
            no_overwrite_client
                .deleted_indices
                .lock()
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            no_overwrite_client
                .upload_attempts
                .load(std::sync::atomic::Ordering::SeqCst),
            1
        );
    }

    #[test]
    fn test_save_profile_updates_legacy_emby_config_for_default_emby() {
        let temp_dir = tempfile::tempdir().unwrap();
        let services = build_test_services(&temp_dir);
        let initial_config = serde_json::json!({
            "embyServerUrl": "http://old-emby.local:8096",
            "embyApiKey": "old-key",
            "embyUsername": "old-user"
        });
        services.config.save(&initial_config).unwrap();

        let registry = MediaServerRegistry::new();
        hydrate_media_server_registry(&registry, &initial_config);

        let updated_default = MediaServerProfile {
            id: "default-emby".to_string(),
            name: "Updated Emby".to_string(),
            server_type: MediaServerType::Emby,
            server_url: "http://new-emby.local:8096/".to_string(),
            api_key: "new-key".to_string(),
            username: Some("new-admin".to_string()),
            admin_user_id: Some("uid-99".to_string()),
            allow_invalid_certs: true,
            created_at: 1000,
            updated_at: 2000,
        };

        let action = save_profile_to_state(&services, &registry, updated_default).unwrap();
        assert_eq!(
            action,
            LegacyEmbySyncAction::Update(crate::integrations::emby::EmbyConnectionConfig {
                server_url: "http://new-emby.local:8096".to_string(),
                api_key: "new-key".to_string(),
                username: Some("new-admin".to_string()),
                allow_invalid_certs: true,
                admin_user_id: Some("uid-99".to_string()),
            })
        );

        let saved_cfg = services.config.load().unwrap().unwrap();
        assert_eq!(saved_cfg["embyServerUrl"], "http://new-emby.local:8096");
        assert_eq!(saved_cfg["embyApiKey"], "new-key");
        assert_eq!(saved_cfg["embyUsername"], "new-admin");
        assert_eq!(saved_cfg["embyAdminUserId"], "uid-99");
        assert_eq!(saved_cfg["embyAllowInvalidCerts"], true);
    }

    #[test]
    fn test_save_profile_invalid_url_does_not_persist() {
        let temp_dir = tempfile::tempdir().unwrap();
        let services = build_test_services(&temp_dir);
        let registry = MediaServerRegistry::new();

        let invalid_profile = MediaServerProfile {
            id: "bad-1".to_string(),
            name: "Bad Server".to_string(),
            server_type: MediaServerType::Emby,
            server_url: "http://invalid host name".to_string(),
            api_key: "key".to_string(),
            username: None,
            admin_user_id: None,
            allow_invalid_certs: false,
            created_at: 1000,
            updated_at: 1000,
        };

        let err = save_profile_to_state(&services, &registry, invalid_profile).unwrap_err();
        assert!(err.contains("Invalid server URL"));
        assert!(services.config.load().unwrap().is_none());
        assert!(registry.list_profiles().is_empty());
    }

    #[test]
    fn test_delete_last_profile_clears_orphaned_legacy_emby_config() {
        let temp_dir = tempfile::tempdir().unwrap();
        let services = build_test_services(&temp_dir);
        let jf = make_jellyfin_profile();
        let initial_config = serde_json::json!({
            "mediaServers": [jf.clone()],
            "activeMediaServerId": "jf-1",
            "embyServerUrl": "http://orphan-emby.local:8096",
            "embyApiKey": "orphan-key"
        });
        services.config.save(&initial_config).unwrap();

        let registry = MediaServerRegistry::new();
        hydrate_media_server_registry(&registry, &initial_config);

        let cleared = delete_profile_from_state(&services, &registry, "jf-1").unwrap();
        assert!(cleared);
        let saved_cfg = services.config.load().unwrap().unwrap();
        assert_eq!(saved_cfg["mediaServers"], serde_json::json!([]));
        assert!(saved_cfg.get("embyServerUrl").is_none());
        assert!(saved_cfg.get("embyApiKey").is_none());
    }

    #[test]
    fn test_validate_download_filename_rejects_traversal() {
        assert_eq!(
            validate_download_filename("track_01.mp3").unwrap(),
            "track_01.mp3"
        );
        for bad in [
            "",
            "   ",
            ".",
            "..",
            "../evil.mp3",
            "..\\evil.mp3",
            "sub/track.mp3",
            "sub\\track.mp3",
            "/etc/passwd",
        ] {
            assert!(
                validate_download_filename(bad).is_err(),
                "expected '{bad}' to be rejected"
            );
        }
    }

    #[tokio::test]
    async fn test_execute_subtitle_upload_rejects_negative_index() {
        let client = MockSucceedingUploadClient {
            profile: make_jellyfin_profile(),
            upload_attempts: std::sync::atomic::AtomicUsize::new(0),
            deleted_indices: std::sync::Mutex::new(Vec::new()),
            streams: vec![],
        };
        let payload = MediaServerSubtitleUploadPayload {
            item_id: "movie-1".to_string(),
            language: "en".to_string(),
            format: "srt".to_string(),
            is_forced: false,
            content: "1\n00:00:00,000 --> 00:00:01,000\nHello\n".to_string(),
        };

        let err = execute_subtitle_upload(&client, &payload, Some(-1), true, "key")
            .await
            .unwrap_err();
        assert!(err.contains("cannot be negative"));
        assert_eq!(
            client
                .upload_attempts
                .load(std::sync::atomic::Ordering::SeqCst),
            0
        );
    }

    #[tokio::test]
    async fn test_execute_subtitle_upload_does_not_retry_failed_auto_predelete_on_duplicate() {
        use crate::integrations::media_server::MediaServerStreamInfo;

        #[derive(Debug)]
        struct MockFailingDeleteClient {
            profile: MediaServerProfile,
            get_streams_calls: std::sync::atomic::AtomicUsize,
            delete_calls: std::sync::atomic::AtomicUsize,
            upload_calls: std::sync::atomic::AtomicUsize,
            streams: Vec<MediaServerStreamInfo>,
        }

        #[async_trait::async_trait]
        impl crate::integrations::media_server::MediaServerClient for MockFailingDeleteClient {
            fn profile(&self) -> &MediaServerProfile {
                &self.profile
            }
            async fn test_connection(
                &self,
            ) -> Result<crate::integrations::media_server::MediaServerInfo, MediaServerError>
            {
                unreachable!()
            }
            async fn get_libraries(&self) -> Result<Vec<MediaServerLibrary>, MediaServerError> {
                unreachable!()
            }
            async fn get_items(
                &self,
                _: Option<&str>,
                _: Option<&str>,
                _: u32,
                _: u32,
            ) -> Result<MediaServerItemListResult, MediaServerError> {
                unreachable!()
            }
            async fn get_item_streams(
                &self,
                _: &str,
            ) -> Result<Vec<MediaServerStreamInfo>, MediaServerError> {
                self.get_streams_calls
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(self.streams.clone())
            }
            async fn download_audio_stream(
                &self,
                _: &str,
                _: Option<i32>,
                _: &std::path::Path,
            ) -> Result<u64, MediaServerError> {
                unreachable!()
            }
            async fn upload_subtitle(
                &self,
                _: &MediaServerSubtitleUploadPayload,
            ) -> Result<(), MediaServerError> {
                self.upload_calls
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Err(MediaServerError::DuplicateSubtitle)
            }
            async fn delete_subtitle(&self, _: &str, _: i32) -> Result<(), MediaServerError> {
                self.delete_calls
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Err(MediaServerError::ServerError(
                    "Delete denied (403)".to_string(),
                ))
            }
            async fn refresh_item_metadata(&self, _: &str) -> Result<(), MediaServerError> {
                Ok(())
            }
        }

        let client = MockFailingDeleteClient {
            profile: make_jellyfin_profile(),
            get_streams_calls: std::sync::atomic::AtomicUsize::new(0),
            delete_calls: std::sync::atomic::AtomicUsize::new(0),
            upload_calls: std::sync::atomic::AtomicUsize::new(0),
            streams: vec![MediaServerStreamInfo {
                index: 5,
                stream_type: "Subtitle".to_string(),
                language: Some("en".to_string()),
                display_title: Some("English".to_string()),
                codec: Some("srt".to_string()),
                is_external: true,
                is_forced: false,
                is_default: false,
            }],
        };
        let payload = MediaServerSubtitleUploadPayload {
            item_id: "movie-1".to_string(),
            language: "en".to_string(),
            format: "srt".to_string(),
            is_forced: false,
            content: "1\n00:00:00,000 --> 00:00:01,000\nHello\n".to_string(),
        };

        let err = execute_subtitle_upload(&client, &payload, None, true, "key")
            .await
            .unwrap_err();
        assert!(err.contains("Delete denied"));
        assert_eq!(
            client
                .get_streams_calls
                .load(std::sync::atomic::Ordering::SeqCst),
            1
        );
        assert_eq!(
            client
                .delete_calls
                .load(std::sync::atomic::Ordering::SeqCst),
            1
        );
        assert_eq!(
            client
                .upload_calls
                .load(std::sync::atomic::Ordering::SeqCst),
            1
        );
    }
}
