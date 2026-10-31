use serde::{Deserialize, Serialize};
pub use sona_core::config::{MediaServerProfile, MediaServerType};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MediaServerLoginPayload {
    pub server_url: String,
    pub server_type: MediaServerType,
    pub username: String,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub allow_invalid_certs: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MediaServerLoginResult {
    pub access_token: String,
    pub user_id: String,
    pub user_name: String,
    pub is_admin: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MediaServerInfo {
    pub id: String,
    pub server_name: String,
    pub version: String,
    pub has_subtitle_manager_plugin: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admin_user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    pub server_type: MediaServerType,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MediaServerLibrary {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collection_type: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MediaServerStreamInfo {
    #[serde(alias = "Index")]
    pub index: i32,
    #[serde(alias = "Type")]
    pub stream_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "Language")]
    pub language: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "DisplayTitle"
    )]
    pub display_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "Codec")]
    pub codec: Option<String>,
    #[serde(default, alias = "IsExternal")]
    pub is_external: bool,
    #[serde(default, alias = "IsForced")]
    pub is_forced: bool,
    #[serde(default, alias = "IsDefault")]
    pub is_default: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MediaServerMediaItem {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub season_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub season_index: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub episode_index: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_ticks: Option<i64>,
    #[serde(default)]
    pub media_streams: Vec<MediaServerStreamInfo>,
    #[serde(default)]
    pub has_chinese_subtitles: bool,
    #[serde(default)]
    pub has_embedded_chinese: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_source_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MediaServerItemListResult {
    pub items: Vec<MediaServerMediaItem>,
    pub total_record_count: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MediaServerSubtitleUploadPayload {
    pub item_id: String,
    pub language: String,
    pub format: String,
    #[serde(default)]
    pub is_forced: bool,
    pub content: String,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum MediaServerError {
    #[error("Network error: {0}")]
    Network(String),
    #[error("Unauthorized: Invalid API key or insufficient permissions")]
    Unauthorized,
    #[error("Emby.SubtitleManager plugin is not installed on the server")]
    PluginMissing,
    #[error("Duplicate subtitle already exists")]
    DuplicateSubtitle,
    #[error("Media server error: {0}")]
    ServerError(String),
    #[error("IO error: {0}")]
    Io(String),
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("Media server profile '{0}' not found")]
    ServerNotFound(String),
}

impl From<std::io::Error> for MediaServerError {
    fn from(err: std::io::Error) -> Self {
        MediaServerError::Io(err.to_string())
    }
}

impl From<reqwest::Error> for MediaServerError {
    fn from(err: reqwest::Error) -> Self {
        MediaServerError::Network(err.to_string())
    }
}

impl From<serde_json::Error> for MediaServerError {
    fn from(err: serde_json::Error) -> Self {
        MediaServerError::ServerError(err.to_string())
    }
}
