use roxmltree::Document;
use sona_core::sync::{SyncError, SyncObjectKey, SyncObjectMetadata};

#[derive(Debug, PartialEq)]
pub struct S3ListObjectsOutput {
    pub objects: Vec<SyncObjectMetadata>,
    pub is_truncated: bool,
    pub next_continuation_token: Option<String>,
}

#[derive(Debug, PartialEq)]
pub struct S3ErrorResponse {
    pub code: String,
    pub message: String,
}

pub fn parse_error_response(xml: &str) -> Result<S3ErrorResponse, SyncError> {
    let doc = Document::parse(xml)
        .map_err(|e| SyncError::ObjectStore(format!("Malformed S3 XML: {e}")))?;
    let root = doc.root_element();
    if root.tag_name().name() != "Error" {
        return Err(SyncError::ObjectStore(
            "Expected S3 <Error> root element".to_string(),
        ));
    }
    let code = root
        .children()
        .find(|n| n.tag_name().name() == "Code")
        .and_then(|n| n.text())
        .unwrap_or("UnknownError")
        .to_string();
    let message = root
        .children()
        .find(|n| n.tag_name().name() == "Message")
        .and_then(|n| n.text())
        .unwrap_or("")
        .to_string();
    Ok(S3ErrorResponse { code, message })
}

pub fn parse_list_objects_v2_response(
    xml: &str,
    remote_root: &str,
) -> Result<S3ListObjectsOutput, SyncError> {
    let doc = Document::parse(xml)
        .map_err(|e| SyncError::ObjectStore(format!("Malformed S3 XML: {e}")))?;
    let root = doc.root_element();
    if root.tag_name().name() != "ListBucketResult" {
        return Err(SyncError::ObjectStore(
            "Expected S3 <ListBucketResult> root element".to_string(),
        ));
    }

    let is_truncated = root
        .children()
        .find(|n| n.tag_name().name() == "IsTruncated")
        .and_then(|n| n.text())
        .map(|s| s.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let next_continuation_token = root
        .children()
        .find(|n| n.tag_name().name() == "NextContinuationToken")
        .and_then(|n| n.text())
        .map(|s| s.to_string());

    let normalized_root = remote_root.trim().trim_matches('/');
    let mut objects = Vec::new();

    for contents in root
        .children()
        .filter(|n| n.tag_name().name() == "Contents")
    {
        let raw_key = contents
            .children()
            .find(|n| n.tag_name().name() == "Key")
            .and_then(|n| n.text())
            .unwrap_or("");

        // Strip remote_root prefix from the key to restore relative SyncObjectKey
        let relative_key_str = if normalized_root.is_empty() {
            raw_key.trim_start_matches('/')
        } else {
            let prefix = format!("{normalized_root}/");
            if let Some(stripped) = raw_key.strip_prefix(&prefix) {
                stripped
            } else if raw_key == normalized_root {
                continue; // Skip collection marker
            } else {
                continue; // Outside remote_root
            }
        };

        if relative_key_str.is_empty() || relative_key_str.ends_with('/') {
            continue; // Skip directory/folder markers
        }

        let key = match SyncObjectKey::parse(relative_key_str) {
            Ok(k) => k,
            Err(_) => continue, // Ignore invalid keys gracefully
        };

        let etag = contents
            .children()
            .find(|n| n.tag_name().name() == "ETag")
            .and_then(|n| n.text())
            .map(|s| s.trim_matches('"').to_string());

        let size = contents
            .children()
            .find(|n| n.tag_name().name() == "Size")
            .and_then(|n| n.text())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        let modified_at = contents
            .children()
            .find(|n| n.tag_name().name() == "LastModified")
            .and_then(|n| n.text())
            .map(|s| s.to_string());

        objects.push(SyncObjectMetadata {
            key,
            etag,
            size,
            modified_at,
        });
    }

    Ok(S3ListObjectsOutput {
        objects,
        is_truncated,
        next_continuation_token,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_list_objects_v2() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
    <Name>mybucket</Name>
    <Prefix>sona/</Prefix>
    <KeyCount>2</KeyCount>
    <MaxKeys>1000</MaxKeys>
    <IsTruncated>false</IsTruncated>
    <Contents>
        <Key>sona/vaults/v1/vault.json</Key>
        <LastModified>2026-09-18T12:00:00.000Z</LastModified>
        <ETag>&quot;3e25960a79dbc69b674cd4ec67a72c62&quot;</ETag>
        <Size>1234</Size>
    </Contents>
    <Contents>
        <Key>sona/</Key>
        <LastModified>2026-09-18T11:00:00.000Z</LastModified>
        <ETag>&quot;d41d8cd98f00b204e9800998ecf8427e&quot;</ETag>
        <Size>0</Size>
    </Contents>
</ListBucketResult>"#;

        let output = parse_list_objects_v2_response(xml, "sona").unwrap();
        assert!(!output.is_truncated);
        assert_eq!(output.objects.len(), 1);
        assert_eq!(output.objects[0].key.as_str(), "vaults/v1/vault.json");
        assert_eq!(
            output.objects[0].etag.as_deref(),
            Some("3e25960a79dbc69b674cd4ec67a72c62")
        );
        assert_eq!(output.objects[0].size, 1234);
    }

    #[test]
    fn test_parse_error() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Error>
    <Code>NoSuchBucket</Code>
    <Message>The specified bucket does not exist</Message>
    <BucketName>missing-bucket</BucketName>
    <RequestId>tx0000000000000000</RequestId>
</Error>"#;

        let err = parse_error_response(xml).unwrap();
        assert_eq!(err.code, "NoSuchBucket");
        assert_eq!(err.message, "The specified bucket does not exist");
    }
}
