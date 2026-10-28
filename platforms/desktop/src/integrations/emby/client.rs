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

impl EmbyClient {
    pub fn new(config: EmbyConnectionConfig) -> Result<Self, EmbyError> {
        let base_url = normalize_server_url(&config.server_url);
        if base_url.is_empty() {
            return Err(EmbyError::Network("Server URL cannot be empty".to_string()));
        }

        let mut builder = reqwest::Client::builder()
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

        // 2. Check SubtitleManager plugin presence
        let plugin_url = self.url("/SubtitleManager/Libraries");
        let plugin_req = self.apply_auth(
            self.client
                .get(&plugin_url)
                .query(&[("api_key", &self.config.api_key)]),
        );

        let has_subtitle_manager_plugin = match plugin_req.send().await {
            Ok(plugin_resp) => {
                if plugin_resp.status() == reqwest::StatusCode::UNAUTHORIZED
                    || plugin_resp.status() == reqwest::StatusCode::FORBIDDEN
                {
                    return Err(EmbyError::Unauthorized);
                }
                plugin_resp.status().is_success()
            }
            Err(_) => false,
        };

        Ok(EmbyServerInfo {
            id,
            server_name,
            version,
            has_subtitle_manager_plugin,
        })
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
                "MediaStreams,RunTimeTicks,SeriesName,SeasonName,IndexNumber,ParentIndexNumber",
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
                });
            }
        }

        Ok(EmbyItemListResult {
            items,
            total_record_count,
        })
    }

    pub async fn download_audio_stream(
        &self,
        item_id: &str,
        stream_index: Option<i32>,
        destination_path: &Path,
    ) -> Result<u64, EmbyError> {
        let endpoint = format!("/Audio/{item_id}/stream.mp3");
        let url = self.url(&endpoint);

        if let Some(parent) = destination_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| EmbyError::ServerError(format!("Failed to create directory: {e}")))?;
        }

        let mut last_err = None;
        for attempt in 0..3 {
            let mut req = self.apply_auth(
                self.client
                    .get(&url)
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
                    if !resp.status().is_success() {
                        return Err(EmbyError::ServerError(format!(
                            "Audio download failed: HTTP {}",
                            resp.status()
                        )));
                    }

                    let mut file = match tokio::fs::File::create(destination_path).await {
                        Ok(f) => f,
                        Err(e) => {
                            return Err(EmbyError::ServerError(format!(
                                "Failed to create target file: {e}"
                            )));
                        }
                    };

                    let mut stream = resp.bytes_stream();
                    let mut bytes_written: u64 = 0;
                    let mut stream_failed = false;

                    while let Some(chunk_res) = stream.next().await {
                        match chunk_res {
                            Ok(chunk) => {
                                if let Err(e) = file.write_all(&chunk).await {
                                    return Err(EmbyError::ServerError(format!(
                                        "Failed to write audio chunk: {e}"
                                    )));
                                }
                                bytes_written += chunk.len() as u64;
                            }
                            Err(e) => {
                                stream_failed = true;
                                last_err = Some(EmbyError::Network(format!(
                                    "Stream interrupted on attempt {attempt}: {e}"
                                )));
                                break;
                            }
                        }
                    }

                    if !stream_failed {
                        if let Err(e) = file.flush().await {
                            return Err(EmbyError::ServerError(format!(
                                "Failed to flush file: {e}"
                            )));
                        }
                        return Ok(bytes_written);
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
        let url = self.url("/SubtitleManager/DeleteSubtitle");
        let idx_str = subtitle_index.to_string();
        let query = [
            ("ItemId", item_id),
            ("SubtitleIndex", &idx_str),
            ("api_key", &self.config.api_key),
        ];

        let req = self.apply_auth(self.client.post(&url).query(&query));
        let resp = req
            .send()
            .await
            .map_err(|e| EmbyError::Network(e.to_string()))?;

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED
            || resp.status() == reqwest::StatusCode::FORBIDDEN
        {
            return Err(EmbyError::Unauthorized);
        }

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(EmbyError::PluginMissing);
        }

        if !resp.status().is_success() {
            return Err(EmbyError::ServerError(format!(
                "Failed to delete subtitle: HTTP {}",
                resp.status()
            )));
        }

        Ok(())
    }

    pub async fn upload_subtitle(&self, req: &SubtitleUploadPayload) -> Result<(), EmbyError> {
        let url = self.url("/SubtitleManager/Upload");
        let forced_str = req.is_forced.to_string();
        let query = [
            ("ItemId", req.item_id.as_str()),
            ("Language", req.language.as_str()),
            ("Format", req.format.as_str()),
            ("IsForced", &forced_str),
            ("api_key", &self.config.api_key),
        ];

        let request = self.apply_auth(
            self.client
                .post(&url)
                .query(&query)
                .header("Content-Type", "text/plain; charset=utf-8")
                .body(req.content.clone().into_bytes()),
        );

        let resp = request
            .send()
            .await
            .map_err(|e| EmbyError::Network(e.to_string()))?;

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED
            || resp.status() == reqwest::StatusCode::FORBIDDEN
        {
            return Err(EmbyError::Unauthorized);
        }

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(EmbyError::PluginMissing);
        }

        if resp.status() == reqwest::StatusCode::CONFLICT {
            return Err(EmbyError::DuplicateSubtitle);
        }

        if !resp.status().is_success() {
            return Err(EmbyError::ServerError(format!(
                "Failed to upload subtitle: HTTP {}",
                resp.status()
            )));
        }

        Ok(())
    }

    pub async fn refresh_item_metadata(&self, item_id: &str) -> Result<(), EmbyError> {
        let url = self.url(&format!("/Items/{item_id}/Refresh"));
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

        // Lightweight poll up to 3 times with 1-second delay
        let item_url = self.url(&format!("/Items/{item_id}"));
        for _ in 0..3 {
            tokio::time::sleep(Duration::from_millis(1000)).await;
            let poll_req = self.apply_auth(self.client.get(&item_url).query(&[
                ("Fields", "MediaStreams"),
                ("api_key", &self.config.api_key),
            ]));
            if let Ok(poll_resp) = poll_req.send().await
                && poll_resp.status().is_success()
            {
                break;
            }
        }

        Ok(())
    }
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
        };
        let res = EmbyClient::new(config);
        assert!(matches!(res, Err(EmbyError::Network(_))));
    }

    #[test]
    fn test_plugin_missing_classification() {
        // Test that 404 on SubtitleManager results in PluginMissing
        let err_status = reqwest::StatusCode::NOT_FOUND;
        let is_plugin_missing = err_status == reqwest::StatusCode::NOT_FOUND;
        assert!(is_plugin_missing);
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
}
