use futures_util::StreamExt;
use serde_json::Value;
use std::path::Path;
use std::time::Duration;
use tokio::io::AsyncWriteExt;

use super::types::{
    EmbyConnectionConfig, EmbyError, EmbyItemListResult, EmbyLibrary, EmbyMediaItem,
    EmbyServerInfo, EmbyStreamInfo, SubtitleUploadPayload, is_chinese_language,
    normalize_server_url,
};

#[derive(Clone, Debug)]
pub struct EmbyClient {
    client: reqwest::Client,
    config: EmbyConnectionConfig,
    base_url: String,
}

struct PartFileGuard<'a> {
    path: &'a Path,
    disarmed: bool,
}

impl<'a> PartFileGuard<'a> {
    fn new(path: &'a Path) -> Self {
        Self {
            path,
            disarmed: false,
        }
    }

    fn disarm(&mut self) {
        self.disarmed = true;
    }
}

impl<'a> Drop for PartFileGuard<'a> {
    fn drop(&mut self) {
        if !self.disarmed {
            let _ = std::fs::remove_file(self.path);
        }
    }
}

impl EmbyClient {
    pub fn new(mut config: EmbyConnectionConfig) -> Result<Self, EmbyError> {
        config.server_url = config.server_url.trim().to_string();
        config.api_key = config.api_key.trim().to_string();
        let base_url = normalize_server_url(&config.server_url);
        if base_url.is_empty() {
            return Err(EmbyError::Network("Server URL cannot be empty".to_string()));
        }

        let mut builder = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .user_agent("Sona-Desktop/1.0");
        if config.allow_invalid_certs {
            builder = builder.danger_accept_invalid_certs(true);
        }

        let client = builder
            .build()
            .map_err(|e| EmbyError::Network(e.to_string()))?;

        Ok(Self {
            client,
            config,
            base_url,
        })
    }

    pub fn config(&self) -> &EmbyConnectionConfig {
        &self.config
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    fn url(&self, endpoint: &str) -> String {
        let clean_endpoint = endpoint.trim_start_matches('/');
        format!("{}/{}", self.base_url, clean_endpoint)
    }

    fn validate_item_id(item_id: &str) -> Result<&str, EmbyError> {
        let trimmed = item_id.trim();
        if trimmed.is_empty()
            || !trimmed
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(EmbyError::InvalidConfig(format!(
                "Invalid item ID: '{item_id}'"
            )));
        }
        Ok(trimmed)
    }

    fn apply_auth(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        req.header("X-Emby-Token", &self.config.api_key)
    }

    pub async fn test_connection(&self) -> Result<EmbyServerInfo, EmbyError> {
        // 1. Check System Info
        let system_info_url = self.url("/System/Info");
        let req = self.apply_auth(
            self.client
                .get(&system_info_url)
                .query(&[("api_key", &self.config.api_key)]),
        );

        let resp = req
            .send()
            .await
            .map_err(|e| EmbyError::Network(e.to_string()))?;

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED
            || resp.status() == reqwest::StatusCode::FORBIDDEN
        {
            return Err(EmbyError::Unauthorized);
        }

        if !resp.status().is_success() {
            return Err(EmbyError::ServerError(format!(
                "Failed to query system info: HTTP {}",
                resp.status()
            )));
        }

        let json: Value = resp
            .json()
            .await
            .map_err(|e| EmbyError::ServerError(format!("Invalid system info JSON: {e}")))?;

        let id = json
            .get("Id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let server_name = json
            .get("ServerName")
            .and_then(|v| v.as_str())
            .unwrap_or("Emby Server")
            .to_string();
        let version = json
            .get("Version")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        // 2. Check SubtitleManager plugin presence via /Plugins
        let plugins_url = self.url("/Plugins");
        let plugins_req = self.apply_auth(
            self.client
                .get(&plugins_url)
                .query(&[("api_key", &self.config.api_key)]),
        );

        let has_subtitle_manager_plugin = match plugins_req.send().await {
            Ok(resp) if resp.status().is_success() => match resp.json::<Value>().await {
                Ok(json) => has_subtitle_manager_plugin_in_list(&json),
                Err(_) => false,
            },
            _ => false,
        };

        // 3. Discover administrator user ID
        let admin_user_id = self.discover_admin_user_id().await;

        Ok(EmbyServerInfo {
            id,
            server_name,
            version,
            has_subtitle_manager_plugin,
            admin_user_id,
        })
    }

    pub async fn discover_admin_user_id(&self) -> Option<String> {
        let users_url = self.url("/Users");
        let req = self.apply_auth(
            self.client
                .get(&users_url)
                .query(&[("api_key", &self.config.api_key)]),
        );

        let resp = req.send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }

        let json: Value = resp.json().await.ok()?;
        parse_admin_user_id(&json)
    }

    pub async fn resolve_admin_user_id(&self) -> Option<String> {
        if let Some(ref id) = self.config.admin_user_id {
            let trimmed = id.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        self.discover_admin_user_id().await
    }

    pub async fn get_libraries(&self) -> Result<Vec<EmbyLibrary>, EmbyError> {
        let url = self.url("/Library/MediaFolders");
        let req = self.apply_auth(
            self.client
                .get(&url)
                .query(&[("api_key", &self.config.api_key)]),
        );

        let resp = req
            .send()
            .await
            .map_err(|e| EmbyError::Network(e.to_string()))?;

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED
            || resp.status() == reqwest::StatusCode::FORBIDDEN
        {
            return Err(EmbyError::Unauthorized);
        }

        if !resp.status().is_success() {
            return Err(EmbyError::ServerError(format!(
                "Failed to fetch libraries: HTTP {}",
                resp.status()
            )));
        }

        let json: Value = resp
            .json()
            .await
            .map_err(|e| EmbyError::ServerError(format!("Invalid libraries response: {e}")))?;

        let mut libraries = Vec::new();
        if let Some(items) = json.get("Items").and_then(|v| v.as_array()) {
            for item in items {
                let id = item.get("Id").and_then(|v| v.as_str()).unwrap_or_default();
                let name = item
                    .get("Name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let collection_type = item
                    .get("CollectionType")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                if !id.is_empty() && !name.is_empty() {
                    libraries.push(EmbyLibrary {
                        id: id.to_string(),
                        name: name.to_string(),
                        collection_type,
                    });
                }
            }
        }

        Ok(libraries)
    }

    pub async fn get_items(
        &self,
        parent_id: Option<&str>,
        search_term: Option<&str>,
        start_index: u32,
        limit: u32,
    ) -> Result<EmbyItemListResult, EmbyError> {
        let url = self.url("/Items");
        let start_str = start_index.to_string();
        let limit_str = limit.to_string();

        let mut query: Vec<(&str, &str)> = vec![
            ("IncludeItemTypes", "Movie,Episode"),
            (
                "Fields",
                "MediaStreams,RunTimeTicks,SeriesName,SeasonName,IndexNumber,ParentIndexNumber,MediaSources,Container",
            ),
            ("StartIndex", &start_str),
            ("Limit", &limit_str),
            ("Recursive", "true"),
            ("api_key", &self.config.api_key),
        ];

        if let Some(pid) = parent_id.filter(|s| !s.is_empty()) {
            query.push(("ParentId", pid));
        }
        if let Some(st) = search_term.filter(|s| !s.is_empty()) {
            query.push(("SearchTerm", st));
        }

        let req = self.apply_auth(self.client.get(&url).query(&query));
        let resp = req
            .send()
            .await
            .map_err(|e| EmbyError::Network(e.to_string()))?;

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED
            || resp.status() == reqwest::StatusCode::FORBIDDEN
        {
            return Err(EmbyError::Unauthorized);
        }

        if !resp.status().is_success() {
            return Err(EmbyError::ServerError(format!(
                "Failed to fetch items: HTTP {}",
                resp.status()
            )));
        }

        let json: Value = resp
            .json()
            .await
            .map_err(|e| EmbyError::ServerError(format!("Invalid items response: {e}")))?;

        let total_record_count = json
            .get("TotalRecordCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        let mut items = Vec::new();
        if let Some(raw_items) = json.get("Items").and_then(|v| v.as_array()) {
            for raw in raw_items {
                let id = raw
                    .get("Id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let name = raw
                    .get("Name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let series_name = raw
                    .get("SeriesName")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let season_name = raw
                    .get("SeasonName")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let season_index = raw
                    .get("ParentIndexNumber")
                    .and_then(|v| v.as_i64())
                    .map(|i| i as i32);
                let episode_index = raw
                    .get("IndexNumber")
                    .and_then(|v| v.as_i64())
                    .map(|i| i as i32);
                let runtime_ticks = raw.get("RunTimeTicks").and_then(|v| v.as_i64());
                let item_type = raw
                    .get("Type")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let media_source_id = raw
                    .get("MediaSources")
                    .and_then(|v| v.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|s| s.get("Id").and_then(|v| v.as_str()))
                    .map(|s| s.to_string())
                    .or_else(|| {
                        if !id.is_empty() {
                            Some(format!("mediasource_{id}"))
                        } else {
                            None
                        }
                    });
                let container = raw
                    .get("MediaSources")
                    .and_then(|v| v.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|s| s.get("Container").and_then(|v| v.as_str()))
                    .or_else(|| raw.get("Container").and_then(|v| v.as_str()))
                    .map(|s| s.to_string());
                let mut media_streams = Vec::new();
                let mut has_chinese_subtitles = false;
                let mut has_embedded_chinese = false;

                if let Some(streams) = raw.get("MediaStreams").and_then(|v| v.as_array()) {
                    for s in streams {
                        let stream_type = s
                            .get("Type")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string();
                        let index = s.get("Index").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                        let language = s
                            .get("Language")
                            .and_then(|v| v.as_str())
                            .map(|v| v.to_string());
                        let display_title = s
                            .get("DisplayTitle")
                            .and_then(|v| v.as_str())
                            .map(|v| v.to_string());
                        let codec = s
                            .get("Codec")
                            .and_then(|v| v.as_str())
                            .map(|v| v.to_string());
                        let is_external = s
                            .get("IsExternal")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let is_forced =
                            s.get("IsForced").and_then(|v| v.as_bool()).unwrap_or(false);
                        let is_default = s
                            .get("IsDefault")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);

                        if stream_type.eq_ignore_ascii_case("Subtitle") {
                            let is_zh =
                                is_chinese_language(language.as_deref(), display_title.as_deref());
                            if is_zh {
                                has_chinese_subtitles = true;
                                if !is_external {
                                    has_embedded_chinese = true;
                                }
                            }
                        }

                        media_streams.push(EmbyStreamInfo {
                            index,
                            stream_type,
                            language,
                            display_title,
                            codec,
                            is_external,
                            is_forced,
                            is_default,
                        });
                    }
                }

                items.push(EmbyMediaItem {
                    id,
                    name,
                    series_name,
                    season_name,
                    season_index,
                    episode_index,
                    runtime_ticks,
                    media_streams,
                    has_chinese_subtitles,
                    has_embedded_chinese,
                    item_type,
                    media_source_id,
                    container,
                });
            }
        }

        Ok(EmbyItemListResult {
            items,
            total_record_count,
        })
    }
    pub async fn get_item_streams(&self, item_id: &str) -> Result<Vec<EmbyStreamInfo>, EmbyError> {
        let clean_id = Self::validate_item_id(item_id)?;
        let url = self.url(&format!("/Items/{clean_id}"));
        let req = self.apply_auth(self.client.get(&url).query(&[
            ("Fields", "MediaStreams"),
            ("api_key", &self.config.api_key),
        ]));
        let resp = req
            .send()
            .await
            .map_err(|e| EmbyError::Network(e.to_string()))?;

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED
            || resp.status() == reqwest::StatusCode::FORBIDDEN
        {
            return Err(EmbyError::Unauthorized);
        }

        if !resp.status().is_success() {
            return Err(EmbyError::ServerError(format!(
                "Failed to get item details: HTTP {}",
                resp.status()
            )));
        }

        let raw: Value = resp
            .json()
            .await
            .map_err(|e| EmbyError::ServerError(format!("Invalid item JSON: {e}")))?;

        let mut media_streams = Vec::new();
        if let Some(streams) = raw.get("MediaStreams").and_then(|v| v.as_array()) {
            for s in streams {
                let stream_type = s
                    .get("Type")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let index = s.get("Index").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let language = s
                    .get("Language")
                    .and_then(|v| v.as_str())
                    .map(|v| v.to_string());
                let display_title = s
                    .get("DisplayTitle")
                    .and_then(|v| v.as_str())
                    .map(|v| v.to_string());
                let codec = s
                    .get("Codec")
                    .and_then(|v| v.as_str())
                    .map(|v| v.to_string());
                let is_external = s
                    .get("IsExternal")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let is_forced = s.get("IsForced").and_then(|v| v.as_bool()).unwrap_or(false);
                let is_default = s
                    .get("IsDefault")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                media_streams.push(EmbyStreamInfo {
                    index,
                    stream_type,
                    language,
                    display_title,
                    codec,
                    is_external,
                    is_forced,
                    is_default,
                });
            }
        }
        Ok(media_streams)
    }

    pub async fn download_audio_stream(
        &self,
        item_id: &str,
        stream_index: Option<i32>,
        destination_path: &Path,
    ) -> Result<u64, EmbyError> {
        let clean_id = Self::validate_item_id(item_id)?;
        let endpoint = format!("/Audio/{clean_id}/stream.mp3");
        let url = self.url(&endpoint);

        if let Some(parent) = destination_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| EmbyError::Io(e.to_string()))?;
        }

        let mut last_err = None;
        for attempt in 0..3 {
            let part_path =
                destination_path.with_extension(format!("part_{}", uuid::Uuid::new_v4()));
            let mut part_guard = PartFileGuard::new(&part_path);
            let mut req = self.apply_auth(
                self.client
                    .get(&url)
                    .timeout(Duration::from_secs(600))
                    .query(&[("api_key", &self.config.api_key)]),
            );

            let idx_str;
            if let Some(idx) = stream_index {
                idx_str = idx.to_string();
                req = req.query(&[("AudioStreamIndex", &idx_str)]);
            }

            match req.send().await {
                Ok(resp) => {
                    if resp.status() == reqwest::StatusCode::UNAUTHORIZED
                        || resp.status() == reqwest::StatusCode::FORBIDDEN
                    {
                        return Err(EmbyError::Unauthorized);
                    }
                    if resp.status().is_server_error() {
                        last_err = Some(EmbyError::ServerError(format!(
                            "Audio download failed with server error: HTTP {}",
                            resp.status()
                        )));
                        tokio::time::sleep(Duration::from_millis(500 * (attempt + 1) as u64)).await;
                        continue;
                    }
                    if !resp.status().is_success() {
                        return Err(EmbyError::ServerError(format!(
                            "Audio download failed: HTTP {}",
                            resp.status()
                        )));
                    }

                    let file = match tokio::fs::File::create(&part_path).await {
                        Ok(f) => f,
                        Err(e) => {
                            return Err(EmbyError::Io(e.to_string()));
                        }
                    };
                    let mut file_opt = Some(file);

                    let mut stream = resp.bytes_stream();
                    let mut bytes_written: u64 = 0;

                    while let Some(chunk_res) = stream.next().await {
                        match chunk_res {
                            Ok(chunk) => {
                                let write_res = if let Some(f) = file_opt.as_mut() {
                                    f.write_all(&chunk).await
                                } else {
                                    Ok(())
                                };
                                if let Err(e) = write_res {
                                    drop(file_opt.take());
                                    return Err(EmbyError::Io(e.to_string()));
                                }
                                bytes_written += chunk.len() as u64;
                            }
                            Err(e) => {
                                drop(file_opt.take());
                                last_err = Some(EmbyError::Network(format!(
                                    "Stream interrupted on attempt {attempt}: {e}"
                                )));
                                break;
                            }
                        }
                    }

                    if let Some(mut file) = file_opt {
                        if let Err(e) = file.flush().await {
                            drop(file);
                            return Err(EmbyError::Io(e.to_string()));
                        }
                        drop(file);

                        if bytes_written == 0 {
                            last_err = Some(EmbyError::ServerError(
                                "Downloaded audio stream is empty (0 bytes)".into(),
                            ));
                        } else if let Err(e) = tokio::fs::rename(&part_path, destination_path).await
                        {
                            return Err(EmbyError::Io(e.to_string()));
                        } else {
                            part_guard.disarm();
                            return Ok(bytes_written);
                        }
                    }
                }
                Err(e) => {
                    last_err = Some(EmbyError::Network(format!(
                        "Download request failed on attempt {attempt}: {e}"
                    )));
                }
            }

            tokio::time::sleep(Duration::from_millis(500 * (attempt + 1) as u64)).await;
        }

        Err(last_err.unwrap_or_else(|| EmbyError::Network("Download audio stream failed".into())))
    }

    pub async fn delete_subtitle(
        &self,
        item_id: &str,
        subtitle_index: i32,
    ) -> Result<(), EmbyError> {
        if subtitle_index < 0 {
            return Err(EmbyError::InvalidConfig(
                "Subtitle index cannot be negative".into(),
            ));
        }
        let clean_id = Self::validate_item_id(item_id)?;
        let url = self.url("/SubtitleManager/DeleteSubtitle");
        let idx_str = subtitle_index.to_string();
        let admin_id_opt = self.resolve_admin_user_id().await;

        let mut query = vec![
            ("ItemId", clean_id),
            ("SubtitleIndex", idx_str.as_str()),
            ("api_key", self.config.api_key.as_str()),
        ];
        if let Some(ref admin_id) = admin_id_opt {
            query.push(("UserId", admin_id.as_str()));
        }

        let mut req = self.apply_auth(self.client.post(&url).query(&query));
        if let Some(ref admin_id) = admin_id_opt {
            req = req.header(
                "X-Emby-Authorization",
                format!(
                    "MediaBrowser Client=\"Sona\", Device=\"Desktop\", DeviceId=\"sona-desktop\", Version=\"1.0\", UserId=\"{admin_id}\""
                ),
            );
        }

        let resp = req
            .send()
            .await
            .map_err(|e| EmbyError::Network(e.to_string()))?;

        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(EmbyError::Unauthorized);
        }

        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(EmbyError::PluginMissing);
        }

        let body_text = resp
            .text()
            .await
            .map_err(|e| EmbyError::Network(e.to_string()))?;

        if !status.is_success() {
            check_plugin_response_body(&body_text)?;
            let msg = if body_text.trim().is_empty() {
                format!("Failed to delete subtitle: HTTP {}", status)
            } else {
                format!(
                    "Failed to delete subtitle: HTTP {} - {}",
                    status,
                    body_text.trim()
                )
            };
            return Err(EmbyError::ServerError(msg));
        }

        check_plugin_response_body(&body_text)?;

        Ok(())
    }

    pub async fn upload_subtitle(&self, req: &SubtitleUploadPayload) -> Result<(), EmbyError> {
        let clean_id = Self::validate_item_id(&req.item_id)?;
        let url = self.url("/SubtitleManager/Upload");
        let forced_str = req.is_forced.to_string();
        let admin_id_opt = self.resolve_admin_user_id().await;

        let mut query = vec![
            ("ItemId", clean_id),
            ("Language", req.language.as_str()),
            ("Format", req.format.as_str()),
            ("IsForced", forced_str.as_str()),
            ("api_key", self.config.api_key.as_str()),
        ];
        if let Some(ref admin_id) = admin_id_opt {
            query.push(("UserId", admin_id.as_str()));
        }

        let mut request = self.apply_auth(
            self.client
                .post(&url)
                .query(&query)
                .header("Content-Type", "text/plain; charset=utf-8")
                .body(req.content.clone().into_bytes()),
        );
        if let Some(ref admin_id) = admin_id_opt {
            request = request.header(
                "X-Emby-Authorization",
                format!(
                    "MediaBrowser Client=\"Sona\", Device=\"Desktop\", DeviceId=\"sona-desktop\", Version=\"1.0\", UserId=\"{admin_id}\""
                ),
            );
        }

        let resp = request
            .send()
            .await
            .map_err(|e| EmbyError::Network(e.to_string()))?;

        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(EmbyError::Unauthorized);
        }

        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(EmbyError::PluginMissing);
        }

        if status == reqwest::StatusCode::CONFLICT {
            return Err(EmbyError::DuplicateSubtitle);
        }

        let body_text = resp
            .text()
            .await
            .map_err(|e| EmbyError::Network(e.to_string()))?;

        if !status.is_success() {
            check_plugin_response_body(&body_text)?;
            let msg = if body_text.trim().is_empty() {
                format!("Failed to upload subtitle: HTTP {}", status)
            } else {
                format!(
                    "Failed to upload subtitle: HTTP {} - {}",
                    status,
                    body_text.trim()
                )
            };
            return Err(EmbyError::ServerError(msg));
        }

        check_plugin_response_body(&body_text)?;

        Ok(())
    }

    pub async fn refresh_item_metadata(&self, item_id: &str) -> Result<(), EmbyError> {
        let clean_id = Self::validate_item_id(item_id)?;
        let url = self.url(&format!("/Items/{clean_id}/Refresh"));
        let req = self.apply_auth(
            self.client
                .post(&url)
                .query(&[("api_key", &self.config.api_key)]),
        );

        let resp = req
            .send()
            .await
            .map_err(|e| EmbyError::Network(e.to_string()))?;

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED
            || resp.status() == reqwest::StatusCode::FORBIDDEN
        {
            return Err(EmbyError::Unauthorized);
        }

        if !resp.status().is_success() {
            return Err(EmbyError::ServerError(format!(
                "Failed to refresh item metadata: HTTP {}",
                resp.status()
            )));
        }
        tokio::time::sleep(Duration::from_millis(1500)).await;
        let item_url = self.url(&format!("/Items/{clean_id}"));
        let poll_req = self.apply_auth(self.client.get(&item_url).query(&[
            ("Fields", "MediaStreams"),
            ("api_key", &self.config.api_key),
        ]));
        let _ = poll_req.send().await;
        Ok(())
    }
}
pub(crate) fn check_plugin_response_body(body_text: &str) -> Result<(), EmbyError> {
    if let Ok(json) = serde_json::from_str::<Value>(body_text) {
        let is_success = json
            .get("Success")
            .or_else(|| json.get("success"))
            .and_then(|v| v.as_bool());
        if is_success == Some(false) {
            let msg = json
                .get("Message")
                .or_else(|| json.get("message"))
                .and_then(|v| v.as_str())
                .unwrap_or("Operation failed");
            let lower = msg.to_ascii_lowercase();
            if lower.contains("duplicate")
                || lower.contains("already exist")
                || msg.contains("已存在")
                || msg.contains("重复")
                || msg.contains("既に存在")
                || msg.contains("重複")
                || msg.contains("이미 있습니다")
                || msg.contains("중복")
            {
                return Err(EmbyError::DuplicateSubtitle);
            } else {
                return Err(EmbyError::ServerError(msg.to_string()));
            }
        }
    }
    Ok(())
}

pub(crate) fn is_subtitle_manager_plugin(plugin: &Value) -> bool {
    const TARGET_PLUGIN_ID: &str = "944151cb-7308-487b-8023-3e5c352bc545";
    const TARGET_CONFIG_FILE: &str = "Emby.SubtitleManager.xml";

    if let Some(id) = plugin.get("Id").and_then(|v| v.as_str())
        && id.eq_ignore_ascii_case(TARGET_PLUGIN_ID)
    {
        return true;
    }

    if let Some(name) = plugin
        .get("Name")
        .or_else(|| plugin.get("name"))
        .and_then(|v| v.as_str())
    {
        let name_lower = name.to_ascii_lowercase();
        if name_lower.contains("subtitlemanager")
            || name_lower.contains("subtitle manager")
            || name.contains("字幕管理")
        {
            return true;
        }
    }

    if let Some(config_file) = plugin.get("ConfigurationFileName").and_then(|v| v.as_str())
        && config_file.eq_ignore_ascii_case(TARGET_CONFIG_FILE)
    {
        return true;
    }

    false
}

pub(crate) fn has_subtitle_manager_plugin_in_list(plugins_json: &Value) -> bool {
    if let Some(array) = plugins_json.as_array() {
        array.iter().any(is_subtitle_manager_plugin)
    } else {
        false
    }
}

pub(crate) fn parse_admin_user_id(users_json: &Value) -> Option<String> {
    let users = users_json.as_array()?;
    for user in users {
        let policy = user.get("Policy").or_else(|| user.get("policy"));
        let is_admin = policy
            .and_then(|p| {
                p.get("IsAdministrator")
                    .or_else(|| p.get("isAdministrator"))
            })
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let is_disabled = policy
            .and_then(|p| p.get("IsDisabled").or_else(|| p.get("isDisabled")))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if is_admin && !is_disabled {
            let id_val = user.get("Id").or_else(|| user.get("id"));
            if let Some(id) = id_val.and_then(|v| v.as_str()) {
                let trimmed = id.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    None
}

#[cfg(test)]
pub(crate) fn map_plugin_response_status(
    status: reqwest::StatusCode,
    body_text: Option<&str>,
) -> Result<(), EmbyError> {
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(EmbyError::Unauthorized);
    }
    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(EmbyError::PluginMissing);
    }
    if status == reqwest::StatusCode::CONFLICT {
        return Err(EmbyError::DuplicateSubtitle);
    }
    if !status.is_success() {
        return Err(EmbyError::ServerError(format!("HTTP {status}")));
    }
    if let Some(body) = body_text {
        check_plugin_response_body(body)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emby_client_new() {
        let config = EmbyConnectionConfig {
            server_url: "http://192.168.1.100:8096/emby/".into(),
            api_key: "test_key".into(),
            allow_invalid_certs: true,
            admin_user_id: None,
        };
        let client = EmbyClient::new(config).unwrap();
        assert_eq!(client.base_url(), "http://192.168.1.100:8096/emby");
        assert_eq!(
            client.url("/System/Info"),
            "http://192.168.1.100:8096/emby/System/Info"
        );
    }

    #[test]
    fn test_emby_client_empty_url() {
        let config = EmbyConnectionConfig {
            server_url: "   ".into(),
            api_key: "test_key".into(),
            allow_invalid_certs: false,
            admin_user_id: None,
        };
        let res = EmbyClient::new(config);
        assert!(matches!(res, Err(EmbyError::Network(_))));
    }

    #[test]
    fn test_plugin_missing_classification() {
        assert_eq!(
            map_plugin_response_status(reqwest::StatusCode::NOT_FOUND, None),
            Err(EmbyError::PluginMissing)
        );
        assert_eq!(
            map_plugin_response_status(reqwest::StatusCode::UNAUTHORIZED, None),
            Err(EmbyError::Unauthorized)
        );
        assert_eq!(
            map_plugin_response_status(reqwest::StatusCode::CONFLICT, None),
            Err(EmbyError::DuplicateSubtitle)
        );
        assert_eq!(
            map_plugin_response_status(
                reqwest::StatusCode::OK,
                Some(r#"{"Success":false,"Message":"Duplicate subtitle already exists"}"#)
            ),
            Err(EmbyError::DuplicateSubtitle)
        );
        assert_eq!(
            map_plugin_response_status(
                reqwest::StatusCode::OK,
                Some(r#"{"Success":false,"Message":"该字幕已存在"}"#)
            ),
            Err(EmbyError::DuplicateSubtitle)
        );
        assert_eq!(
            map_plugin_response_status(
                reqwest::StatusCode::OK,
                Some(r#"{"Success":false,"Message":"Subtitle already exists for item"}"#)
            ),
            Err(EmbyError::DuplicateSubtitle)
        );
        assert_eq!(
            map_plugin_response_status(
                reqwest::StatusCode::OK,
                Some(r#"{"Success":false,"Message":"字幕文件重复"}"#)
            ),
            Err(EmbyError::DuplicateSubtitle)
        );
        assert_eq!(
            map_plugin_response_status(
                reqwest::StatusCode::OK,
                Some(r#"{"Success":false,"Message":"字幕は既に存在します"}"#)
            ),
            Err(EmbyError::DuplicateSubtitle)
        );
        assert_eq!(
            map_plugin_response_status(
                reqwest::StatusCode::OK,
                Some(r#"{"Success":false,"Message":"重複する字幕です"}"#)
            ),
            Err(EmbyError::DuplicateSubtitle)
        );
        assert_eq!(
            map_plugin_response_status(
                reqwest::StatusCode::OK,
                Some(r#"{"Success":false,"Message":"자막이 이미 있습니다"}"#)
            ),
            Err(EmbyError::DuplicateSubtitle)
        );
        assert_eq!(
            map_plugin_response_status(
                reqwest::StatusCode::OK,
                Some(r#"{"Success":false,"Message":"중복된 자막 파일"}"#)
            ),
            Err(EmbyError::DuplicateSubtitle)
        );
        assert_eq!(
            map_plugin_response_status(
                reqwest::StatusCode::OK,
                Some(r#"{"Success":false,"Message":"Internal error occurred"}"#)
            ),
            Err(EmbyError::ServerError("Internal error occurred".into()))
        );
        assert_eq!(
            map_plugin_response_status(reqwest::StatusCode::OK, Some(r#"{"Success":true}"#)),
            Ok(())
        );
    }
    #[test]
    fn test_emby_error_display() {
        assert_eq!(
            EmbyError::PluginMissing.to_string(),
            "Emby.SubtitleManager plugin is not installed on the server"
        );
        assert_eq!(
            EmbyError::Unauthorized.to_string(),
            "Unauthorized: Invalid API key or insufficient permissions"
        );
        assert_eq!(
            EmbyError::DuplicateSubtitle.to_string(),
            "Duplicate subtitle already exists"
        );
    }

    #[test]
    fn test_validate_item_id() {
        assert_eq!(EmbyClient::validate_item_id("12345").unwrap(), "12345");
        assert_eq!(
            EmbyClient::validate_item_id("item-abc_123").unwrap(),
            "item-abc_123"
        );
        assert!(EmbyClient::validate_item_id("").is_err());
        assert!(EmbyClient::validate_item_id("  ").is_err());
        assert!(EmbyClient::validate_item_id("../etc/passwd").is_err());
        assert!(EmbyClient::validate_item_id("item/123").is_err());
        assert!(EmbyClient::validate_item_id("item?query=1").is_err());
    }

    #[test]
    fn test_part_file_guard() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.part");

        // Case 1: dropped without disarm -> deletes file
        std::fs::write(&file_path, b"data").unwrap();
        assert!(file_path.exists());
        {
            let _guard = PartFileGuard::new(&file_path);
        }
        assert!(!file_path.exists());

        // Case 2: disarmed before drop -> retains file
        std::fs::write(&file_path, b"data").unwrap();
        assert!(file_path.exists());
        {
            let mut guard = PartFileGuard::new(&file_path);
            guard.disarm();
        }
        assert!(file_path.exists());
    }

    #[test]
    fn test_is_subtitle_manager_plugin_by_guid() {
        let json = serde_json::json!({
            "Id": "944151CB-7308-487B-8023-3E5C352BC545",
            "Name": "Custom Subtitle Extension",
            "ConfigurationFileName": "other.xml"
        });
        assert!(is_subtitle_manager_plugin(&json));
    }

    #[test]
    fn test_is_subtitle_manager_plugin_by_name() {
        let json1 = serde_json::json!({
            "Id": "some-other-guid",
            "Name": "SubtitleManager Plugin",
            "ConfigurationFileName": "plugin.xml"
        });
        assert!(is_subtitle_manager_plugin(&json1));

        let json2 = serde_json::json!({
            "Id": "some-other-guid",
            "Name": "Emby Subtitle Manager",
            "ConfigurationFileName": "plugin.xml"
        });
        assert!(is_subtitle_manager_plugin(&json2));

        let json3 = serde_json::json!({
            "Id": "some-other-guid",
            "Name": "通用字幕管理插件",
            "ConfigurationFileName": "plugin.xml"
        });
        assert!(is_subtitle_manager_plugin(&json3));
    }

    #[test]
    fn test_is_subtitle_manager_plugin_by_config_file() {
        let json = serde_json::json!({
            "Id": "some-other-guid",
            "Name": "Custom Plugin",
            "ConfigurationFileName": "emby.subtitlemanager.xml"
        });
        assert!(is_subtitle_manager_plugin(&json));
    }

    #[test]
    fn test_is_subtitle_manager_plugin_negative() {
        let json = serde_json::json!({
            "Id": "12345678-1234-1234-1234-123456789abc",
            "Name": "Trakt",
            "ConfigurationFileName": "Trakt.xml"
        });
        assert!(!is_subtitle_manager_plugin(&json));
    }

    #[test]
    fn test_has_subtitle_manager_plugin_in_list() {
        let list = serde_json::json!([
            {
                "Id": "11111111-1111-1111-1111-111111111111",
                "Name": "Open Subtitles",
                "ConfigurationFileName": "OpenSubtitles.xml"
            },
            {
                "Id": "944151cb-7308-487b-8023-3e5c352bc545",
                "Name": "SubtitleManager",
                "ConfigurationFileName": "Emby.SubtitleManager.xml"
            }
        ]);
        assert!(has_subtitle_manager_plugin_in_list(&list));

        let list_without = serde_json::json!([
            {
                "Id": "11111111-1111-1111-1111-111111111111",
                "Name": "Open Subtitles",
                "ConfigurationFileName": "OpenSubtitles.xml"
            }
        ]);
        assert!(!has_subtitle_manager_plugin_in_list(&list_without));
    }

    #[test]
    fn test_parse_admin_user_id() {
        let users = serde_json::json!([
            {
                "Id": "user_normal",
                "Name": "Guest",
                "Policy": {
                    "IsAdministrator": false
                }
            },
            {
                "Id": "admin_uuid_123",
                "Name": "Admin",
                "Policy": {
                    "IsAdministrator": true
                }
            },
            {
                "Id": "second_admin",
                "Name": "Admin2",
                "Policy": {
                    "IsAdministrator": true
                }
            }
        ]);

        assert_eq!(
            parse_admin_user_id(&users),
            Some("admin_uuid_123".to_string())
        );

        let no_admins = serde_json::json!([
            {
                "Id": "user1",
                "Name": "Normal",
                "Policy": {
                    "IsAdministrator": false
                }
            }
        ]);
        assert_eq!(parse_admin_user_id(&no_admins), None);

        let empty_list = serde_json::json!([]);
        assert_eq!(parse_admin_user_id(&empty_list), None);
    }

    #[test]
    fn test_emby_media_item_media_source_fields() {
        let json = serde_json::json!({
            "id": "125717",
            "name": "Test Movie",
            "mediaSourceId": "mediasource_125717",
            "container": "mkv",
            "hasChineseSubtitles": false,
            "hasEmbeddedChinese": false,
            "mediaStreams": []
        });
        let item: EmbyMediaItem = serde_json::from_value(json).unwrap();
        assert_eq!(item.id, "125717");
        assert_eq!(item.media_source_id, Some("mediasource_125717".to_string()));
        assert_eq!(item.container, Some("mkv".to_string()));
    }
}
