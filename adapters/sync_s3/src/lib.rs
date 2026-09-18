pub mod sigv4;
pub mod xml;

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use reqwest::header::{ETAG, HeaderMap, HeaderName, HeaderValue};
use reqwest::{Client, Method, Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sona_core::sync::{
    SyncDeleteResult, SyncError, SyncListPage, SyncObject, SyncObjectKey, SyncObjectMetadata,
    SyncObjectPrefix, SyncObjectStore, SyncObjectStoreCapabilities, SyncProviderDescriptor,
    SyncPutResult,
};
use sona_sync::{SyncProvider, SyncProviderFactory};
use url::Url;

use self::sigv4::{SigV4Credentials, SigV4Signer, sha256_hex};
use self::xml::{parse_error_response, parse_list_objects_v2_response};

const MAX_OBJECT_BYTES: usize = 72 * 1024 * 1024; // 72MB limit, same as WebDAV
const PROBE_PREFIX: &str = ".sona-sync-probe";

#[derive(Clone, Copy, Debug, Default)]
pub struct S3SyncProviderFactory;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersistedS3Config {
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    #[serde(default)]
    pub remote_root: String,
    pub access_key_id: String,
    #[serde(default)]
    pub force_path_style: bool,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct S3ObjectStoreConfig {
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    #[serde(default)]
    pub remote_root: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_token: Option<String>,
    #[serde(default)]
    pub force_path_style: bool,
}

impl std::fmt::Debug for S3ObjectStoreConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("S3ObjectStoreConfig")
            .field("endpoint", &self.endpoint)
            .field("region", &self.region)
            .field("bucket", &self.bucket)
            .field("remote_root", &self.remote_root)
            .field("access_key_id", &self.access_key_id)
            .field("secret_access_key", &"<redacted>")
            .field("force_path_style", &self.force_path_style)
            .finish()
    }
}

impl S3ObjectStoreConfig {
    pub fn validate(&self) -> Result<(), SyncError> {
        if self.endpoint.trim().is_empty() {
            return Err(store_error("S3 endpoint is required."));
        }
        if self.region.trim().is_empty() {
            return Err(store_error("S3 region is required."));
        }
        if self.bucket.trim().is_empty() {
            return Err(store_error("S3 bucket name is required."));
        }
        if self.access_key_id.trim().is_empty() {
            return Err(store_error("S3 accessKeyId is required."));
        }
        if self.secret_access_key.trim().is_empty() {
            return Err(store_error("S3 secretAccessKey is required."));
        }
        Url::parse(&self.endpoint)
            .map_err(|e| store_error(format!("Invalid S3 endpoint URL: {e}")))?;
        Ok(())
    }
}

#[async_trait]
impl SyncProviderFactory for S3SyncProviderFactory {
    fn provider_id(&self) -> &str {
        "s3"
    }

    fn credential_secret_key(&self, vault_id: &str) -> String {
        format!("s3-secret-key:{vault_id}")
    }

    async fn prepare(&self, configuration: Value) -> Result<SyncProvider, SyncError> {
        let config: S3ObjectStoreConfig =
            serde_json::from_value(configuration).map_err(provider_configuration_error)?;
        build_provider(config)
    }

    async fn restore(
        &self,
        persisted_configuration: Value,
        credential: Vec<u8>,
    ) -> Result<SyncProvider, SyncError> {
        let persisted: PersistedS3Config = serde_json::from_value(persisted_configuration)
            .map_err(provider_configuration_error)?;
        let secret_access_key = String::from_utf8(credential)
            .map_err(|_| store_error("S3 provider credential must be valid UTF-8."))?;
        build_provider(S3ObjectStoreConfig {
            endpoint: persisted.endpoint,
            region: persisted.region,
            bucket: persisted.bucket,
            remote_root: persisted.remote_root,
            access_key_id: persisted.access_key_id,
            secret_access_key,
            session_token: None,
            force_path_style: persisted.force_path_style,
        })
    }
}

fn build_provider(config: S3ObjectStoreConfig) -> Result<SyncProvider, SyncError> {
    config.validate()?;
    let persisted_configuration = serde_json::to_value(PersistedS3Config {
        endpoint: config.endpoint.clone(),
        region: config.region.clone(),
        bucket: config.bucket.clone(),
        remote_root: config.remote_root.clone(),
        access_key_id: config.access_key_id.clone(),
        force_path_style: config.force_path_style,
    })
    .map_err(provider_configuration_error)?;

    let credential = config.secret_access_key.as_bytes().to_vec();
    let store = Arc::new(S3ObjectStore::new(config)?);
    Ok(SyncProvider {
        descriptor: SyncProviderDescriptor {
            id: "s3".to_string(),
            display_name: "S3-Compatible Storage".to_string(),
        },
        store,
        persisted_configuration,
        credential,
    })
}

fn provider_configuration_error(error: impl std::fmt::Display) -> SyncError {
    store_error(format!("S3 provider configuration is invalid: {error}"))
}

pub struct S3ObjectStore {
    config: S3ObjectStoreConfig,
    client: Client,
    endpoint_url: Url,
    signer: SigV4Signer,
}

impl S3ObjectStore {
    pub fn new(config: S3ObjectStoreConfig) -> Result<Self, SyncError> {
        config.validate()?;
        let endpoint_url = Url::parse(&config.endpoint)
            .map_err(|e| store_error(format!("Invalid endpoint URL: {e}")))?;
        let client = Client::builder()
            .user_agent("Sona/1.0")
            .build()
            .map_err(|e| store_error(format!("Failed to build HTTP client: {e}")))?;

        let signer = SigV4Signer::new(
            "s3",
            &config.region,
            SigV4Credentials {
                access_key_id: config.access_key_id.clone(),
                secret_access_key: config.secret_access_key.clone(),
                session_token: config.session_token.clone(),
            },
        );

        Ok(Self {
            config,
            client,
            endpoint_url,
            signer,
        })
    }

    /// Resolves the full URL and the URI path used for SigV4 signing.
    fn resolve_object_url_and_path(&self, relative_key: &str) -> Result<(Url, String), SyncError> {
        let normalized_root = self.config.remote_root.trim().trim_matches('/');
        let full_key = if normalized_root.is_empty() {
            relative_key.trim_start_matches('/').to_string()
        } else if relative_key.is_empty() {
            normalized_root.to_string()
        } else {
            format!("{normalized_root}/{}", relative_key.trim_start_matches('/'))
        };

        if self.config.force_path_style {
            // Path style: https://endpoint/bucket/key
            let mut url = self.endpoint_url.clone();
            let encoded_path = if full_key.is_empty() {
                format!("/{}", self.config.bucket)
            } else {
                format!("/{}/{}", self.config.bucket, full_key)
            };
            url.set_path(&encoded_path);
            Ok((url, encoded_path))
        } else {
            // Virtual-hosted style: https://bucket.endpoint/key
            let mut url = self.endpoint_url.clone();
            let host_str = url
                .host_str()
                .ok_or_else(|| store_error("Endpoint missing host"))?;

            // Check if endpoint already includes the bucket prefix
            let new_host = if host_str.starts_with(&format!("{}.", self.config.bucket)) {
                host_str.to_string()
            } else {
                format!("{}.{}", self.config.bucket, host_str)
            };
            url.set_host(Some(&new_host))
                .map_err(|_| store_error("Failed to set virtual host on URL"))?;

            let encoded_path = if full_key.is_empty() {
                "/".to_string()
            } else {
                format!("/{}", full_key)
            };
            url.set_path(&encoded_path);
            Ok((url, encoded_path))
        }
    }

    fn resolve_host_header(&self, url: &Url) -> String {
        if let Some(port) = url.port() {
            format!("{}:{}", url.host_str().unwrap_or(""), port)
        } else {
            url.host_str().unwrap_or("").to_string()
        }
    }

    async fn send_signed_request(
        &self,
        method: Method,
        url: Url,
        uri_path: &str,
        query_params: &[(String, String)],
        extra_headers: &BTreeMap<String, String>,
        body: Option<Vec<u8>>,
    ) -> Result<Response, SyncError> {
        let payload_bytes = body.as_deref().unwrap_or(b"");
        let payload_hash = sha256_hex(payload_bytes);

        let mut headers_to_sign = extra_headers.clone();
        headers_to_sign.insert("host".to_string(), self.resolve_host_header(&url));

        let signed_headers = self.signer.sign(
            method.as_str(),
            uri_path,
            query_params,
            &headers_to_sign,
            &payload_hash,
            Utc::now(),
        );

        let mut req_headers = HeaderMap::new();
        for (k, v) in signed_headers {
            let header_name = HeaderName::from_bytes(k.as_bytes())
                .map_err(|e| store_error(format!("Invalid header name: {e}")))?;
            let header_val = HeaderValue::from_str(&v)
                .map_err(|e| store_error(format!("Invalid header value: {e}")))?;
            req_headers.insert(header_name, header_val);
        }

        let mut builder = self
            .client
            .request(method, url.clone())
            .headers(req_headers);
        if let Some(bytes) = body {
            builder = builder.body(bytes);
        }

        builder
            .send()
            .await
            .map_err(|e| store_error(format!("S3 HTTP request failed: {e}")))
    }
}

#[async_trait]
impl SyncObjectStore for S3ObjectStore {
    async fn probe(&self) -> Result<SyncObjectStoreCapabilities, SyncError> {
        let probe_key_str = format!("{PROBE_PREFIX}/{}", uuid::Uuid::new_v4());
        let probe_key = SyncObjectKey::parse(probe_key_str)?;

        // 1. Put if absent (Probe Step 1)
        let created = self.put_if_absent(&probe_key, b"probe-a".to_vec()).await?;
        let created_etag = match created {
            SyncPutResult::Created { etag: Some(etag) } => etag,
            SyncPutResult::Created { etag: None } => {
                let _ = self.delete(&probe_key, None).await;
                return Err(store_error(
                    "S3 server must return ETag for uploaded objects.",
                ));
            }
            SyncPutResult::AlreadyExists { .. } | SyncPutResult::Conflict { .. } => {
                return Err(store_error("S3 sync probe object already exists."));
            }
        };

        let result = async {
            // 2. Read back
            let obj = self
                .get(&probe_key)
                .await?
                .ok_or_else(|| store_error("S3 probe object could not be read back."))?;
            if obj.bytes != b"probe-a" {
                return Err(store_error("S3 probe object corrupted after upload."));
            }

            // 3. CAS update with expected ETag
            let updated = self
                .compare_and_swap(&probe_key, Some(&created_etag), b"probe-b".to_vec())
                .await?;
            let updated_etag = match updated {
                SyncPutResult::Created { etag: Some(etag) } => etag,
                _ => {
                    return Err(store_error(
                        "S3 server does not support atomic conditional update (CAS/If-Match).",
                    ));
                }
            };

            // 4. Delete with conditional ETag
            match self.delete(&probe_key, Some(&updated_etag)).await? {
                SyncDeleteResult::Deleted => Ok(SyncObjectStoreCapabilities {
                    conditional_create: true,
                    compare_and_swap: true,
                    delete: true,
                }),
                _ => Err(store_error(
                    "S3 server does not support conditional delete.",
                )),
            }
        }
        .await;

        if result.is_err() {
            let _ = self.delete(&probe_key, None).await;
        }
        result
    }

    async fn list(
        &self,
        prefix: &SyncObjectPrefix,
        continuation: Option<&str>,
    ) -> Result<SyncListPage, SyncError> {
        let (mut url, uri_path) = self.resolve_object_url_and_path("")?;
        let normalized_root = self.config.remote_root.trim().trim_matches('/');

        let s3_prefix = if normalized_root.is_empty() {
            prefix.as_str().to_string()
        } else if prefix.as_str().is_empty() {
            format!("{normalized_root}/")
        } else {
            format!("{normalized_root}/{}", prefix.as_str())
        };

        let mut query_params = vec![
            ("list-type".to_string(), "2".to_string()),
            ("prefix".to_string(), s3_prefix),
            ("max-keys".to_string(), "1000".to_string()),
        ];
        if let Some(token) = continuation {
            query_params.push(("continuation-token".to_string(), token.to_string()));
        }

        // Attach query params to URL
        {
            let mut pairs = url.query_pairs_mut();
            for (k, v) in &query_params {
                pairs.append_pair(k, v);
            }
        }

        let resp = self
            .send_signed_request(
                Method::GET,
                url,
                &uri_path,
                &query_params,
                &BTreeMap::new(),
                None,
            )
            .await?;

        let status = resp.status();
        let body_text = resp
            .text()
            .await
            .map_err(|e| store_error(format!("Failed to read S3 list response: {e}")))?;

        if !status.is_success() {
            if let Ok(err) = parse_error_response(&body_text) {
                return Err(store_error(format!(
                    "S3 ListObjects failed: {} - {}",
                    err.code, err.message
                )));
            }
            return Err(store_error(format!(
                "S3 ListObjects failed with status {status}: {body_text}"
            )));
        }

        let output = parse_list_objects_v2_response(&body_text, &self.config.remote_root)?;
        Ok(SyncListPage {
            objects: output.objects,
            continuation: if output.is_truncated {
                output.next_continuation_token
            } else {
                None
            },
        })
    }

    async fn get(&self, key: &SyncObjectKey) -> Result<Option<SyncObject>, SyncError> {
        let (url, uri_path) = self.resolve_object_url_and_path(key.as_str())?;
        let resp = self
            .send_signed_request(Method::GET, url, &uri_path, &[], &BTreeMap::new(), None)
            .await?;

        let status = resp.status();
        if status == StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            if let Ok(err) = parse_error_response(&body_text) {
                if err.code == "NoSuchKey" {
                    return Ok(None);
                }
                return Err(store_error(format!(
                    "S3 GetObject failed: {} - {}",
                    err.code, err.message
                )));
            }
            return Err(store_error(format!(
                "S3 GetObject failed with status {status}"
            )));
        }

        let etag = resp
            .headers()
            .get(ETAG)
            .and_then(|h| h.to_str().ok())
            .map(|s| s.trim_matches('"').to_string());

        let modified_at = resp
            .headers()
            .get("last-modified")
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string());

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| store_error(format!("Failed to read S3 object bytes: {e}")))?
            .to_vec();

        Ok(Some(SyncObject {
            metadata: SyncObjectMetadata {
                key: key.clone(),
                etag,
                size: bytes.len() as u64,
                modified_at,
            },
            bytes,
        }))
    }

    async fn put_if_absent(
        &self,
        key: &SyncObjectKey,
        bytes: Vec<u8>,
    ) -> Result<SyncPutResult, SyncError> {
        let mut extra_headers = BTreeMap::new();
        // Standard S3 conditional header: fail if object already exists
        extra_headers.insert("if-none-match".to_string(), "*".to_string());
        // Aliyun OSS specific header: forbid overwriting existing object
        extra_headers.insert("x-oss-forbid-overwrite".to_string(), "true".to_string());

        self.put_object(key, extra_headers, bytes).await
    }

    async fn compare_and_swap(
        &self,
        key: &SyncObjectKey,
        expected_etag: Option<&str>,
        bytes: Vec<u8>,
    ) -> Result<SyncPutResult, SyncError> {
        let mut extra_headers = BTreeMap::new();
        if let Some(etag) = expected_etag {
            let quoted = if etag.starts_with('"') && etag.ends_with('"') {
                etag.to_string()
            } else {
                format!("\"{etag}\"")
            };
            extra_headers.insert("if-match".to_string(), quoted);
        } else {
            extra_headers.insert("if-none-match".to_string(), "*".to_string());
        }

        self.put_object(key, extra_headers, bytes).await
    }

    async fn delete(
        &self,
        key: &SyncObjectKey,
        expected_etag: Option<&str>,
    ) -> Result<SyncDeleteResult, SyncError> {
        let (url, uri_path) = self.resolve_object_url_and_path(key.as_str())?;
        let mut extra_headers = BTreeMap::new();
        if let Some(etag) = expected_etag {
            let quoted = if etag.starts_with('"') && etag.ends_with('"') {
                etag.to_string()
            } else {
                format!("\"{etag}\"")
            };
            extra_headers.insert("if-match".to_string(), quoted);
        }

        let resp = self
            .send_signed_request(Method::DELETE, url, &uri_path, &[], &extra_headers, None)
            .await?;

        let status = resp.status();
        if status.is_success() || status == StatusCode::NO_CONTENT {
            return Ok(SyncDeleteResult::Deleted);
        }
        if status == StatusCode::NOT_FOUND {
            return Ok(SyncDeleteResult::NotFound);
        }
        if status == StatusCode::PRECONDITION_FAILED {
            let current_etag = resp
                .headers()
                .get(ETAG)
                .and_then(|h| h.to_str().ok())
                .map(|s| s.trim_matches('"').to_string());
            return Ok(SyncDeleteResult::Conflict { current_etag });
        }

        let body_text = resp.text().await.unwrap_or_default();
        if let Ok(err) = parse_error_response(&body_text) {
            if err.code == "NoSuchKey" {
                return Ok(SyncDeleteResult::NotFound);
            }
            if err.code == "PreconditionFailed" {
                return Ok(SyncDeleteResult::Conflict { current_etag: None });
            }
            return Err(store_error(format!(
                "S3 DeleteObject failed: {} - {}",
                err.code, err.message
            )));
        }

        Err(store_error(format!(
            "S3 DeleteObject failed with status {status}: {body_text}"
        )))
    }
}

impl S3ObjectStore {
    async fn put_object(
        &self,
        key: &SyncObjectKey,
        extra_headers: BTreeMap<String, String>,
        bytes: Vec<u8>,
    ) -> Result<SyncPutResult, SyncError> {
        if bytes.len() > MAX_OBJECT_BYTES {
            return Err(store_error(format!(
                "Object size {} exceeds maximum allowed limit {MAX_OBJECT_BYTES}",
                bytes.len()
            )));
        }

        let (url, uri_path) = self.resolve_object_url_and_path(key.as_str())?;
        let resp = self
            .send_signed_request(
                Method::PUT,
                url,
                &uri_path,
                &[],
                &extra_headers,
                Some(bytes),
            )
            .await?;

        let status = resp.status();
        if status.is_success() {
            let etag = resp
                .headers()
                .get(ETAG)
                .and_then(|h| h.to_str().ok())
                .map(|s| s.trim_matches('"').to_string());
            return Ok(SyncPutResult::Created { etag });
        }

        if status == StatusCode::PRECONDITION_FAILED {
            let current_etag = resp
                .headers()
                .get(ETAG)
                .and_then(|h| h.to_str().ok())
                .map(|s| s.trim_matches('"').to_string());
            return Ok(SyncPutResult::Conflict { current_etag });
        }

        if status == StatusCode::CONFLICT {
            return Ok(SyncPutResult::AlreadyExists { etag: None });
        }

        let body_text = resp.text().await.unwrap_or_default();
        if let Ok(err) = parse_error_response(&body_text) {
            if err.code == "PreconditionFailed" {
                return Ok(SyncPutResult::Conflict { current_etag: None });
            }
            if err.code == "FileAlreadyExists" || err.code == "ObjectAlreadyExists" {
                return Ok(SyncPutResult::AlreadyExists { etag: None });
            }
            return Err(store_error(format!(
                "S3 PutObject failed: {} - {}",
                err.code, err.message
            )));
        }

        Err(store_error(format!(
            "S3 PutObject failed with status {status}: {body_text}"
        )))
    }
}

fn store_error(message: impl Into<String>) -> SyncError {
    SyncError::ObjectStore(message.into())
}
