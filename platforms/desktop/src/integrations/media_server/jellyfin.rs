use base64::prelude::*;
use serde_json::Value;
use std::path::Path;
use std::time::Duration;

use super::common::{
    download_audio_stream_common, normalize_server_url, parse_admin_user_id, parse_login_response,
    parse_media_items, parse_media_streams, validate_item_id,
};
use super::traits::MediaServerClient;
use super::types::{
    MediaServerError, MediaServerInfo, MediaServerItemListResult, MediaServerLibrary,
    MediaServerLoginResult, MediaServerProfile, MediaServerStreamInfo,
    MediaServerSubtitleUploadPayload, MediaServerType,
};

#[derive(Clone, Debug)]
pub struct JellyfinClient {
    client: reqwest::Client,
    profile: MediaServerProfile,
    base_url: String,
}

impl JellyfinClient {
    pub fn new(mut profile: MediaServerProfile) -> Result<Self, MediaServerError> {
        profile.server_url = profile.server_url.trim().to_string();
        profile.api_key = profile.api_key.trim().to_string();
        let base_url = normalize_server_url(&profile.server_url);
        if base_url.is_empty() {
            return Err(MediaServerError::Network(
                "Server URL cannot be empty".to_string(),
            ));
        }

        let mut builder = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .user_agent("Sona-Desktop/1.0");
        if profile.allow_invalid_certs {
            builder = builder.danger_accept_invalid_certs(true);
        }

        let client = builder
            .build()
            .map_err(|e| MediaServerError::Network(e.to_string()))?;

        Ok(Self {
            client,
            profile,
            base_url,
        })
    }

    pub async fn authenticate_by_name(
        server_url: &str,
        username: &str,
        password: &str,
        allow_invalid_certs: bool,
    ) -> Result<MediaServerLoginResult, MediaServerError> {
        let clean_url = normalize_server_url(server_url);
        if clean_url.is_empty() {
            return Err(MediaServerError::Network(
                "Server URL cannot be empty".to_string(),
            ));
        }
        let trimmed_user = username.trim();
        if trimmed_user.is_empty() {
            return Err(MediaServerError::InvalidConfig(
                "Username cannot be empty".into(),
            ));
        }

        let mut builder = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .user_agent("Sona-Desktop/1.0");
        if allow_invalid_certs {
            builder = builder.danger_accept_invalid_certs(true);
        }
        let client = builder
            .build()
            .map_err(|e| MediaServerError::Network(e.to_string()))?;

        let auth_url = format!("{}/Users/AuthenticateByName", clean_url);
        let auth_header = "MediaBrowser Client=\"Sona\", Device=\"Desktop\", DeviceId=\"sona-desktop\", Version=\"1.0\"";

        let body = serde_json::json!({
            "Username": trimmed_user,
            "Pw": password,
        });

        let resp = client
            .post(&auth_url)
            .header("X-Emby-Authorization", auth_header)
            .header("Authorization", auth_header)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| MediaServerError::Network(e.to_string()))?;

        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(MediaServerError::Unauthorized);
        }

        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            let msg = if body_text.trim().is_empty() {
                format!("Login failed: HTTP {}", status)
            } else {
                format!("Login failed: HTTP {} - {}", status, body_text.trim())
            };
            return Err(MediaServerError::ServerError(msg));
        }

        let json: Value = resp
            .json()
            .await
            .map_err(|e| MediaServerError::ServerError(format!("Invalid login response: {e}")))?;

        parse_login_response(&json, trimmed_user)
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn url(&self, endpoint: &str) -> String {
        let clean_endpoint = endpoint.trim_start_matches('/');
        format!("{}/{}", self.base_url, clean_endpoint)
    }

    fn apply_auth(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        let auth_header = format!(
            "MediaBrowser Client=\"Sona\", Device=\"Desktop\", DeviceId=\"sona-desktop\", Version=\"1.0\", Token=\"{}\"",
            self.profile.api_key
        );
        req.header("X-Emby-Token", &self.profile.api_key)
            .header("X-MediaBrowser-Token", &self.profile.api_key)
            .header("Authorization", auth_header)
    }

    pub async fn discover_admin_user_id(&self) -> Option<String> {
        let users_url = self.url("/Users");
        let req = self.apply_auth(
            self.client
                .get(&users_url)
                .query(&[("api_key", &self.profile.api_key)]),
        );

        let resp = req.send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }

        let json: Value = resp.json().await.ok()?;
        parse_admin_user_id(&json)
    }

    pub async fn resolve_admin_user_id(&self) -> Option<String> {
        if let Some(id) = &self.profile.admin_user_id {
            let trimmed = id.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        self.discover_admin_user_id().await
    }
}

#[async_trait::async_trait]
impl MediaServerClient for JellyfinClient {
    fn profile(&self) -> &MediaServerProfile {
        &self.profile
    }

    async fn test_connection(&self) -> Result<MediaServerInfo, MediaServerError> {
        let system_info_url = self.url("/System/Info");
        let req = self.apply_auth(
            self.client
                .get(&system_info_url)
                .query(&[("api_key", &self.profile.api_key)]),
        );

        let resp = req
            .send()
            .await
            .map_err(|e| MediaServerError::Network(e.to_string()))?;

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED
            || resp.status() == reqwest::StatusCode::FORBIDDEN
        {
            return Err(MediaServerError::Unauthorized);
        }

        if !resp.status().is_success() {
            return Err(MediaServerError::ServerError(format!(
                "Failed to query system info: HTTP {}",
                resp.status()
            )));
        }

        let json: Value = resp
            .json()
            .await
            .map_err(|e| MediaServerError::ServerError(format!("Invalid system info JSON: {e}")))?;

        let id = json
            .get("Id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let server_name = json
            .get("ServerName")
            .and_then(|v| v.as_str())
            .unwrap_or("Jellyfin Server")
            .to_string();
        let version = json
            .get("Version")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let admin_user_id = self.resolve_admin_user_id().await;

        Ok(MediaServerInfo {
            id,
            server_name,
            version,
            has_subtitle_manager_plugin: true,
            admin_user_id,
            username: self.profile.username.clone(),
            server_type: MediaServerType::Jellyfin,
        })
    }

    async fn get_libraries(&self) -> Result<Vec<MediaServerLibrary>, MediaServerError> {
        let url = self.url("/Library/MediaFolders");
        let req = self.apply_auth(
            self.client
                .get(&url)
                .query(&[("api_key", &self.profile.api_key)]),
        );

        let resp = req
            .send()
            .await
            .map_err(|e| MediaServerError::Network(e.to_string()))?;

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED
            || resp.status() == reqwest::StatusCode::FORBIDDEN
        {
            return Err(MediaServerError::Unauthorized);
        }

        if !resp.status().is_success() {
            return Err(MediaServerError::ServerError(format!(
                "Failed to fetch libraries: HTTP {}",
                resp.status()
            )));
        }

        let json: Value = resp.json().await.map_err(|e| {
            MediaServerError::ServerError(format!("Invalid libraries response: {e}"))
        })?;

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
                    libraries.push(MediaServerLibrary {
                        id: id.to_string(),
                        name: name.to_string(),
                        collection_type,
                    });
                }
            }
        }

        Ok(libraries)
    }

    async fn get_items(
        &self,
        parent_id: Option<&str>,
        search_term: Option<&str>,
        limit: u32,
        start_index: u32,
    ) -> Result<MediaServerItemListResult, MediaServerError> {
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
            ("api_key", &self.profile.api_key),
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
            .map_err(|e| MediaServerError::Network(e.to_string()))?;

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED
            || resp.status() == reqwest::StatusCode::FORBIDDEN
        {
            return Err(MediaServerError::Unauthorized);
        }

        if !resp.status().is_success() {
            return Err(MediaServerError::ServerError(format!(
                "Failed to fetch items: HTTP {}",
                resp.status()
            )));
        }

        let json: Value = resp
            .json()
            .await
            .map_err(|e| MediaServerError::ServerError(format!("Invalid items response: {e}")))?;

        let total_record_count = json
            .get("TotalRecordCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        let items = if let Some(raw_items) = json.get("Items").and_then(|v| v.as_array()) {
            parse_media_items(raw_items)
        } else {
            Vec::new()
        };

        Ok(MediaServerItemListResult {
            items,
            total_record_count,
        })
    }

    async fn get_item_streams(
        &self,
        item_id: &str,
    ) -> Result<Vec<MediaServerStreamInfo>, MediaServerError> {
        let clean_id = validate_item_id(item_id)?;
        let url = self.url(&format!("/Items/{clean_id}"));
        let req = self.apply_auth(self.client.get(&url).query(&[
            ("Fields", "MediaStreams"),
            ("api_key", &self.profile.api_key),
        ]));
        let resp = req
            .send()
            .await
            .map_err(|e| MediaServerError::Network(e.to_string()))?;

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED
            || resp.status() == reqwest::StatusCode::FORBIDDEN
        {
            return Err(MediaServerError::Unauthorized);
        }

        if !resp.status().is_success() {
            return Err(MediaServerError::ServerError(format!(
                "Failed to get item details: HTTP {}",
                resp.status()
            )));
        }

        let raw: Value = resp
            .json()
            .await
            .map_err(|e| MediaServerError::ServerError(format!("Invalid item JSON: {e}")))?;

        if let Some(streams_val) = raw.get("MediaStreams") {
            let (streams, _, _) = parse_media_streams(streams_val);
            Ok(streams)
        } else {
            Ok(Vec::new())
        }
    }

    async fn download_audio_stream(
        &self,
        item_id: &str,
        stream_index: Option<i32>,
        destination_path: &Path,
    ) -> Result<u64, MediaServerError> {
        download_audio_stream_common(
            &self.client,
            &self.base_url,
            &self.profile.api_key,
            item_id,
            stream_index,
            destination_path,
        )
        .await
    }

    async fn upload_subtitle(
        &self,
        payload: &MediaServerSubtitleUploadPayload,
    ) -> Result<(), MediaServerError> {
        let clean_id = validate_item_id(&payload.item_id)?;
        let url = self.url(&format!("/Videos/{clean_id}/Subtitles"));

        let format = if payload.format.trim().is_empty() {
            "srt"
        } else {
            payload.format.trim()
        };

        let base64_data = BASE64_STANDARD.encode(payload.content.as_bytes());

        let body = serde_json::json!({
            "Format": format,
            "Data": base64_data,
            "Language": payload.language,
            "IsForced": payload.is_forced
        });

        let req = self.apply_auth(
            self.client
                .post(&url)
                .query(&[("api_key", &self.profile.api_key)])
                .json(&body),
        );

        let resp = req
            .send()
            .await
            .map_err(|e| MediaServerError::Network(e.to_string()))?;

        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(MediaServerError::Unauthorized);
        }

        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(MediaServerError::ServerError(
                "Item not found or Jellyfin version < 10.7 (endpoint /Videos/{itemId}/Subtitles returned 404)".to_string(),
            ));
        }

        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            let msg = if body_text.trim().is_empty() {
                format!("Failed to upload subtitle: HTTP {}", status)
            } else {
                format!(
                    "Failed to upload subtitle: HTTP {} - {}",
                    status,
                    body_text.trim()
                )
            };
            return Err(MediaServerError::ServerError(msg));
        }

        Ok(())
    }

    async fn delete_subtitle(
        &self,
        item_id: &str,
        subtitle_index: i32,
    ) -> Result<(), MediaServerError> {
        if subtitle_index < 0 {
            return Err(MediaServerError::InvalidConfig(
                "Subtitle index cannot be negative".into(),
            ));
        }
        let clean_id = validate_item_id(item_id)?;
        let url = self.url(&format!("/Videos/{clean_id}/Subtitles/{subtitle_index}"));

        let req = self.apply_auth(
            self.client
                .delete(&url)
                .query(&[("api_key", &self.profile.api_key)]),
        );

        let resp = req
            .send()
            .await
            .map_err(|e| MediaServerError::Network(e.to_string()))?;

        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(MediaServerError::Unauthorized);
        }

        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            let msg = if body_text.trim().is_empty() {
                format!("Failed to delete subtitle: HTTP {}", status)
            } else {
                format!(
                    "Failed to delete subtitle: HTTP {} - {}",
                    status,
                    body_text.trim()
                )
            };
            return Err(MediaServerError::ServerError(msg));
        }

        Ok(())
    }

    async fn refresh_item_metadata(&self, item_id: &str) -> Result<(), MediaServerError> {
        let clean_id = validate_item_id(item_id)?;
        let url = self.url(&format!("/Items/{clean_id}/Refresh"));
        let req = self.apply_auth(
            self.client
                .post(&url)
                .query(&[("api_key", &self.profile.api_key)]),
        );

        let resp = req
            .send()
            .await
            .map_err(|e| MediaServerError::Network(e.to_string()))?;

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED
            || resp.status() == reqwest::StatusCode::FORBIDDEN
        {
            return Err(MediaServerError::Unauthorized);
        }

        if !resp.status().is_success() {
            return Err(MediaServerError::ServerError(format!(
                "Failed to refresh item metadata: HTTP {}",
                resp.status()
            )));
        }
        tokio::time::sleep(Duration::from_millis(1500)).await;
        let item_url = self.url(&format!("/Items/{clean_id}"));
        let poll_req = self.apply_auth(self.client.get(&item_url).query(&[
            ("Fields", "MediaStreams"),
            ("api_key", &self.profile.api_key),
        ]));
        let _ = poll_req.send().await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jellyfin_client_upload_subtitle_base64_payload() {
        let raw_srt = "1\n00:00:01,000 --> 00:00:03,000\nHello World\n";
        let encoded = BASE64_STANDARD.encode(raw_srt.as_bytes());

        let payload_json = serde_json::json!({
            "Format": "srt",
            "Data": encoded,
            "Language": "eng",
            "IsForced": false
        });

        assert_eq!(payload_json["Format"], "srt");
        assert_eq!(payload_json["Language"], "eng");
        assert_eq!(payload_json["IsForced"], false);

        // Verify decoding matches original
        let decoded = BASE64_STANDARD
            .decode(payload_json["Data"].as_str().unwrap())
            .unwrap();
        assert_eq!(String::from_utf8(decoded).unwrap(), raw_srt);
    }

    #[test]
    fn test_jellyfin_client_delete_subtitle_url() {
        let profile = MediaServerProfile {
            id: "jf-1".to_string(),
            name: "Test Jellyfin".to_string(),
            server_type: MediaServerType::Jellyfin,
            server_url: "http://localhost:8096".to_string(),
            api_key: "test_key".to_string(),
            username: Some("admin".to_string()),
            admin_user_id: None,
            allow_invalid_certs: false,
            created_at: 1000,
            updated_at: 1000,
        };
        let client = JellyfinClient::new(profile).unwrap();
        let item_id = "abc123xyz";
        let sub_index = 3;
        let delete_url = client.url(&format!("/Videos/{item_id}/Subtitles/{sub_index}"));
        assert_eq!(
            delete_url,
            "http://localhost:8096/Videos/abc123xyz/Subtitles/3"
        );
    }
}
