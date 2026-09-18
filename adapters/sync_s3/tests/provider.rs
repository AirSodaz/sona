use std::collections::HashMap;
use std::sync::Arc;

use axum::Router;
use axum::extract::Request;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::any;
use serde_json::json;
use sona_core::sync::{
    SyncDeleteResult, SyncObjectKey, SyncObjectPrefix, SyncObjectStore, SyncPutResult,
};
use sona_sync::{SyncProviderFactory, SyncProviderInput, SyncProviderRegistry};
use sona_sync_s3::{S3ObjectStore, S3ObjectStoreConfig, S3SyncProviderFactory};
use tokio::sync::Mutex;

fn full_s3_configuration(endpoint: &str) -> serde_json::Value {
    json!({
        "endpoint": endpoint,
        "region": "us-east-1",
        "bucket": "test-bucket",
        "remoteRoot": "sona/sync",
        "accessKeyId": "AKIAIOSFODNN7EXAMPLE",
        "secretAccessKey": "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
        "forcePathStyle": true
    })
}

#[tokio::test]
async fn registry_prepares_and_restores_s3_provider() {
    let registry =
        SyncProviderRegistry::new(
            [Arc::new(S3SyncProviderFactory) as Arc<dyn SyncProviderFactory>],
        );

    let provider = registry
        .prepare(SyncProviderInput {
            provider_id: "s3".to_string(),
            configuration: full_s3_configuration("https://s3.amazonaws.com"),
        })
        .await
        .unwrap();

    assert_eq!(provider.descriptor.id, "s3");
    assert_eq!(provider.descriptor.display_name, "S3-Compatible Storage");
    assert_eq!(
        provider.persisted_configuration,
        json!({
            "endpoint": "https://s3.amazonaws.com",
            "region": "us-east-1",
            "bucket": "test-bucket",
            "remoteRoot": "sona/sync",
            "accessKeyId": "AKIAIOSFODNN7EXAMPLE",
            "forcePathStyle": true
        })
    );
    assert_eq!(
        provider.credential,
        b"wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
    );
    assert_eq!(
        registry.credential_secret_key("s3", "vault-123").unwrap(),
        "s3-secret-key:vault-123"
    );

    // Test restore
    let restored = registry
        .restore(
            "s3",
            provider.persisted_configuration.clone(),
            b"restored-secret-key".to_vec(),
        )
        .await
        .unwrap();
    assert_eq!(restored.descriptor.id, "s3");
    assert_eq!(restored.credential, b"restored-secret-key");
}

#[tokio::test]
async fn factory_rejects_missing_required_fields() {
    let factory = S3SyncProviderFactory;

    let malformed = factory
        .prepare(json!({
            "endpoint": "https://s3.amazonaws.com",
            "region": "us-east-1",
            "bucket": "test-bucket"
            // Missing accessKeyId and secretAccessKey
        }))
        .await
        .err()
        .expect("missing keys must fail");

    assert!(
        malformed
            .to_string()
            .contains("S3 provider configuration is invalid")
    );
}

type StoredObject = (Vec<u8>, String);
type ObjectMap = HashMap<String, StoredObject>;

#[derive(Default, Clone)]
struct MockS3State {
    objects: Arc<Mutex<ObjectMap>>,
}

#[tokio::test]
async fn s3_object_store_end_to_end_against_mock_server() {
    let state = MockS3State::default();
    let state_clone = state.clone();

    // Spawn an in-memory mock S3 server
    let app = Router::new().fallback(any(move |req: Request| {
        let state = state_clone.clone();
        async move {
            let method = req.method().clone();
            let uri = req.uri().clone();
            let headers = req.headers().clone();
            let path = uri.path().to_string();

            // Strip bucket prefix for path-style: /test-bucket/...
            let key = path.trim_start_matches('/').strip_prefix("test-bucket/").unwrap_or(&path).to_string();

            if method == axum::http::Method::GET {
                if uri.query().map(|q| q.contains("list-type=2")).unwrap_or(false) {
                    // ListObjectsV2
                    let map = state.objects.lock().await;
                    let mut xml = String::from(r#"<?xml version="1.0" encoding="UTF-8"?><ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/"><Name>test-bucket</Name><IsTruncated>false</IsTruncated>"#);
                    for (k, (data, etag)) in map.iter() {
                        xml.push_str(&format!(
                            "<Contents><Key>{k}</Key><LastModified>2026-09-18T12:00:00.000Z</LastModified><ETag>\"{etag}\"</ETag><Size>{}</Size></Contents>",
                            data.len()
                        ));
                    }
                    xml.push_str("</ListBucketResult>");
                    return (
                        StatusCode::OK,
                        [("content-type", "application/xml")],
                        xml,
                    ).into_response();
                }

                // GetObject
                let map = state.objects.lock().await;
                if let Some((data, etag)) = map.get(&key) {
                    let mut resp_headers = HeaderMap::new();
                    resp_headers.insert("etag", format!("\"{etag}\"").parse().unwrap());
                    resp_headers.insert("last-modified", "2026-09-18T12:00:00.000Z".parse().unwrap());
                    return (StatusCode::OK, resp_headers, data.clone()).into_response();
                } else {
                    return (StatusCode::NOT_FOUND, "NoSuchKey").into_response();
                }
            }

            if method == axum::http::Method::PUT {
                let if_none_match = headers.get("if-none-match").and_then(|h| h.to_str().ok());
                let if_match = headers.get("if-match").and_then(|h| h.to_str().ok());
                let body_bytes = axum::body::to_bytes(req.into_body(), usize::MAX).await.unwrap().to_vec();

                let mut map = state.objects.lock().await;
                if if_none_match == Some("*") && map.contains_key(&key) {
                    return (StatusCode::PRECONDITION_FAILED, "PreconditionFailed").into_response();
                }

                if let Some(im) = if_match {
                    let expected = im.trim_matches('"');
                    if let Some((_, current_etag)) = map.get(&key) {
                        if current_etag != expected {
                            return (StatusCode::PRECONDITION_FAILED, "PreconditionFailed").into_response();
                        }
                    } else {
                        return (StatusCode::PRECONDITION_FAILED, "PreconditionFailed").into_response();
                    }
                }

                let etag = format!("{:x}", md5_or_simple(&body_bytes));
                map.insert(key, (body_bytes, etag.clone()));

                let mut resp_headers = HeaderMap::new();
                resp_headers.insert("etag", format!("\"{etag}\"").parse().unwrap());
                return (StatusCode::OK, resp_headers, "").into_response();
            }

            if method == axum::http::Method::DELETE {
                let if_match = headers.get("if-match").and_then(|h| h.to_str().ok());
                let mut map = state.objects.lock().await;

                if let Some(im) = if_match {
                    let expected = im.trim_matches('"');
                    if let Some((_, current_etag)) = map.get(&key) {
                        if current_etag != expected {
                            return (StatusCode::PRECONDITION_FAILED, "PreconditionFailed").into_response();
                        }
                    } else {
                        return (StatusCode::NOT_FOUND, "NoSuchKey").into_response();
                    }
                }

                if map.remove(&key).is_some() {
                    return StatusCode::NO_CONTENT.into_response();
                } else {
                    return StatusCode::NOT_FOUND.into_response();
                }
            }

            StatusCode::BAD_REQUEST.into_response()
        }
    }));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let endpoint = format!("http://127.0.0.1:{port}");
    let config = S3ObjectStoreConfig {
        endpoint,
        region: "us-east-1".to_string(),
        bucket: "test-bucket".to_string(),
        remote_root: "sona".to_string(),
        access_key_id: "AKIAIOSFODNN7EXAMPLE".to_string(),
        secret_access_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
        session_token: None,
        force_path_style: true,
    };

    let store = S3ObjectStore::new(config).unwrap();

    // 1. Probe
    let caps = store
        .probe()
        .await
        .expect("probe must succeed against mock server");
    assert!(caps.conditional_create);
    assert!(caps.compare_and_swap);
    assert!(caps.delete);

    // 2. Put if absent
    let key = SyncObjectKey::parse("vaults/v1/test.sync").unwrap();
    let put_res = store
        .put_if_absent(&key, b"hello s3".to_vec())
        .await
        .unwrap();
    let etag = match put_res {
        SyncPutResult::Created { etag } => etag.expect("etag present"),
        _ => panic!("expected created"),
    };

    // Second put_if_absent on same key must conflict
    let conflict = store
        .put_if_absent(&key, b"overwrite attempt".to_vec())
        .await
        .unwrap();
    match conflict {
        SyncPutResult::Conflict { .. } | SyncPutResult::AlreadyExists { .. } => {}
        _ => panic!("expected conflict/already exists"),
    }

    // 3. Get
    let obj = store.get(&key).await.unwrap().expect("object must exist");
    assert_eq!(obj.bytes, b"hello s3");
    assert_eq!(obj.metadata.etag.as_deref(), Some(etag.as_str()));

    // 4. Compare and swap with valid etag
    let cas_res = store
        .compare_and_swap(&key, Some(&etag), b"updated content".to_vec())
        .await
        .unwrap();
    let new_etag = match cas_res {
        SyncPutResult::Created { etag } => etag.expect("new etag present"),
        _ => panic!("expected cas created"),
    };
    assert_ne!(etag, new_etag);

    // CAS with stale etag must fail
    let cas_stale = store
        .compare_and_swap(&key, Some(&etag), b"stale update".to_vec())
        .await
        .unwrap();
    assert!(matches!(cas_stale, SyncPutResult::Conflict { .. }));

    // 5. List
    let page = store
        .list(&SyncObjectPrefix::parse("vaults/v1").unwrap(), None)
        .await
        .unwrap();
    assert_eq!(page.objects.len(), 1);
    assert_eq!(page.objects[0].key, key);

    // 6. Delete
    let del_res = store.delete(&key, Some(&new_etag)).await.unwrap();
    assert_eq!(del_res, SyncDeleteResult::Deleted);

    // Get after delete
    let after_del = store.get(&key).await.unwrap();
    assert!(after_del.is_none());
}

fn md5_or_simple(bytes: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}
