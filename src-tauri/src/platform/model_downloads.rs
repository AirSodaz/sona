use sona_model_downloads::DownloadClient;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};

const DOWNLOAD_PROGRESS_EVENT: &str = "download-progress";

pub struct DownloadState {
    downloads: Mutex<HashMap<String, Arc<Notify>>>,
    client: DownloadClient,
}

impl Default for DownloadState {
    fn default() -> Self {
        Self::new()
    }
}

impl DownloadState {
    pub fn new() -> Self {
        Self {
            downloads: Mutex::new(HashMap::new()),
            client: DownloadClient::new(),
        }
    }

    pub(crate) fn client(&self) -> &DownloadClient {
        &self.client
    }

    pub(crate) async fn insert_download(&self, id: String, notify: Arc<Notify>) {
        self.downloads.lock().await.insert(id, notify);
    }

    pub(crate) async fn remove_download(&self, id: &str) -> Option<Arc<Notify>> {
        self.downloads.lock().await.remove(id)
    }

    pub(crate) async fn notify_download(&self, id: &str) {
        if let Some(notify) = self.notify_for_download(id).await {
            notify.notify_one();
        }
    }

    pub(crate) async fn has_active_downloads(&self) -> bool {
        !self.downloads.lock().await.is_empty()
    }

    async fn notify_for_download(&self, id: &str) -> Option<Arc<Notify>> {
        self.downloads.lock().await.get(id).cloned()
    }
}

pub async fn cancel_download(
    state: tauri::State<'_, DownloadState>,
    id: String,
) -> Result<(), String> {
    state.notify_download(&id).await;
    Ok(())
}

pub async fn has_active_downloads(state: tauri::State<'_, DownloadState>) -> Result<bool, String> {
    Ok(state.has_active_downloads().await)
}

pub async fn download_file<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, DownloadState>,
    url: String,
    output_path: String,
    id: String,
    expected_sha256: Option<String>,
) -> Result<(), String> {
    use sona_model_downloads::{complete_download_file, temporary_download_path};
    use tauri::Emitter;

    let final_path = std::path::PathBuf::from(&output_path);
    let temp_path = temporary_download_path(&final_path);

    let notify = Arc::new(Notify::new());
    state.insert_download(id.clone(), notify.clone()).await;

    let app_clone = app.clone();
    let id_clone = id.clone();
    let mut last_emit = std::time::Instant::now();
    let progress_cb = Box::new(move |downloaded: u64, total: u64| {
        if downloaded == total || last_emit.elapsed().as_millis() >= 100 {
            let _ = app_clone.emit(DOWNLOAD_PROGRESS_EVENT, (downloaded, total, &id_clone));
            last_emit = std::time::Instant::now();
        }
    });

    let result = state
        .client()
        .download_file(&url, &temp_path, notify, Some(progress_cb))
        .await;

    state.remove_download(&id).await;

    match result {
        Ok(()) => complete_download_file(&temp_path, &final_path, expected_sha256.as_deref())
            .await
            .map_err(|error| error.to_string()),
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn download_state_tracks_active_downloads_by_id() {
        let state = DownloadState::new();
        let notify = Arc::new(Notify::new());

        assert!(!state.has_active_downloads().await);

        state
            .insert_download("model-a".to_string(), notify.clone())
            .await;

        assert!(state.has_active_downloads().await);
        let stored = state
            .notify_for_download("model-a")
            .await
            .expect("download exists");
        assert!(Arc::ptr_eq(&notify, &stored));

        let removed = state.remove_download("model-a").await;
        assert!(removed.is_some());
        assert!(!state.has_active_downloads().await);
    }
}
