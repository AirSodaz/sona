use sona_core::sync::SyncObjectKey;
use sona_sync_webdav::{
    WebDavObjectStoreConfig, build_collection_url, build_object_url, parse_propfind_objects,
};

#[test]
fn configuration_rejects_http_before_credentials_are_sent() {
    let error =
        WebDavObjectStoreConfig::new("http://nas.local/dav", "sona", "demo", "secret").unwrap_err();

    assert_eq!(
        error.to_string(),
        "Sync object store error: WebDAV server URL must start with https://."
    );
}

#[test]
fn collection_and_object_urls_encode_each_relative_key_segment() {
    let root = build_collection_url(
        "https://dav.example.com/remote.php/dav/files/demo/",
        "/sync root/sona/",
    )
    .unwrap();
    let key =
        SyncObjectKey::parse("sona-sync/v1/vault-a/devices/device-a/segments/file.sync").unwrap();

    assert_eq!(
        root.as_str(),
        "https://dav.example.com/remote.php/dav/files/demo/sync%20root/sona/"
    );
    assert_eq!(
        build_object_url(&root, &key).unwrap().as_str(),
        "https://dav.example.com/remote.php/dav/files/demo/sync%20root/sona/sona-sync/v1/vault-a/devices/device-a/segments/file.sync"
    );
}

#[test]
fn propfind_parser_returns_generic_objects_and_collections_without_backup_filtering() {
    let xml = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:">
  <d:response>
    <d:href>/remote.php/dav/files/demo/sync/</d:href>
    <d:propstat><d:prop><d:resourcetype><d:collection /></d:resourcetype></d:prop></d:propstat>
  </d:response>
  <d:response>
    <d:href>/remote.php/dav/files/demo/sync/sona-sync/</d:href>
    <d:propstat><d:prop><d:resourcetype><d:collection /></d:resourcetype></d:prop></d:propstat>
  </d:response>
  <d:response>
    <d:href>/remote.php/dav/files/demo/sync/sona-sync/vault.json</d:href>
    <d:propstat><d:prop>
      <d:getetag>&quot;etag-1&quot;</d:getetag>
      <d:getcontentlength>321</d:getcontentlength>
      <d:getlastmodified>Wed, 29 Apr 2026 01:00:00 GMT</d:getlastmodified>
      <d:resourcetype />
    </d:prop></d:propstat>
  </d:response>
  <d:response>
    <d:href>/remote.php/dav/files/demo/sync/legacy.tar.bz2</d:href>
    <d:propstat><d:prop>
      <d:getetag>&quot;etag-2&quot;</d:getetag>
      <d:getcontentlength>99</d:getcontentlength>
      <d:resourcetype />
    </d:prop></d:propstat>
  </d:response>
</d:multistatus>"#;
    let root = url::Url::parse("https://dav.example.com/remote.php/dav/files/demo/sync/").unwrap();

    let entries = parse_propfind_objects(xml, &root).unwrap();

    assert_eq!(entries.len(), 3);
    assert!(entries[0].is_collection);
    assert_eq!(entries[0].relative_path, "sona-sync");
    assert!(!entries[1].is_collection);
    assert_eq!(entries[1].relative_path, "sona-sync/vault.json");
    assert_eq!(entries[1].etag.as_deref(), Some("\"etag-1\""));
    assert_eq!(entries[1].size, 321);
    assert_eq!(entries[2].relative_path, "legacy.tar.bz2");
}

#[test]
fn propfind_parser_rejects_entries_outside_the_configured_root() {
    let xml = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:">
  <d:response>
    <d:href>/remote.php/dav/files/other/private.sync</d:href>
    <d:propstat><d:prop><d:resourcetype /></d:prop></d:propstat>
  </d:response>
</d:multistatus>"#;
    let root = url::Url::parse("https://dav.example.com/remote.php/dav/files/demo/sync/").unwrap();

    let error = parse_propfind_objects(xml, &root).unwrap_err();

    assert!(error.to_string().contains("outside the configured root"));
}
