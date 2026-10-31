use super::types::{
    MediaServerError, MediaServerInfo, MediaServerItemListResult, MediaServerLibrary,
    MediaServerProfile, MediaServerStreamInfo, MediaServerSubtitleUploadPayload,
};
use std::path::Path;

#[async_trait::async_trait]
pub trait MediaServerClient: Send + Sync + std::fmt::Debug {
    fn profile(&self) -> &MediaServerProfile;
    async fn test_connection(&self) -> Result<MediaServerInfo, MediaServerError>;
    async fn get_libraries(&self) -> Result<Vec<MediaServerLibrary>, MediaServerError>;
    async fn get_items(
        &self,
        parent_id: Option<&str>,
        search_term: Option<&str>,
        limit: u32,
        start_index: u32,
    ) -> Result<MediaServerItemListResult, MediaServerError>;
    async fn get_item_streams(
        &self,
        item_id: &str,
    ) -> Result<Vec<MediaServerStreamInfo>, MediaServerError>;
    async fn download_audio_stream(
        &self,
        item_id: &str,
        stream_index: Option<i32>,
        destination_path: &Path,
    ) -> Result<u64, MediaServerError>;
    async fn upload_subtitle(
        &self,
        payload: &MediaServerSubtitleUploadPayload,
    ) -> Result<(), MediaServerError>;
    async fn delete_subtitle(
        &self,
        item_id: &str,
        subtitle_index: i32,
    ) -> Result<(), MediaServerError>;
    async fn refresh_item_metadata(&self, item_id: &str) -> Result<(), MediaServerError>;
}
