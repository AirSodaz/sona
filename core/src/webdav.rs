use chrono::DateTime;
use roxmltree::{Document, Node};
use serde::{Deserialize, Serialize};

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

#[cfg(test)]
mod tests {
    use super::{WebDavConnectionStatus, build_collection_url, parse_propfind_entries};

    #[test]
    fn parse_server_url_rejects_http_before_credentials_are_sent() {
        let error = super::parse_server_url("http://nas.local/dav").unwrap_err();

        assert_eq!(error, "WebDAV server URL must start with https://.");
    }

    #[test]
    fn parse_server_url_accepts_https() {
        let result = super::parse_server_url("https://dav.example.com/root").unwrap();

        assert_eq!(result.as_str(), "https://dav.example.com/root/");
    }

    #[test]
    fn checked_webdav_request_url_rejects_http_before_credentials_are_sent() {
        let error = super::checked_webdav_request_url(
            "http://dav.example.com/backups/sona.tar.bz2",
            "WebDAV backup URL",
        )
        .unwrap_err();

        assert_eq!(error, "WebDAV backup URL must start with https://.");
    }

    #[test]
    fn build_collection_url_appends_nested_remote_directory() {
        let base_url = "https://dav.example.com/remote.php/dav/files/demo/";

        let result = build_collection_url(base_url, "/backups/sona/").unwrap();

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
        let collection_url = "https://dav.example.com/remote.php/dav/files/demo/backups/sona/";

        let result = parse_propfind_entries(xml, collection_url).unwrap();

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
    fn resolve_warning_message_returns_success_when_collection_exists() {
        let result = super::resolve_warning_message(false);

        assert_eq!(result.status, WebDavConnectionStatus::Success);
        assert_eq!(result.message, "WebDAV connection is ready.");
    }
}
