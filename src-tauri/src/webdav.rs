use chrono::DateTime;
use reqwest::{Client, Method, StatusCode, Url};
use roxmltree::{Document, Node};
use serde::{Deserialize, Serialize};
use std::path::Path;

const PROPFIND_BODY: &str = r#"<?xml version="1.0" encoding="utf-8"?>
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CollectionProbe {
    Exists,
    Missing,
}

fn build_client() -> Result<Client, String> {
    Client::builder()
        .user_agent("Sona/1.0")
        .build()
        .map_err(|error| format!("Failed to create the WebDAV client: {error}"))
}

fn trim_required(value: &str, label: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{label} is required."));
    }

    Ok(trimmed.to_string())
}

fn parse_server_url(value: &str) -> Result<Url, String> {
    let trimmed = trim_required(value, "WebDAV server URL")?;
    let mut url =
        Url::parse(&trimmed).map_err(|error| format!("WebDAV server URL is invalid: {error}"))?;

    match url.scheme() {
        "http" | "https" => {}
        _ => {
            return Err("WebDAV server URL must start with http:// or https://.".to_string());
        }
    }

    let normalized_path = if url.path().ends_with('/') {
        url.path().to_string()
    } else {
        format!("{}/", url.path())
    };
    url.set_path(&normalized_path);

    Ok(url)
}

fn normalize_config(config: &WebDavConfigPayload) -> Result<WebDavConfigPayload, String> {
    Ok(WebDavConfigPayload {
        server_url: parse_server_url(&config.server_url)?.to_string(),
        remote_dir: config.remote_dir.trim().to_string(),
        username: trim_required(&config.username, "WebDAV username")?,
        password: trim_required(&config.password, "WebDAV password")?,
    })
}

fn remote_dir_segments(remote_dir: &str) -> Vec<&str> {
    remote_dir
        .split('/')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .collect()
}

fn build_collection_url(base_url: &Url, remote_dir: &str) -> Result<Url, String> {
    let mut url = base_url.clone();
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

    Ok(url)
}

fn build_file_url(collection_url: &Url, file_name: &str) -> Result<Url, String> {
    let mut url = collection_url.clone();
    {
        let mut path_segments = url
            .path_segments_mut()
            .map_err(|_| "WebDAV collection URL cannot be used as a file base.".to_string())?;
        path_segments.pop_if_empty();
        path_segments.push(file_name);
    }
    Ok(url)
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

fn decode_file_name_from_href(resolved_url: &Url) -> Option<String> {
    resolved_url
        .path_segments()
        .and_then(|segments| segments.filter(|segment| !segment.is_empty()).last())
        .and_then(|segment| urlencoding::decode(segment).ok())
        .map(|decoded| decoded.into_owned())
}

fn parse_http_date_timestamp(value: Option<&str>) -> i64 {
    value
        .and_then(|raw| DateTime::parse_from_rfc2822(raw).ok())
        .map(|parsed| parsed.timestamp())
        .unwrap_or(0)
}

fn normalize_collection_url(url: &Url) -> Result<Url, String> {
    build_collection_url(&parse_server_url(url.as_str())?, "")
}

fn parse_propfind_entries(
    xml: &str,
    collection_url: &Url,
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
            let resolved_collection = build_collection_url(&resolved_url, "")?;
            if resolved_collection == normalized_collection {
                continue;
            }
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

async fn propfind(
    client: &Client,
    config: &WebDavConfigPayload,
    url: Url,
    depth: &str,
) -> Result<reqwest::Response, String> {
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
    collection_url: &Url,
) -> Result<CollectionProbe, String> {
    let response = propfind(client, config, collection_url.clone(), "0").await?;
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
    base_url: &Url,
) -> Result<Url, String> {
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
                segment_url.clone(),
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

fn resolve_warning_message(collection_missing: bool, uses_http: bool) -> WebDavConnectionResult {
    let mut warnings = Vec::new();
    if collection_missing {
        warnings.push("Connected to the WebDAV server, but the remote directory does not exist yet. It will be created on the first upload.".to_string());
    }
    if uses_http {
        warnings.push("This WebDAV endpoint uses HTTP, so credentials and backup archives are not protected in transit.".to_string());
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

#[tauri::command]
pub async fn webdav_test_connection(
    config: WebDavConfigPayload,
) -> Result<WebDavConnectionResult, String> {
    let normalized = normalize_config(&config)?;
    let client = build_client()?;
    let base_url = parse_server_url(&normalized.server_url)?;
    let collection_url = build_collection_url(&base_url, &normalized.remote_dir)?;
    let probe = probe_collection(&client, &normalized, &collection_url).await?;

    Ok(resolve_warning_message(
        probe == CollectionProbe::Missing,
        base_url.scheme() == "http",
    ))
}

#[tauri::command]
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

#[tauri::command]
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

#[tauri::command]
pub async fn webdav_download_backup(
    config: WebDavConfigPayload,
    href: String,
    output_path: String,
) -> Result<(), String> {
    let normalized = normalize_config(&config)?;
    let client = build_client()?;
    let base_url = parse_server_url(&normalized.server_url)?;
    let backup_url = base_url
        .join(href.trim())
        .map_err(|error| format!("WebDAV backup URL is invalid: {error}"))?;

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
    use super::{
        build_collection_url, parse_propfind_entries, resolve_warning_message,
        WebDavConnectionStatus,
    };
    use reqwest::Url;

    #[test]
    fn build_collection_url_appends_nested_remote_directory() {
        let base_url = Url::parse("https://dav.example.com/remote.php/dav/files/demo/").unwrap();

        let result = build_collection_url(&base_url, "/backups/sona/").unwrap();

        assert_eq!(
            result.as_str(),
            "https://dav.example.com/remote.php/dav/files/demo/backups/sona/"
        );
    }

    #[test]
    fn parse_propfind_entries_keeps_only_tar_bz2_files_and_sorts_by_modified_time() {
        let xml = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:">
  <d:response>
    <d:href>/remote.php/dav/files/demo/backups/sona/</d:href>
    <d:propstat>
      <d:prop>
        <d:displayname>sona</d:displayname>
        <d:resourcetype><d:collection /></d:resourcetype>
      </d:prop>
    </d:propstat>
  </d:response>
  <d:response>
    <d:href>/remote.php/dav/files/demo/backups/sona/sona-backup-2026-04-28_01-00-00.tar.bz2</d:href>
    <d:propstat>
      <d:prop>
        <d:getcontentlength>100</d:getcontentlength>
        <d:getlastmodified>Tue, 28 Apr 2026 01:00:00 GMT</d:getlastmodified>
      </d:prop>
    </d:propstat>
  </d:response>
  <d:response>
    <d:href>/remote.php/dav/files/demo/backups/sona/notes.txt</d:href>
    <d:propstat>
      <d:prop>
        <d:getcontentlength>12</d:getcontentlength>
      </d:prop>
    </d:propstat>
  </d:response>
  <d:response>
    <d:href>/remote.php/dav/files/demo/backups/sona/sona-backup-2026-04-29_01-00-00.tar.bz2</d:href>
    <d:propstat>
      <d:prop>
        <d:getcontentlength>200</d:getcontentlength>
        <d:getlastmodified>Wed, 29 Apr 2026 01:00:00 GMT</d:getlastmodified>
      </d:prop>
    </d:propstat>
  </d:response>
</d:multistatus>"#;
        let collection_url =
            Url::parse("https://dav.example.com/remote.php/dav/files/demo/backups/sona/").unwrap();

        let result = parse_propfind_entries(xml, &collection_url).unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(
            result[0].file_name,
            "sona-backup-2026-04-29_01-00-00.tar.bz2"
        );
        assert_eq!(
            result[1].file_name,
            "sona-backup-2026-04-28_01-00-00.tar.bz2"
        );
    }

    #[test]
    fn resolve_warning_message_marks_http_connections_as_warnings() {
        let result = resolve_warning_message(false, true);

        assert_eq!(result.status, WebDavConnectionStatus::Warning);
        assert!(result.message.contains("HTTP"));
    }
}
