use chrono::DateTime;
use reqwest::{Client, Method, StatusCode};
use roxmltree::{Document, Node};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const PROPFIND_BODY: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<propfind xmlns="DAV:">
  <prop>
    <displayname />
    <getcontentlength />
    <getlastmodified />
    <resourcetype />
  </prop>
</propfind>"#;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebDavConfigPayload {
    pub server_url: String,
    pub remote_dir: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBackupEntry {
    pub href: String,
    pub file_name: String,
    pub size: u64,
    pub modified_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WebDavConnectionStatus {
    Success,
    Warning,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebDavConnectionResult {
    pub status: WebDavConnectionStatus,
    pub message: String,
}

pub fn trim_required(value: &str, label: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{label} is required."));
    }

    Ok(trimmed.to_string())
}

pub fn normalize_config(config: &WebDavConfigPayload) -> Result<WebDavConfigPayload, String> {
    Ok(WebDavConfigPayload {
        server_url: parse_server_url(&config.server_url)?,
        remote_dir: config.remote_dir.trim().to_string(),
        username: trim_required(&config.username, "WebDAV username")?,
        password: trim_required(&config.password, "WebDAV password")?,
    })
}

pub fn parse_server_url(value: &str) -> Result<String, String> {
    let trimmed = trim_required(value, "WebDAV server URL")?;
    let mut url = url::Url::parse(&trimmed)
        .map_err(|error| format!("WebDAV server URL is invalid: {error}"))?;

    enforce_https_url(url.as_str(), "WebDAV server URL")?;

    let normalized_path = if url.path().ends_with('/') {
        url.path().to_string()
    } else {
        format!("{}/", url.path())
    };
    url.set_path(&normalized_path);

    Ok(url.to_string())
}

pub fn enforce_https_url(url: &str, label: &str) -> Result<(), String> {
    let parsed = url::Url::parse(url).map_err(|error| format!("{label} is invalid: {error}"))?;
    if parsed.scheme() != "https" {
        return Err(format!("{label} must start with https://."));
    }

    Ok(())
}

pub fn checked_webdav_request_url(url: &str, label: &str) -> Result<String, String> {
    enforce_https_url(url, label)?;
    Ok(url.to_string())
}

pub fn remote_dir_segments(remote_dir: &str) -> Vec<&str> {
    remote_dir
        .split('/')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .collect()
}

pub fn build_collection_url(base_url: &str, remote_dir: &str) -> Result<String, String> {
    let mut url = url::Url::parse(base_url)
        .map_err(|error| format!("WebDAV server URL is invalid: {error}"))?;
    let segments = remote_dir_segments(remote_dir);

    {
        let mut path_segments = url
            .path_segments_mut()
            .map_err(|_| "WebDAV server URL cannot be used as a directory base.".to_string())?;
        path_segments.pop_if_empty();
        for segment in segments {
            path_segments.push(segment);
        }
        path_segments.push("");
    }

    checked_webdav_request_url(url.as_str(), "WebDAV collection URL")
}

pub fn build_file_url(collection_url: &str, file_name: &str) -> Result<String, String> {
    let mut url = url::Url::parse(collection_url)
        .map_err(|error| format!("WebDAV collection URL is invalid: {error}"))?;
    {
        let mut path_segments = url
            .path_segments_mut()
            .map_err(|_| "WebDAV collection URL cannot be used as a file base.".to_string())?;
        path_segments.pop_if_empty();
        path_segments.push(file_name);
    }
    checked_webdav_request_url(url.as_str(), "WebDAV backup URL")
}

pub fn join_backup_href(base_url: &str, href: &str) -> Result<String, String> {
    let base = url::Url::parse(base_url)
        .map_err(|error| format!("WebDAV server URL is invalid: {error}"))?;
    let joined = base
        .join(href.trim())
        .map_err(|error| format!("WebDAV backup URL is invalid: {error}"))?;
    checked_webdav_request_url(joined.as_str(), "WebDAV backup URL")
}

fn find_descendant_text(node: Node<'_, '_>, name: &str) -> Option<String> {
    node.descendants()
        .find(|candidate| candidate.is_element() && candidate.tag_name().name() == name)
        .and_then(|candidate| candidate.text())
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn response_is_collection(node: Node<'_, '_>) -> bool {
    node.descendants()
        .any(|candidate| candidate.is_element() && candidate.tag_name().name() == "collection")
}

fn decode_file_name_from_href(resolved_url: &url::Url) -> Option<String> {
    resolved_url
        .path_segments()
        .and_then(|mut segments| segments.rfind(|segment| !segment.is_empty()))
        .and_then(|segment| urlencoding::decode(segment).ok())
        .map(|decoded| decoded.into_owned())
}

fn parse_http_date_timestamp(value: Option<&str>) -> i64 {
    value
        .and_then(|raw| DateTime::parse_from_rfc2822(raw).ok())
        .map(|parsed| parsed.timestamp())
        .unwrap_or(0)
}

fn normalize_collection_url(url: &str) -> Result<url::Url, String> {
    let normalized = build_collection_url(&parse_server_url(url)?, "")?;
    url::Url::parse(&normalized)
        .map_err(|error| format!("WebDAV collection URL is invalid: {error}"))
}

pub fn parse_propfind_entries(
    xml: &str,
    collection_url: &str,
) -> Result<Vec<RemoteBackupEntry>, String> {
    let document = Document::parse(xml)
        .map_err(|error| format!("Failed to parse the WebDAV response: {error}"))?;
    let normalized_collection = normalize_collection_url(collection_url)?;
    let mut entries = Vec::new();

    for response in document
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "response")
    {
        let href = match find_descendant_text(response, "href") {
            Some(value) => value,
            None => continue,
        };
        let resolved_url = normalized_collection
            .join(&href)
            .map_err(|error| format!("Failed to resolve a WebDAV entry URL: {error}"))?;

        if response_is_collection(response) {
            continue;
        }

        let file_name = find_descendant_text(response, "displayname")
            .filter(|value| !value.is_empty())
            .or_else(|| decode_file_name_from_href(&resolved_url))
            .unwrap_or_default();

        if !file_name.ends_with(".tar.bz2") {
            continue;
        }

        let size = find_descendant_text(response, "getcontentlength")
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0);
        let modified_at = find_descendant_text(response, "getlastmodified");

        entries.push(RemoteBackupEntry {
            href: resolved_url.to_string(),
            file_name,
            size,
            modified_at,
        });
    }

    entries.sort_by(|left, right| {
        parse_http_date_timestamp(right.modified_at.as_deref())
            .cmp(&parse_http_date_timestamp(left.modified_at.as_deref()))
            .then_with(|| left.file_name.cmp(&right.file_name))
    });

    Ok(entries)
}

pub fn resolve_warning_message(collection_missing: bool) -> WebDavConnectionResult {
    let mut warnings = Vec::new();
    if collection_missing {
        warnings.push("Connected to the WebDAV server, but the remote directory does not exist yet. It will be created on the first upload.".to_string());
    }

    if warnings.is_empty() {
        return WebDavConnectionResult {
            status: WebDavConnectionStatus::Success,
            message: "WebDAV connection is ready.".to_string(),
        };
    }

    WebDavConnectionResult {
        status: WebDavConnectionStatus::Warning,
        message: warnings.join(" "),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CollectionProbe {
    Exists,
    Missing,
}

fn build_client() -> Result<Client, String> {
    Client::builder()
        .user_agent("Sona/1.0")
        .https_only(true)
        .build()
        .map_err(|error| format!("Failed to create the WebDAV client: {error}"))
}

async fn propfind(
    client: &Client,
    config: &WebDavConfigPayload,
    url: String,
    depth: &str,
) -> Result<reqwest::Response, String> {
    let url = checked_webdav_request_url(&url, "WebDAV request URL")?;
    client
        .request(
            Method::from_bytes(b"PROPFIND")
                .map_err(|error| format!("Failed to build the PROPFIND request: {error}"))?,
            url,
        )
        .basic_auth(&config.username, Some(&config.password))
        .header("Depth", depth)
        .header("Content-Type", "application/xml; charset=utf-8")
        .body(PROPFIND_BODY)
        .send()
        .await
        .map_err(|error| format!("WebDAV request failed: {error}"))
}

async fn probe_collection(
    client: &Client,
    config: &WebDavConfigPayload,
    collection_url: &str,
) -> Result<CollectionProbe, String> {
    let response = propfind(client, config, collection_url.to_string(), "0").await?;
    match response.status() {
        StatusCode::MULTI_STATUS | StatusCode::OK => Ok(CollectionProbe::Exists),
        StatusCode::NOT_FOUND => Ok(CollectionProbe::Missing),
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(format!(
            "WebDAV authentication failed with status {}.",
            response.status()
        )),
        status => Err(format!(
            "WebDAV connection check failed with status {status}."
        )),
    }
}

async fn ensure_remote_directory(
    client: &Client,
    config: &WebDavConfigPayload,
    base_url: &str,
) -> Result<String, String> {
    let collection_url = build_collection_url(base_url, &config.remote_dir)?;
    if probe_collection(client, config, &collection_url).await? == CollectionProbe::Exists {
        return Ok(collection_url);
    }

    let segments = remote_dir_segments(&config.remote_dir);
    if segments.is_empty() {
        return Ok(collection_url);
    }

    let mut accumulated = Vec::new();
    for segment in segments {
        accumulated.push(segment);
        let segment_path = accumulated.join("/");
        let segment_url = build_collection_url(base_url, &segment_path)?;
        if probe_collection(client, config, &segment_url).await? == CollectionProbe::Exists {
            continue;
        }

        let response = client
            .request(
                Method::from_bytes(b"MKCOL")
                    .map_err(|error| format!("Failed to build the MKCOL request: {error}"))?,
                checked_webdav_request_url(&segment_url, "WebDAV directory URL")?,
            )
            .basic_auth(&config.username, Some(&config.password))
            .send()
            .await
            .map_err(|error| format!("Failed to create the WebDAV directory: {error}"))?;

        match response.status() {
            StatusCode::CREATED
            | StatusCode::OK
            | StatusCode::NO_CONTENT
            | StatusCode::METHOD_NOT_ALLOWED => {}
            status => {
                return Err(format!(
                    "Failed to create the WebDAV directory \"{segment_path}\" with status {status}."
                ));
            }
        }
    }

    Ok(collection_url)
}

pub async fn webdav_test_connection(
    config: WebDavConfigPayload,
) -> Result<WebDavConnectionResult, String> {
    let normalized = normalize_config(&config)?;
    let client = build_client()?;
    let base_url = parse_server_url(&normalized.server_url)?;
    let collection_url = build_collection_url(&base_url, &normalized.remote_dir)?;
    let probe = probe_collection(&client, &normalized, &collection_url).await?;

    Ok(resolve_warning_message(probe == CollectionProbe::Missing))
}

pub async fn webdav_list_backups(
    config: WebDavConfigPayload,
) -> Result<Vec<RemoteBackupEntry>, String> {
    let normalized = normalize_config(&config)?;
    let client = build_client()?;
    let base_url = parse_server_url(&normalized.server_url)?;
    let collection_url = build_collection_url(&base_url, &normalized.remote_dir)?;

    if probe_collection(&client, &normalized, &collection_url).await? == CollectionProbe::Missing {
        return Ok(Vec::new());
    }

    let response = propfind(&client, &normalized, collection_url.clone(), "1").await?;
    match response.status() {
        StatusCode::MULTI_STATUS | StatusCode::OK => {
            let body = response
                .text()
                .await
                .map_err(|error| format!("Failed to read the WebDAV listing response: {error}"))?;
            parse_propfind_entries(&body, &collection_url)
        }
        status => Err(format!(
            "Failed to list WebDAV backups with status {status}."
        )),
    }
}

pub async fn webdav_upload_backup(
    config: WebDavConfigPayload,
    local_archive_path: String,
) -> Result<(), String> {
    let normalized = normalize_config(&config)?;
    let client = build_client()?;
    let base_url = parse_server_url(&normalized.server_url)?;
    let collection_url = ensure_remote_directory(&client, &normalized, &base_url).await?;
    let file_name = Path::new(&local_archive_path)
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Backup archive path is missing a file name.".to_string())?;
    let target_url = build_file_url(&collection_url, file_name)?;
    let archive_bytes = tokio::fs::read(&local_archive_path)
        .await
        .map_err(|error| format!("Failed to read the local backup archive: {error}"))?;

    let response = client
        .put(target_url)
        .basic_auth(&normalized.username, Some(&normalized.password))
        .header("Content-Type", "application/x-bzip2")
        .body(archive_bytes)
        .send()
        .await
        .map_err(|error| format!("Failed to upload the backup archive to WebDAV: {error}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "WebDAV upload failed with status {}.",
            response.status()
        ));
    }

    Ok(())
}

pub async fn webdav_download_backup(
    config: WebDavConfigPayload,
    href: String,
    output_path: String,
) -> Result<(), String> {
    let normalized = normalize_config(&config)?;
    let client = build_client()?;
    let base_url = parse_server_url(&normalized.server_url)?;
    let backup_url = join_backup_href(&base_url, &href)?;

    if let Some(parent) = Path::new(&output_path).parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|error| {
            format!("Failed to prepare the temporary download directory: {error}")
        })?;
    }

    let response = client
        .get(backup_url)
        .basic_auth(&normalized.username, Some(&normalized.password))
        .send()
        .await
        .map_err(|error| format!("Failed to download the WebDAV backup archive: {error}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "WebDAV download failed with status {}.",
            response.status()
        ));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("Failed to read the WebDAV backup archive: {error}"))?;

    if let Err(error) = tokio::fs::write(&output_path, &bytes).await {
        let _ = tokio::fs::remove_file(&output_path).await;
        return Err(format!(
            "Failed to save the downloaded backup archive: {error}"
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{webdav_download_backup, webdav_test_connection};
    use crate::WebDavConfigPayload;

    fn insecure_config() -> WebDavConfigPayload {
        WebDavConfigPayload {
            server_url: "http://dav.example.com/root".to_string(),
            remote_dir: "backups/sona".to_string(),
            username: "demo".to_string(),
            password: "secret".to_string(),
        }
    }

    #[tokio::test]
    async fn test_connection_rejects_http_before_network_request() {
        let error = webdav_test_connection(insecure_config()).await.unwrap_err();

        assert_eq!(error, "WebDAV server URL must start with https://.");
    }

    #[tokio::test]
    async fn download_rejects_http_before_writing_output_file() {
        let output_dir = tempfile::tempdir().unwrap();
        let output_path = output_dir.path().join("backup.tar.bz2");

        let error = webdav_download_backup(
            insecure_config(),
            "/remote.php/dav/files/demo/backups/sona.tar.bz2".to_string(),
            output_path.to_string_lossy().into_owned(),
        )
        .await
        .unwrap_err();

        assert_eq!(error, "WebDAV server URL must start with https://.");
        assert!(!output_path.exists());
    }
}
