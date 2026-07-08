use sona_webdav::{
    WebDavConnectionStatus, build_collection_url, checked_webdav_request_url,
    parse_propfind_entries, parse_server_url, resolve_warning_message,
};

#[test]
fn parse_server_url_rejects_http_before_credentials_are_sent() {
    let error = parse_server_url("http://nas.local/dav").unwrap_err();

    assert_eq!(error, "WebDAV server URL must start with https://.");
}

#[test]
fn checked_webdav_request_url_rejects_http_before_credentials_are_sent() {
    let error = checked_webdav_request_url(
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
    let result = resolve_warning_message(false);

    assert_eq!(result.status, WebDavConnectionStatus::Success);
    assert_eq!(result.message, "WebDAV connection is ready.");
}
