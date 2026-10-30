pub mod client;
pub mod types;

use std::sync::Arc;
use tokio::sync::RwLock;

pub use client::EmbyClient;
pub use types::*;

#[derive(Clone, Default)]
pub struct EmbyState {
    client: Arc<RwLock<Option<EmbyClient>>>,
}

impl EmbyState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_client(client: EmbyClient) -> Self {
        Self {
            client: Arc::new(RwLock::new(Some(client))),
        }
    }

    pub async fn set_client(&self, client: Option<EmbyClient>) {
        let mut guard = self.client.write().await;
        *guard = client;
    }

    pub async fn clear_client(&self) {
        let mut guard = self.client.write().await;
        *guard = None;
    }

    pub async fn get_client(&self) -> Result<EmbyClient, EmbyError> {
        let guard = self.client.read().await;
        guard
            .as_ref()
            .cloned()
            .ok_or_else(|| EmbyError::InvalidConfig("Emby client is not connected".to_string()))
    }
}
