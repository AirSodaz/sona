pub use sona_webdav::{RemoteBackupEntry, WebDavConfigPayload, WebDavConnectionResult};

pub async fn test_connection(
    config: WebDavConfigPayload,
) -> Result<WebDavConnectionResult, String> {
    sona_webdav::webdav_test_connection(config).await
}

pub async fn list_backups(config: WebDavConfigPayload) -> Result<Vec<RemoteBackupEntry>, String> {
    sona_webdav::webdav_list_backups(config).await
}

pub async fn upload_backup(
    config: WebDavConfigPayload,
    local_archive_path: String,
) -> Result<(), String> {
    sona_webdav::webdav_upload_backup(config, local_archive_path).await
}

pub async fn download_backup(
    config: WebDavConfigPayload,
    href: String,
    output_path: String,
) -> Result<(), String> {
    sona_webdav::webdav_download_backup(config, href, output_path).await
}
