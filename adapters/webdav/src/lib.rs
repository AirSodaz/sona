use reqwest::{Client, Method, StatusCode};
use sona_core::webdav::{
    PROPFIND_BODY, RemoteBackupEntry, WebDavConfigPayload, WebDavConnectionResult,
    build_collection_url, build_file_url, checked_webdav_request_url, join_backup_href,
    normalize_config, parse_propfind_entries, parse_server_url, remote_dir_segments,
    resolve_warning_message,
};
use std::path::Path;

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
    use sona_core::webdav::WebDavConfigPayload;

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
