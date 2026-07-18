use std::collections::{BTreeSet, VecDeque};
use std::sync::Arc;

use async_trait::async_trait;
use reqwest::header::{ETAG, IF_MATCH, IF_NONE_MATCH};
use reqwest::{Client, Method, StatusCode};
use roxmltree::{Document, Node};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sona_core::sync::{
    SyncDeleteResult, SyncError, SyncListPage, SyncObject, SyncObjectKey, SyncObjectMetadata,
    SyncObjectPrefix, SyncObjectStore, SyncObjectStoreCapabilities, SyncProviderDescriptor,
    SyncPutResult,
};
use sona_sync::{SyncProvider, SyncProviderFactory};
use url::Url;

const PROPFIND_BODY: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<propfind xmlns="DAV:">
  <prop>
    <getetag />
    <getcontentlength />
    <getlastmodified />
    <resourcetype />
  </prop>
</propfind>"#;
const LIST_PAGE_SIZE: usize = 1_000;
const MAX_OBJECT_BYTES: usize = 72 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default)]
pub struct WebDavSyncProviderFactory;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PersistedWebDavConfig {
    server_url: String,
    remote_root: String,
    username: String,
}

#[async_trait]
impl SyncProviderFactory for WebDavSyncProviderFactory {
    fn provider_id(&self) -> &str {
        "webdav"
    }

    fn credential_secret_key(&self, vault_id: &str) -> String {
        format!("webdav-password:{vault_id}")
    }

    async fn prepare(&self, configuration: Value) -> Result<SyncProvider, SyncError> {
        let config: WebDavObjectStoreConfig =
            serde_json::from_value(configuration).map_err(provider_configuration_error)?;
        build_provider(config)
    }

    async fn restore(
        &self,
        persisted_configuration: Value,
        credential: Vec<u8>,
    ) -> Result<SyncProvider, SyncError> {
        let persisted: PersistedWebDavConfig = serde_json::from_value(persisted_configuration)
            .map_err(provider_configuration_error)?;
        let password = String::from_utf8(credential)
            .map_err(|_| store_error("WebDAV provider credential must be valid UTF-8."))?;
        build_provider(WebDavObjectStoreConfig {
            server_url: persisted.server_url,
            remote_root: persisted.remote_root,
            username: persisted.username,
            password,
        })
    }
}

fn build_provider(config: WebDavObjectStoreConfig) -> Result<SyncProvider, SyncError> {
    let config = WebDavObjectStoreConfig::new(
        &config.server_url,
        &config.remote_root,
        &config.username,
        &config.password,
    )?;
    let persisted_configuration = serde_json::to_value(PersistedWebDavConfig {
        server_url: config.server_url.clone(),
        remote_root: config.remote_root.clone(),
        username: config.username.clone(),
    })
    .map_err(provider_configuration_error)?;
    let credential = config.password.as_bytes().to_vec();
    let store = Arc::new(WebDavObjectStore::new(config)?);
    Ok(SyncProvider {
        descriptor: SyncProviderDescriptor {
            id: "webdav".to_string(),
            display_name: "WebDAV".to_string(),
        },
        store,
        persisted_configuration,
        credential,
    })
}

fn provider_configuration_error(error: impl std::fmt::Display) -> SyncError {
    store_error(format!("WebDAV provider configuration is invalid: {error}"))
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebDavObjectStoreConfig {
    pub server_url: String,
    pub remote_root: String,
    pub username: String,
    pub password: String,
}

impl std::fmt::Debug for WebDavObjectStoreConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WebDavObjectStoreConfig")
            .field("server_url", &self.server_url)
            .field("remote_root", &self.remote_root)
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .finish()
    }
}

impl WebDavObjectStoreConfig {
    pub fn new(
        server_url: impl AsRef<str>,
        remote_root: impl AsRef<str>,
        username: impl AsRef<str>,
        password: impl AsRef<str>,
    ) -> Result<Self, SyncError> {
        let server_url = parse_server_url(server_url.as_ref())?.to_string();
        let username = required(username.as_ref(), "WebDAV username")?;
        let password = required(password.as_ref(), "WebDAV password")?;
        Ok(Self {
            server_url,
            remote_root: remote_root.as_ref().trim().to_string(),
            username,
            password,
        })
    }
}

#[derive(Clone)]
pub struct WebDavObjectStore {
    config: WebDavObjectStoreConfig,
    client: Client,
    server_url: Url,
    root_url: Url,
}

impl WebDavObjectStore {
    pub fn new(config: WebDavObjectStoreConfig) -> Result<Self, SyncError> {
        let config = WebDavObjectStoreConfig::new(
            &config.server_url,
            &config.remote_root,
            &config.username,
            &config.password,
        )?;
        let server_url = parse_server_url(&config.server_url)?;
        let root_url = build_collection_url(server_url.as_str(), &config.remote_root)?;
        let client = Client::builder()
            .user_agent("Sona/1.0")
            .https_only(true)
            .redirect(webdav_redirect_policy(server_url.clone(), root_url.clone()))
            .build()
            .map_err(|error| store_error(format!("Failed to create WebDAV client: {error}")))?;
        Ok(Self {
            config,
            client,
            server_url,
            root_url,
        })
    }

    async fn propfind(&self, url: Url, depth: &str) -> Result<reqwest::Response, SyncError> {
        let response = self
            .client
            .request(webdav_method(b"PROPFIND")?, url)
            .basic_auth(&self.config.username, Some(&self.config.password))
            .header("Depth", depth)
            .header("Content-Type", "application/xml; charset=utf-8")
            .body(PROPFIND_BODY)
            .send()
            .await
            .map_err(|error| store_error(format!("WebDAV PROPFIND failed: {error}")))?;
        self.validate_response_url(response.url())?;
        Ok(response)
    }

    async fn probe_collection(&self, url: Url) -> Result<bool, SyncError> {
        let response = self.propfind(url, "0").await?;
        match response.status() {
            StatusCode::OK | StatusCode::MULTI_STATUS => Ok(true),
            StatusCode::NOT_FOUND => Ok(false),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(store_error(format!(
                "WebDAV authentication failed with status {}.",
                response.status()
            ))),
            status => Err(store_error(format!(
                "WebDAV collection probe failed with status {status}."
            ))),
        }
    }

    async fn ensure_collection(&self, collection_url: &Url) -> Result<(), SyncError> {
        ensure_url_within_root_or_server(collection_url, &self.server_url, &self.root_url)?;
        if self.probe_collection(collection_url.clone()).await? {
            return Ok(());
        }

        let server_segments = path_segments(&self.server_url)?;
        let target_segments = path_segments(collection_url)?;
        if target_segments.len() < server_segments.len()
            || target_segments[..server_segments.len()] != server_segments
        {
            return Err(store_error(
                "WebDAV collection is outside the configured server root.",
            ));
        }
        let mut current = self.server_url.clone();
        for segment in &target_segments[server_segments.len()..] {
            append_segment(&mut current, segment, true)?;
            if self.probe_collection(current.clone()).await? {
                continue;
            }
            let response = self
                .client
                .request(webdav_method(b"MKCOL")?, current.clone())
                .basic_auth(&self.config.username, Some(&self.config.password))
                .send()
                .await
                .map_err(|error| store_error(format!("WebDAV MKCOL failed: {error}")))?;
            self.validate_response_url(response.url())?;
            match response.status() {
                StatusCode::CREATED
                | StatusCode::OK
                | StatusCode::NO_CONTENT
                | StatusCode::METHOD_NOT_ALLOWED => {}
                status => {
                    return Err(store_error(format!(
                        "WebDAV MKCOL failed with status {status}."
                    )));
                }
            }
        }
        Ok(())
    }

    async fn ensure_parent_collection(&self, key: &SyncObjectKey) -> Result<(), SyncError> {
        let parent = self.parent_collection_url(key)?;
        self.ensure_collection(&parent).await
    }

    fn parent_collection_url(&self, key: &SyncObjectKey) -> Result<Url, SyncError> {
        let mut parent = build_object_url(&self.root_url, key)?;
        {
            let mut segments = parent.path_segments_mut().map_err(|_| {
                store_error("WebDAV object URL cannot be used as a collection base.")
            })?;
            segments.pop();
            segments.push("");
        }
        Ok(parent)
    }

    async fn propfind_object_metadata(
        &self,
        key: &SyncObjectKey,
    ) -> Result<Option<SyncObjectMetadata>, SyncError> {
        let response = self.propfind(self.parent_collection_url(key)?, "1").await?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !matches!(response.status(), StatusCode::OK | StatusCode::MULTI_STATUS) {
            return Err(store_error(format!(
                "WebDAV object metadata lookup failed with status {}.",
                response.status()
            )));
        }
        let body = response.text().await.map_err(|error| {
            store_error(format!("Failed to read WebDAV metadata response: {error}"))
        })?;
        Ok(parse_propfind_objects(&body, &self.root_url)?
            .into_iter()
            .find(|entry| !entry.is_collection && entry.relative_path == key.as_str())
            .map(|entry| SyncObjectMetadata {
                key: key.clone(),
                etag: entry.etag,
                size: entry.size,
                modified_at: entry.modified_at,
            }))
    }

    fn validate_response_url(&self, url: &Url) -> Result<(), SyncError> {
        enforce_https(url, "WebDAV response URL")?;
        ensure_same_origin(url, &self.server_url)?;
        let root_path = self.root_url.path();
        if url.path() != root_path.trim_end_matches('/') && !url.path().starts_with(root_path) {
            return Err(store_error(
                "WebDAV response URL is outside the configured remote root.",
            ));
        }
        Ok(())
    }

    async fn list_all(
        &self,
        prefix: &SyncObjectPrefix,
    ) -> Result<Vec<SyncObjectMetadata>, SyncError> {
        let mut start_url = if prefix.as_str().is_empty() {
            self.root_url.clone()
        } else {
            let key = SyncObjectKey::parse(prefix.as_str())?;
            build_object_url(&self.root_url, &key)?
        };
        ensure_trailing_slash(&mut start_url);
        if !self.probe_collection(start_url.clone()).await? {
            return Ok(Vec::new());
        }

        let mut queue = VecDeque::from([start_url]);
        let mut visited = BTreeSet::new();
        let mut objects = Vec::new();
        while let Some(collection_url) = queue.pop_front() {
            if !visited.insert(collection_url.to_string()) {
                continue;
            }
            let response = self.propfind(collection_url, "1").await?;
            if !matches!(response.status(), StatusCode::OK | StatusCode::MULTI_STATUS) {
                return Err(store_error(format!(
                    "WebDAV listing failed with status {}.",
                    response.status()
                )));
            }
            let body = response.text().await.map_err(|error| {
                store_error(format!("Failed to read WebDAV listing response: {error}"))
            })?;
            for entry in parse_propfind_objects(&body, &self.root_url)? {
                if !entry.relative_path.starts_with(prefix.as_str()) {
                    continue;
                }
                if entry.is_collection {
                    let key = SyncObjectKey::parse(entry.relative_path)?;
                    let mut url = build_object_url(&self.root_url, &key)?;
                    ensure_trailing_slash(&mut url);
                    queue.push_back(url);
                } else {
                    objects.push(SyncObjectMetadata {
                        key: SyncObjectKey::parse(entry.relative_path)?,
                        etag: entry.etag,
                        size: entry.size,
                        modified_at: entry.modified_at,
                    });
                }
            }
        }
        objects.sort_by(|left, right| left.key.cmp(&right.key));
        objects.dedup_by(|left, right| left.key == right.key);
        Ok(objects)
    }
}

#[async_trait]
impl SyncObjectStore for WebDavObjectStore {
    async fn probe(&self) -> Result<SyncObjectStoreCapabilities, SyncError> {
        self.ensure_collection(&self.root_url).await?;
        let key = SyncObjectKey::parse(format!(".sona-sync-probe/{}", uuid::Uuid::new_v4()))?;
        let created = self.put_if_absent(&key, b"probe-a".to_vec()).await?;
        let created_etag = match created {
            SyncPutResult::Created { etag: Some(etag) } => etag,
            SyncPutResult::Created { etag: None } => {
                let _ = self.delete(&key, None).await;
                return Err(store_error(
                    "WebDAV server must return ETag values for sync objects.",
                ));
            }
            _ => return Err(store_error("WebDAV sync probe object already exists.")),
        };
        let result = async {
            let object = self
                .get(&key)
                .await?
                .ok_or_else(|| store_error("WebDAV sync probe object could not be read."))?;
            if object.bytes != b"probe-a" {
                return Err(store_error(
                    "WebDAV sync probe object changed after upload.",
                ));
            }
            let updated = self
                .compare_and_swap(&key, Some(&created_etag), b"probe-b".to_vec())
                .await?;
            let updated_etag = match updated {
                SyncPutResult::Created { etag: Some(etag) } => etag,
                _ => return Err(store_error("WebDAV conditional update is unsupported.")),
            };
            match self.delete(&key, Some(&updated_etag)).await? {
                SyncDeleteResult::Deleted => Ok(SyncObjectStoreCapabilities {
                    conditional_create: true,
                    compare_and_swap: true,
                    delete: true,
                }),
                _ => Err(store_error("WebDAV conditional delete is unsupported.")),
            }
        }
        .await;
        if result.is_err() {
            let _ = self.delete(&key, None).await;
        }
        result
    }

    async fn list(
        &self,
        prefix: &SyncObjectPrefix,
        continuation: Option<&str>,
    ) -> Result<SyncListPage, SyncError> {
        let objects = self.list_all(prefix).await?;
        let offset = match continuation {
            Some(value) => value
                .parse::<usize>()
                .map_err(|_| store_error("WebDAV continuation token is invalid."))?,
            None => 0,
        };
        if offset > objects.len() {
            return Err(store_error("WebDAV continuation token is out of range."));
        }
        let end = offset.saturating_add(LIST_PAGE_SIZE).min(objects.len());
        Ok(SyncListPage {
            objects: objects[offset..end].to_vec(),
            continuation: (end < objects.len()).then(|| end.to_string()),
        })
    }

    async fn get(&self, key: &SyncObjectKey) -> Result<Option<SyncObject>, SyncError> {
        let url = build_object_url(&self.root_url, key)?;
        let response = self
            .client
            .get(url)
            .basic_auth(&self.config.username, Some(&self.config.password))
            .send()
            .await
            .map_err(|error| store_error(format!("WebDAV GET failed: {error}")))?;
        self.validate_response_url(response.url())?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(store_error(format!(
                "WebDAV GET failed with status {}.",
                response.status()
            )));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_OBJECT_BYTES as u64)
        {
            return Err(store_error(
                "WebDAV object exceeds the supported size limit.",
            ));
        }
        let mut etag = response_header(&response, ETAG);
        let mut modified_at = response_header(&response, reqwest::header::LAST_MODIFIED);
        let bytes = response
            .bytes()
            .await
            .map_err(|error| store_error(format!("Failed to read WebDAV object: {error}")))?;
        if bytes.len() > MAX_OBJECT_BYTES {
            return Err(store_error(
                "WebDAV object exceeds the supported size limit.",
            ));
        }
        if etag.is_none()
            && let Some(metadata) = self.propfind_object_metadata(key).await?
        {
            etag = metadata.etag;
            modified_at = modified_at.or(metadata.modified_at);
        }
        Ok(Some(SyncObject {
            metadata: SyncObjectMetadata {
                key: key.clone(),
                etag,
                size: bytes.len() as u64,
                modified_at,
            },
            bytes: bytes.to_vec(),
        }))
    }

    async fn put_if_absent(
        &self,
        key: &SyncObjectKey,
        bytes: Vec<u8>,
    ) -> Result<SyncPutResult, SyncError> {
        self.ensure_parent_collection(key).await?;
        self.put(key, Some((IF_NONE_MATCH, "*")), bytes).await
    }

    async fn compare_and_swap(
        &self,
        key: &SyncObjectKey,
        expected_etag: Option<&str>,
        bytes: Vec<u8>,
    ) -> Result<SyncPutResult, SyncError> {
        self.ensure_parent_collection(key).await?;
        let condition = expected_etag
            .map(|etag| (IF_MATCH, etag))
            .or(Some((IF_NONE_MATCH, "*")));
        self.put(key, condition, bytes).await
    }

    async fn delete(
        &self,
        key: &SyncObjectKey,
        expected_etag: Option<&str>,
    ) -> Result<SyncDeleteResult, SyncError> {
        let url = build_object_url(&self.root_url, key)?;
        let mut request = self
            .client
            .delete(url)
            .basic_auth(&self.config.username, Some(&self.config.password));
        if let Some(etag) = expected_etag {
            request = request.header(IF_MATCH, etag);
        }
        let response = request
            .send()
            .await
            .map_err(|error| store_error(format!("WebDAV DELETE failed: {error}")))?;
        self.validate_response_url(response.url())?;
        match response.status() {
            status if status.is_success() => Ok(SyncDeleteResult::Deleted),
            StatusCode::NOT_FOUND => Ok(SyncDeleteResult::NotFound),
            StatusCode::PRECONDITION_FAILED => Ok(SyncDeleteResult::Conflict {
                current_etag: response_header(&response, ETAG),
            }),
            status => Err(store_error(format!(
                "WebDAV DELETE failed with status {status}."
            ))),
        }
    }
}

impl WebDavObjectStore {
    async fn put(
        &self,
        key: &SyncObjectKey,
        condition: Option<(reqwest::header::HeaderName, &str)>,
        bytes: Vec<u8>,
    ) -> Result<SyncPutResult, SyncError> {
        if bytes.len() > MAX_OBJECT_BYTES {
            return Err(store_error(
                "WebDAV object exceeds the supported size limit.",
            ));
        }
        let url = build_object_url(&self.root_url, key)?;
        let is_conditional_create = condition
            .as_ref()
            .is_some_and(|(name, _)| *name == IF_NONE_MATCH);
        let mut request = self
            .client
            .put(url)
            .basic_auth(&self.config.username, Some(&self.config.password))
            .header("Content-Type", "application/octet-stream")
            .body(bytes.clone());
        if let Some((name, value)) = condition {
            request = request.header(name, value);
        }
        let response = match request.send().await {
            Ok(response) => response,
            Err(error) => {
                return self
                    .recover_ambiguous_put(key, &bytes, is_conditional_create, error)
                    .await;
            }
        };
        self.validate_response_url(response.url())?;
        match response.status() {
            status if status.is_success() => {
                let etag = response_header(&response, ETAG);
                if etag.is_some() {
                    return Ok(SyncPutResult::Created { etag });
                }
                let stored = self
                    .get(key)
                    .await?
                    .ok_or_else(|| store_error("WebDAV object disappeared after upload."))?;
                if stored.bytes != bytes {
                    return Err(store_error(
                        "WebDAV object differs from the bytes accepted by the server.",
                    ));
                }
                Ok(SyncPutResult::Created {
                    etag: stored.metadata.etag,
                })
            }
            StatusCode::PRECONDITION_FAILED => {
                let etag = response_header(&response, ETAG);
                if is_conditional_create {
                    Ok(SyncPutResult::AlreadyExists { etag })
                } else {
                    Ok(SyncPutResult::Conflict { current_etag: etag })
                }
            }
            status => Err(store_error(format!(
                "WebDAV PUT failed with status {status}."
            ))),
        }
    }

    async fn recover_ambiguous_put(
        &self,
        key: &SyncObjectKey,
        expected_bytes: &[u8],
        is_conditional_create: bool,
        put_error: reqwest::Error,
    ) -> Result<SyncPutResult, SyncError> {
        match self.get(key).await {
            Ok(Some(object)) if object.bytes == expected_bytes => {
                if is_conditional_create {
                    Ok(SyncPutResult::AlreadyExists {
                        etag: object.metadata.etag,
                    })
                } else {
                    Ok(SyncPutResult::Created {
                        etag: object.metadata.etag,
                    })
                }
            }
            Ok(Some(_)) => Err(store_error(format!(
                "WebDAV PUT response was lost and the stored object differs: {put_error}"
            ))),
            Ok(None) => Err(store_error(format!(
                "WebDAV PUT failed and the object was not created: {put_error}"
            ))),
            Err(recovery_error) => Err(store_error(format!(
                "WebDAV PUT failed and recovery could not verify the object: {put_error}; {recovery_error}"
            ))),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebDavPropfindObject {
    pub relative_path: String,
    pub is_collection: bool,
    pub etag: Option<String>,
    pub size: u64,
    pub modified_at: Option<String>,
}

pub fn parse_propfind_objects(
    xml: &str,
    root_url: &Url,
) -> Result<Vec<WebDavPropfindObject>, SyncError> {
    let document = Document::parse(xml)
        .map_err(|error| store_error(format!("Failed to parse WebDAV response: {error}")))?;
    let mut entries = Vec::new();
    for response in document
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "response")
    {
        let Some(href) = descendant_text(response, "href") else {
            continue;
        };
        let resolved = root_url
            .join(&href)
            .map_err(|error| store_error(format!("Failed to resolve WebDAV href: {error}")))?;
        ensure_same_origin(&resolved, root_url)?;
        let relative_path = relative_path(root_url, &resolved)?;
        if relative_path.is_empty() {
            continue;
        }
        entries.push(WebDavPropfindObject {
            relative_path,
            is_collection: response.descendants().any(|candidate| {
                candidate.is_element() && candidate.tag_name().name() == "collection"
            }),
            etag: descendant_text(response, "getetag"),
            size: descendant_text(response, "getcontentlength")
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(0),
            modified_at: descendant_text(response, "getlastmodified"),
        });
    }
    Ok(entries)
}

pub fn build_collection_url(base_url: &str, remote_root: &str) -> Result<Url, SyncError> {
    let mut url = parse_server_url(base_url)?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| store_error("WebDAV server URL cannot be used as a directory base."))?;
        segments.pop_if_empty();
        for segment in remote_root
            .split('/')
            .map(str::trim)
            .filter(|segment| !segment.is_empty())
        {
            if segment == "." || segment == ".." || segment.contains('\\') {
                return Err(store_error("WebDAV remote root is invalid."));
            }
            segments.push(segment);
        }
        segments.push("");
    }
    enforce_https(&url, "WebDAV collection URL")?;
    Ok(url)
}

pub fn build_object_url(root_url: &Url, key: &SyncObjectKey) -> Result<Url, SyncError> {
    let mut url = root_url.clone();
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| store_error("WebDAV root URL cannot be used as an object base."))?;
        segments.pop_if_empty();
        for segment in key.as_str().split('/') {
            segments.push(segment);
        }
    }
    enforce_https(&url, "WebDAV object URL")?;
    Ok(url)
}

fn parse_server_url(value: &str) -> Result<Url, SyncError> {
    let value = required(value, "WebDAV server URL")?;
    let mut url = Url::parse(&value)
        .map_err(|error| store_error(format!("WebDAV server URL is invalid: {error}")))?;
    enforce_https(&url, "WebDAV server URL")?;
    ensure_trailing_slash(&mut url);
    Ok(url)
}

fn relative_path(root_url: &Url, resolved: &Url) -> Result<String, SyncError> {
    let root_path = root_url.path();
    let resolved_path = resolved.path();
    if resolved_path == root_path.trim_end_matches('/') {
        return Ok(String::new());
    }
    let Some(relative) = resolved_path.strip_prefix(root_path) else {
        return Err(store_error(
            "WebDAV response entry is outside the configured root.",
        ));
    };
    let mut decoded = Vec::new();
    for segment in relative.split('/').filter(|segment| !segment.is_empty()) {
        let value = urlencoding::decode(segment)
            .map_err(|_| store_error("WebDAV response path is not valid UTF-8."))?
            .into_owned();
        if value.is_empty()
            || value == "."
            || value == ".."
            || value.contains('/')
            || value.contains('\\')
        {
            return Err(store_error("WebDAV response path is unsafe."));
        }
        decoded.push(value);
    }
    Ok(decoded.join("/"))
}

fn path_segments(url: &Url) -> Result<Vec<String>, SyncError> {
    url.path_segments()
        .ok_or_else(|| store_error("WebDAV URL cannot be used as a directory base."))?
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            urlencoding::decode(segment)
                .map(|value| value.into_owned())
                .map_err(|_| store_error("WebDAV URL path is not valid UTF-8."))
        })
        .collect()
}

fn append_segment(url: &mut Url, segment: &str, collection: bool) -> Result<(), SyncError> {
    let mut segments = url
        .path_segments_mut()
        .map_err(|_| store_error("WebDAV URL cannot be used as a directory base."))?;
    segments.pop_if_empty();
    segments.push(segment);
    if collection {
        segments.push("");
    }
    Ok(())
}

fn ensure_trailing_slash(url: &mut Url) {
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path());
        url.set_path(&path);
    }
}

fn ensure_url_within_root_or_server(
    candidate: &Url,
    server_url: &Url,
    root_url: &Url,
) -> Result<(), SyncError> {
    ensure_same_origin(candidate, server_url)?;
    if candidate.path().starts_with(server_url.path())
        && (root_url.path().starts_with(candidate.path())
            || candidate.path().starts_with(root_url.path()))
    {
        Ok(())
    } else {
        Err(store_error(
            "WebDAV collection is outside the configured remote root.",
        ))
    }
}

fn webdav_redirect_policy(server_url: Url, root_url: Url) -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(move |attempt| {
        if attempt.previous().len() >= 10 {
            return attempt.error("WebDAV redirect limit exceeded.");
        }
        let candidate = attempt.url();
        if enforce_https(candidate, "WebDAV redirect URL").is_err()
            || ensure_url_within_root_or_server(candidate, &server_url, &root_url).is_err()
        {
            attempt.error("WebDAV redirect target is outside the configured HTTPS root.")
        } else {
            attempt.follow()
        }
    })
}

fn ensure_same_origin(candidate: &Url, expected: &Url) -> Result<(), SyncError> {
    if candidate.scheme() == expected.scheme()
        && candidate.host_str() == expected.host_str()
        && candidate.port_or_known_default() == expected.port_or_known_default()
    {
        Ok(())
    } else {
        Err(store_error("WebDAV response changed origin."))
    }
}

fn enforce_https(url: &Url, label: &str) -> Result<(), SyncError> {
    if url.scheme() == "https" {
        Ok(())
    } else {
        Err(store_error(format!("{label} must start with https://.")))
    }
}

fn descendant_text(node: Node<'_, '_>, name: &str) -> Option<String> {
    node.descendants()
        .find(|candidate| candidate.is_element() && candidate.tag_name().name() == name)
        .and_then(|candidate| candidate.text())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn response_header(
    response: &reqwest::Response,
    name: reqwest::header::HeaderName,
) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned)
}

fn required(value: &str, label: &str) -> Result<String, SyncError> {
    let value = value.trim();
    if value.is_empty() {
        Err(store_error(format!("{label} is required.")))
    } else {
        Ok(value.to_string())
    }
}

fn webdav_method(value: &[u8]) -> Result<Method, SyncError> {
    Method::from_bytes(value)
        .map_err(|error| store_error(format!("Invalid WebDAV method: {error}")))
}

fn store_error(message: impl Into<String>) -> SyncError {
    SyncError::ObjectStore(message.into())
}

#[cfg(test)]
mod contract_tests;
