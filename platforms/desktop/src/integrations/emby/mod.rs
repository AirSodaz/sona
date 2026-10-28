pub mod client;
pub mod types;

use std::sync::Arc;
use tokio::sync::RwLock;

pub use client::EmbyClient;
pub use types::*;

#[derive(Clone, Default)]
pub struct EmbyState {
    pub client: Arc<RwLock<Option<EmbyClient>>>,
}

impl EmbyState {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn set_client(&self, client: EmbyClient) {
        let mut guard = self.client.write().await;
        *guard = Some(client);
    }

    pub async fn get_client(&self) -> Result<EmbyClient, EmbyError> {
        let guard = self.client.read().await;
        guard
            .as_ref()
            .cloned()
            .ok_or_else(|| EmbyError::Network("Emby client is not connected".to_string()))
    }
}
