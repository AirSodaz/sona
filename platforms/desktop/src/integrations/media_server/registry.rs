use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

use super::emby::EmbyClient;
use super::jellyfin::JellyfinClient;
use super::traits::MediaServerClient;
use super::types::{MediaServerError, MediaServerProfile, MediaServerType};

#[derive(Clone, Default)]
pub struct MediaServerRegistry {
    clients: Arc<RwLock<HashMap<String, Arc<dyn MediaServerClient>>>>,
    active_server_id: Arc<RwLock<Option<String>>>,
}

impl MediaServerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(
        &self,
        server_id: Option<&str>,
    ) -> Result<Arc<dyn MediaServerClient>, MediaServerError> {
        let clients = self.clients.read();

        if let Some(id) = server_id {
            let trimmed = id.trim();
            if trimmed.is_empty() {
                return Err(MediaServerError::InvalidConfig(
                    "Server ID cannot be empty".to_string(),
                ));
            }
            return clients
                .get(trimmed)
                .cloned()
                .ok_or_else(|| MediaServerError::ServerNotFound(trimmed.to_string()));
        }

        let active_id_guard = self.active_server_id.read();
        let active_id = active_id_guard.as_deref().ok_or_else(|| {
            MediaServerError::InvalidConfig("No active media server selected".to_string())
        })?;

        clients
            .get(active_id)
            .cloned()
            .ok_or_else(|| MediaServerError::ServerNotFound(active_id.to_string()))
    }

    pub fn register(&self, profile: MediaServerProfile) -> Result<(), MediaServerError> {
        let profile_id = profile.id.clone();
        let client: Arc<dyn MediaServerClient> = match profile.server_type {
            MediaServerType::Emby => Arc::new(EmbyClient::new(profile)?),
            MediaServerType::Jellyfin => Arc::new(JellyfinClient::new(profile)?),
        };

        let mut clients = self.clients.write();
        clients.insert(profile_id.clone(), client);

        let mut active_id = self.active_server_id.write();
        if active_id.is_none() {
            *active_id = Some(profile_id);
        }

        Ok(())
    }

    pub fn unregister(&self, profile_id: &str) {
        let mut clients = self.clients.write();
        clients.remove(profile_id);

        let mut active_id = self.active_server_id.write();
        if active_id.as_deref() == Some(profile_id) {
            *active_id = clients.keys().next().cloned();
        }
    }

    pub fn set_active(&self, profile_id: &str) -> Result<(), MediaServerError> {
        let clients = self.clients.read();
        if !clients.contains_key(profile_id) {
            return Err(MediaServerError::ServerNotFound(profile_id.to_string()));
        }
        drop(clients);

        let mut active_id = self.active_server_id.write();
        *active_id = Some(profile_id.to_string());
        Ok(())
    }

    pub fn get_active_id(&self) -> Option<String> {
        self.active_server_id.read().clone()
    }

    pub fn list_profiles(&self) -> Vec<MediaServerProfile> {
        let clients = self.clients.read();
        clients.values().map(|c| c.profile().clone()).collect()
    }

    pub fn get_profile(&self, profile_id: &str) -> Option<MediaServerProfile> {
        let clients = self.clients.read();
        clients.get(profile_id).map(|c| c.profile().clone())
    }

    pub fn clear(&self) {
        let mut clients = self.clients.write();
        clients.clear();
        let mut active_id = self.active_server_id.write();
        *active_id = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_media_server_registry_strict_lookup_error() {
        let registry = MediaServerRegistry::new();
        let profile = MediaServerProfile {
            id: "emby-default".to_string(),
            name: "Default Emby".to_string(),
            server_type: MediaServerType::Emby,
            server_url: "http://localhost:8096".to_string(),
            api_key: "key1".to_string(),
            username: None,
            admin_user_id: None,
            allow_invalid_certs: false,
            created_at: 1000,
            updated_at: 1000,
        };

        registry.register(profile).unwrap();
        assert_eq!(registry.get_active_id(), Some("emby-default".to_string()));

        // Active server lookup succeeds
        let active_client = registry.get(None);
        assert!(active_client.is_ok());
        assert_eq!(active_client.unwrap().profile().id, "emby-default");

        // Requesting unknown server_id strictly returns ServerNotFound and DOES NOT fallback to active server
        let res = registry.get(Some("non-existent-id"));
        match res {
            Err(MediaServerError::ServerNotFound(id)) => {
                assert_eq!(id, "non-existent-id");
            }
            other => panic!("Expected ServerNotFound error, got {:?}", other),
        }
    }
}
