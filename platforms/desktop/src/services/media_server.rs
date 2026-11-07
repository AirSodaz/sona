use crate::integrations::media_server::{
    EmbyClient, JellyfinClient, MediaServerClient, MediaServerError, MediaServerInfo,
    MediaServerItemListResult, MediaServerLibrary, MediaServerLoginPayload, MediaServerLoginResult,
    MediaServerProfile, MediaServerRegistry, MediaServerSubtitleUploadPayload, MediaServerType,
    normalize_server_url,
};
use crate::platform::app_config::AppConfigService;
use std::sync::Arc;

#[derive(Clone)]
pub struct MediaServerService {
    config: Arc<AppConfigService>,
    registry: MediaServerRegistry,
}

impl MediaServerService {
    pub fn new(config: Arc<AppConfigService>) -> Self {
        let service = Self {
            config,
            registry: MediaServerRegistry::new(),
        };
        service.restore_profiles();
        service
    }

    fn restore_profiles(&self) {
        let Ok(Some(config)) = self.config.load() else {
            return;
        };
        for profile in read_profiles(&config) {
            if let Err(error) = self.registry.register(profile) {
                log::warn!("Skipping invalid media server profile: {error}");
            }
        }
        if let Some(active_id) = config
            .get("activeMediaServerId")
            .and_then(serde_json::Value::as_str)
        {
            let _ = self.registry.set_active(active_id);
        }
    }

    pub fn list_profiles(&self) -> Result<Vec<MediaServerProfile>, String> {
        let profiles = self
            .config
            .load()?
            .map(|config| read_profiles(&config))
            .unwrap_or_default();
        Ok(if profiles.is_empty() {
            self.registry.list_profiles()
        } else {
            profiles
        })
    }

    pub fn active_profile_id(&self) -> Option<String> {
        self.registry.get_active_id()
    }

    pub async fn test_connection(
        &self,
        profile: MediaServerProfile,
    ) -> Result<MediaServerInfo, String> {
        let client = build_client(normalize_profile(profile)?)?;
        let api_key = client.profile().api_key.clone();
        client
            .test_connection()
            .await
            .map_err(|error| sanitize_error(error, &api_key))
    }

    pub async fn login(
        &self,
        mut payload: MediaServerLoginPayload,
    ) -> Result<MediaServerLoginResult, String> {
        payload.server_url = normalize_server_url(&payload.server_url);
        payload.username = payload.username.trim().to_string();
        if payload.server_url.is_empty() || payload.username.is_empty() {
            return Err("Server URL and username cannot be empty".to_string());
        }
        let result = match payload.server_type {
            MediaServerType::Emby => {
                EmbyClient::authenticate_by_name(
                    &payload.server_url,
                    &payload.username,
                    payload.password.as_deref().unwrap_or_default(),
                    payload.allow_invalid_certs,
                )
                .await
            }
            MediaServerType::Jellyfin => {
                JellyfinClient::authenticate_by_name(
                    &payload.server_url,
                    &payload.username,
                    payload.password.as_deref().unwrap_or_default(),
                    payload.allow_invalid_certs,
                )
                .await
            }
        };
        result.map_err(|error| match error {
            MediaServerError::Unauthorized => "Invalid username or password".to_string(),
            other => sanitize_error(other, ""),
        })
    }

    pub fn save_profile(&self, profile: MediaServerProfile) -> Result<(), String> {
        let profile = normalize_profile(profile)?;
        let mut config = self
            .config
            .load()?
            .filter(serde_json::Value::is_object)
            .unwrap_or_else(|| serde_json::json!({}));
        let mut profiles = read_profiles(&config);
        if let Some(existing) = profiles.iter_mut().find(|item| item.id == profile.id) {
            *existing = profile.clone();
        } else {
            profiles.push(profile.clone());
        }
        let current_active = config
            .get("activeMediaServerId")
            .and_then(serde_json::Value::as_str);
        let next_active_id = current_active
            .filter(|id| profiles.iter().any(|item| item.id == *id))
            .unwrap_or(&profile.id)
            .to_string();
        let object = config
            .as_object_mut()
            .expect("configuration was normalized to an object");
        object.insert(
            "mediaServers".into(),
            serde_json::to_value(&profiles).map_err(|e| e.to_string())?,
        );
        object.insert("activeMediaServerId".into(), next_active_id.clone().into());
        self.config.save(&config)?;
        self.registry
            .register(profile)
            .map_err(|error| error.to_string())?;
        let _ = self.registry.set_active(&next_active_id);
        Ok(())
    }

    pub fn delete_profile(&self, profile_id: &str) -> Result<(), String> {
        let mut config = self
            .config
            .load()?
            .filter(serde_json::Value::is_object)
            .unwrap_or_else(|| serde_json::json!({}));
        let mut profiles = read_profiles(&config);
        profiles.retain(|profile| profile.id != profile_id);
        let current_active = config
            .get("activeMediaServerId")
            .and_then(serde_json::Value::as_str);
        let next_active = if current_active == Some(profile_id) {
            profiles.first().map(|profile| profile.id.clone())
        } else {
            current_active.map(str::to_string)
        };
        let object = config
            .as_object_mut()
            .expect("configuration was normalized to an object");
        object.insert(
            "mediaServers".into(),
            serde_json::to_value(profiles).map_err(|e| e.to_string())?,
        );
        if let Some(id) = next_active {
            object.insert("activeMediaServerId".into(), id.into());
        } else {
            object.remove("activeMediaServerId");
        }
        self.config.save(&config)?;
        self.registry.unregister(profile_id);
        if let Some(active_id) = config
            .get("activeMediaServerId")
            .and_then(serde_json::Value::as_str)
        {
            let _ = self.registry.set_active(active_id);
        }
        Ok(())
    }

    pub fn set_active_profile(&self, profile_id: &str) -> Result<(), String> {
        let profiles = self.list_profiles()?;
        if !profiles.iter().any(|profile| profile.id == profile_id) {
            return Err(format!("Media server profile '{profile_id}' not found"));
        }
        let mut config = self
            .config
            .load()?
            .filter(serde_json::Value::is_object)
            .unwrap_or_else(|| serde_json::json!({}));
        config["activeMediaServerId"] = profile_id.into();
        self.config.save(&config)?;
        if self.registry.get(Some(profile_id)).is_err() {
            let profile = profiles
                .into_iter()
                .find(|profile| profile.id == profile_id)
                .unwrap();
            self.registry
                .register(profile)
                .map_err(|error| error.to_string())?;
        }
        self.registry
            .set_active(profile_id)
            .map_err(|error| error.to_string())
    }

    pub async fn libraries(
        &self,
        profile_id: Option<&str>,
    ) -> Result<Vec<MediaServerLibrary>, String> {
        let client = self
            .registry
            .get(profile_id)
            .map_err(|error| error.to_string())?;
        let api_key = client.profile().api_key.clone();
        client
            .get_libraries()
            .await
            .map_err(|error| sanitize_error(error, &api_key))
    }

    pub async fn items(
        &self,
        profile_id: Option<&str>,
        parent_id: Option<&str>,
        search: Option<&str>,
        offset: u32,
        limit: u32,
    ) -> Result<MediaServerItemListResult, String> {
        let client = self
            .registry
            .get(profile_id)
            .map_err(|error| error.to_string())?;
        let api_key = client.profile().api_key.clone();
        client
            .get_items(parent_id, search, limit, offset)
            .await
            .map_err(|error| sanitize_error(error, &api_key))
    }

    pub async fn download_audio(
        &self,
        profile_id: Option<&str>,
        item_id: &str,
        stream_index: Option<i32>,
        destination_path: &std::path::Path,
    ) -> Result<u64, String> {
        let client = self
            .registry
            .get(profile_id)
            .map_err(|error| error.to_string())?;
        let api_key = client.profile().api_key.clone();
        client
            .download_audio_stream(item_id, stream_index, destination_path)
            .await
            .map_err(|error| sanitize_error(error, &api_key))
    }

    pub async fn upload_subtitle(
        &self,
        profile_id: Option<&str>,
        payload: MediaServerSubtitleUploadPayload,
        delete_existing_index: Option<i32>,
    ) -> Result<(), String> {
        let client = self
            .registry
            .get(profile_id)
            .map_err(|error| error.to_string())?;
        let api_key = client.profile().api_key.clone();
        if let Some(index) = delete_existing_index {
            client
                .delete_subtitle(&payload.item_id, index)
                .await
                .map_err(|error| sanitize_error(error, &api_key))?;
        }
        client
            .upload_subtitle(&payload)
            .await
            .map_err(|error| sanitize_error(error, &api_key))?;
        client
            .refresh_item_metadata(&payload.item_id)
            .await
            .map_err(|error| sanitize_error(error, &api_key))
    }
}

fn read_profiles(config: &serde_json::Value) -> Vec<MediaServerProfile> {
    config
        .get("mediaServers")
        .or_else(|| config.get("media_servers"))
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or_default()
}

fn normalize_profile(mut profile: MediaServerProfile) -> Result<MediaServerProfile, String> {
    profile.server_url = normalize_server_url(&profile.server_url);
    profile.api_key = profile.api_key.trim().to_string();
    if profile.server_url.is_empty() || profile.api_key.is_empty() {
        return Err("Server URL and API key cannot be empty".to_string());
    }
    Ok(profile)
}

fn build_client(profile: MediaServerProfile) -> Result<Arc<dyn MediaServerClient>, String> {
    match profile.server_type {
        MediaServerType::Emby => EmbyClient::new(profile)
            .map(|client| Arc::new(client) as Arc<dyn MediaServerClient>)
            .map_err(|error| error.to_string()),
        MediaServerType::Jellyfin => JellyfinClient::new(profile)
            .map(|client| Arc::new(client) as Arc<dyn MediaServerClient>)
            .map_err(|error| error.to_string()),
    }
}

fn sanitize_error(error: impl std::fmt::Display, api_key: &str) -> String {
    let mut message = error.to_string();
    if !api_key.is_empty() {
        message = message.replace(api_key, "[REDACTED]");
    }
    if let Some(index) = message.find("api_key=") {
        let end = message[index..]
            .find(['&', ' ', '"', '\'', '\n', '\r'])
            .map(|offset| index + offset)
            .unwrap_or(message.len());
        message.replace_range(index + 8..end, "[REDACTED]");
    }
    message
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::database::DesktopSqliteState;

    fn service() -> (tempfile::TempDir, MediaServerService) {
        let temp = tempfile::tempdir().unwrap();
        let context = Arc::new(sona_sqlite::SqliteApplicationContext::open(temp.path()).unwrap());
        let config = Arc::new(AppConfigService::new(DesktopSqliteState::new(context)));
        (temp, MediaServerService::new(config))
    }

    fn profile(id: &str, server_type: MediaServerType) -> MediaServerProfile {
        MediaServerProfile {
            id: id.to_string(),
            name: format!("{id} server"),
            server_type,
            server_url: "localhost:8096/".to_string(),
            api_key: format!("key-{id}"),
            username: Some("admin".to_string()),
            admin_user_id: None,
            allow_invalid_certs: false,
            created_at: 1,
            updated_at: 1,
        }
    }

    #[test]
    fn profile_save_select_and_delete_preserve_active_profile() {
        let (_temp, service) = service();
        let first = profile("emby-one", MediaServerType::Emby);
        let second = profile("jellyfin-two", MediaServerType::Jellyfin);

        service.save_profile(first.clone()).unwrap();
        service.save_profile(second.clone()).unwrap();
        assert_eq!(service.active_profile_id(), Some(first.id.clone()));
        assert_eq!(service.list_profiles().unwrap().len(), 2);

        service.set_active_profile(&second.id).unwrap();
        assert_eq!(service.active_profile_id(), Some(second.id.clone()));
        service.delete_profile(&second.id).unwrap();
        assert_eq!(service.active_profile_id(), Some(first.id.clone()));
        let profiles = service.list_profiles().unwrap();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].id, first.id);
        assert_eq!(profiles[0].server_url, "http://localhost:8096");
    }

    #[test]
    fn profile_validation_normalizes_server_url_and_rejects_missing_credentials() {
        let normalized = normalize_profile(profile("one", MediaServerType::Emby)).unwrap();
        assert_eq!(normalized.server_url, "http://localhost:8096");

        let mut invalid = profile("one", MediaServerType::Emby);
        invalid.api_key.clear();
        assert!(normalize_profile(invalid).is_err());
    }

    #[test]
    fn error_sanitization_redacts_credentials_in_messages_and_urls() {
        let message = sanitize_error(
            "request to /items?api_key=secret&other=1 failed with secret",
            "secret",
        );
        assert!(!message.contains("secret"));
        assert!(message.contains("api_key=[REDACTED]"));
    }
}
