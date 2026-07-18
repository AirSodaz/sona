use std::sync::Arc;

use serde_json::json;
use sona_core::sync::SyncError;
use sona_sync::{SyncProviderFactory, SyncProviderInput, SyncProviderRegistry};
use sona_sync_webdav::WebDavSyncProviderFactory;

fn full_configuration() -> serde_json::Value {
    json!({
        "serverUrl": "https://dav.example.com/remote.php/dav/files/alice",
        "remoteRoot": "sona/sync",
        "username": "alice",
        "password": "correct horse battery staple"
    })
}

#[tokio::test]
async fn registry_prepares_webdav_without_persisting_its_password() {
    let registry = SyncProviderRegistry::new([
        Arc::new(WebDavSyncProviderFactory) as Arc<dyn SyncProviderFactory>
    ]);

    let provider = registry
        .prepare(SyncProviderInput {
            provider_id: "webdav".to_string(),
            configuration: full_configuration(),
        })
        .await
        .unwrap();

    assert_eq!(provider.descriptor.id, "webdav");
    assert_eq!(provider.descriptor.display_name, "WebDAV");
    assert_eq!(
        provider.persisted_configuration,
        json!({
            "serverUrl": "https://dav.example.com/remote.php/dav/files/alice/",
            "remoteRoot": "sona/sync",
            "username": "alice"
        })
    );
    assert_eq!(provider.credential, b"correct horse battery staple");
    assert_eq!(
        registry
            .credential_secret_key("webdav", "vault-123")
            .unwrap(),
        "webdav-password:vault-123"
    );
}

#[tokio::test]
async fn registry_restores_webdav_from_public_settings_and_secret_bytes() {
    let registry = SyncProviderRegistry::new([
        Arc::new(WebDavSyncProviderFactory) as Arc<dyn SyncProviderFactory>
    ]);
    let persisted = json!({
        "serverUrl": "https://dav.example.com/dav/",
        "remoteRoot": "sona",
        "username": "alice"
    });

    let provider = registry
        .restore("webdav", persisted.clone(), b"restored-password".to_vec())
        .await
        .unwrap();

    assert_eq!(provider.descriptor.id, "webdav");
    assert_eq!(provider.persisted_configuration, persisted);
    assert_eq!(provider.credential, b"restored-password");
}

#[tokio::test]
async fn factory_rejects_malformed_configuration_and_credentials() {
    let factory = WebDavSyncProviderFactory;

    let malformed = factory
        .prepare(json!({
            "serverUrl": "https://dav.example.com/dav/",
            "remoteRoot": "sona",
            "username": "alice"
        }))
        .await
        .err()
        .expect("configuration without a password must fail");
    assert!(matches!(malformed, SyncError::ObjectStore(_)));
    assert!(malformed.to_string().contains("password"));

    let malformed = factory
        .restore(
            json!({
                "serverUrl": "https://dav.example.com/dav/",
                "remoteRoot": 42,
                "username": "alice"
            }),
            b"password".to_vec(),
        )
        .await
        .err()
        .expect("configuration with a non-string remote root must fail");
    assert!(matches!(malformed, SyncError::ObjectStore(_)));
    assert!(malformed.to_string().contains("configuration"));

    let malformed = factory
        .restore(
            json!({
                "serverUrl": "https://dav.example.com/dav/",
                "remoteRoot": "sona",
                "username": "alice"
            }),
            vec![0xff],
        )
        .await
        .err()
        .expect("non-UTF-8 credentials must fail");
    assert!(matches!(malformed, SyncError::ObjectStore(_)));
    assert!(malformed.to_string().contains("UTF-8"));
}

#[tokio::test]
async fn prepared_provider_exposes_the_store_probe_used_by_sync_application() {
    let provider = WebDavSyncProviderFactory
        .prepare(json!({
            "serverUrl": "https://127.0.0.1:0/dav/",
            "remoteRoot": "sona",
            "username": "alice",
            "password": "password"
        }))
        .await
        .unwrap();

    let error = provider.store.probe().await.unwrap_err();

    assert!(matches!(error, SyncError::ObjectStore(_)));
    assert!(error.to_string().contains("WebDAV PROPFIND failed"));
}
