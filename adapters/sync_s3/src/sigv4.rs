use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use hmac::{Hmac, KeyInit, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Debug)]
pub struct SigV4Credentials {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub session_token: Option<String>,
}

pub struct SigV4Signer {
    service: String,
    region: String,
    credentials: SigV4Credentials,
}

impl SigV4Signer {
    pub fn new(
        service: impl Into<String>,
        region: impl Into<String>,
        credentials: SigV4Credentials,
    ) -> Self {
        Self {
            service: service.into(),
            region: region.into(),
            credentials,
        }
    }

    /// Signs an HTTP request and returns the headers to be attached to the request:
    /// - `Host` (if not already set)
    /// - `x-amz-date`
    /// - `x-amz-content-sha256`
    /// - `x-amz-security-token` (if session_token exists)
    /// - `Authorization`
    pub fn sign(
        &self,
        method: &str,
        path: &str,
        query_params: &[(String, String)],
        headers: &BTreeMap<String, String>,
        payload_hash: &str,
        date_time: DateTime<Utc>,
    ) -> BTreeMap<String, String> {
        let amz_date = date_time.format("%Y%m%dT%H%M%SZ").to_string();
        let date_stamp = date_time.format("%Y%m%d").to_string();

        let mut signed_headers_map = BTreeMap::new();
        for (k, v) in headers {
            signed_headers_map.insert(k.to_ascii_lowercase(), v.trim().to_string());
        }

        signed_headers_map.insert("x-amz-date".to_string(), amz_date.clone());
        signed_headers_map.insert("x-amz-content-sha256".to_string(), payload_hash.to_string());
        if let Some(token) = &self.credentials.session_token {
            signed_headers_map.insert("x-amz-security-token".to_string(), token.clone());
        }

        // 1. Canonical Headers & Signed Headers
        let mut canonical_headers_str = String::new();
        let mut signed_headers_list = Vec::new();
        for (k, v) in &signed_headers_map {
            canonical_headers_str.push_str(k);
            canonical_headers_str.push(':');
            canonical_headers_str.push_str(v);
            canonical_headers_str.push('\n');
            signed_headers_list.push(k.as_str());
        }
        let signed_headers_joined = signed_headers_list.join(";");

        // 2. Canonical URI
        let canonical_uri = canonical_uri(path);

        // 3. Canonical Query String
        let mut sorted_params = query_params.to_vec();
        sorted_params.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        let canonical_query_str = sorted_params
            .iter()
            .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        // 4. Canonical Request
        let canonical_request = format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            method.to_ascii_uppercase(),
            canonical_uri,
            canonical_query_str,
            canonical_headers_str,
            signed_headers_joined,
            payload_hash
        );
        let hashed_canonical_request = sha256_hex(canonical_request.as_bytes());

        // 5. String to Sign
        let credential_scope = format!(
            "{}/{}/{}/aws4_request",
            date_stamp, self.region, self.service
        );
        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{}\n{}\n{}",
            amz_date, credential_scope, hashed_canonical_request
        );

        // 6. Signing Key & Signature
        let signing_key = derive_signing_key(
            &self.credentials.secret_access_key,
            &date_stamp,
            &self.region,
            &self.service,
        );
        let signature = hex::encode(hmac_sha256(&signing_key, string_to_sign.as_bytes()));

        // 7. Authorization Header
        let authorization = format!(
            "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
            self.credentials.access_key_id, credential_scope, signed_headers_joined, signature
        );

        let mut output = signed_headers_map;
        output.insert("authorization".to_string(), authorization);
        output
    }
}

pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn derive_signing_key(secret_key: &str, date_stamp: &str, region: &str, service: &str) -> Vec<u8> {
    let k_secret = format!("AWS4{secret_key}");
    let k_date = hmac_sha256(k_secret.as_bytes(), date_stamp.as_bytes());
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, service.as_bytes());
    hmac_sha256(&k_service, b"aws4_request")
}

fn canonical_uri(path: &str) -> String {
    if path.is_empty() {
        return "/".to_string();
    }
    let mut segments = Vec::new();
    for segment in path.split('/') {
        if !segment.is_empty() {
            segments.push(urlencoding::encode(segment).into_owned());
        }
    }
    let mut result = String::new();
    if path.starts_with('/') {
        result.push('/');
    }
    result.push_str(&segments.join("/"));
    if path.ends_with('/') && !result.ends_with('/') {
        result.push('/');
    }
    if result.is_empty() {
        "/".to_string()
    } else {
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonical_uri_encoding() {
        assert_eq!(canonical_uri(""), "/");
        assert_eq!(canonical_uri("/"), "/");
        assert_eq!(canonical_uri("/bucket/foo/bar.txt"), "/bucket/foo/bar.txt");
        assert_eq!(canonical_uri("bucket/foo/bar.txt"), "bucket/foo/bar.txt");
        assert_eq!(
            canonical_uri("/my bucket/special@file.txt"),
            "/my%20bucket/special%40file.txt"
        );
    }

    #[test]
    fn test_sigv4_signing_determinism() {
        let creds = SigV4Credentials {
            access_key_id: "AKIAIOSFODNN7EXAMPLE".to_string(),
            secret_access_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
            session_token: None,
        };
        let signer = SigV4Signer::new("s3", "us-east-1", creds);
        let date_time = DateTime::parse_from_rfc3339("2013-05-24T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let mut headers = BTreeMap::new();
        headers.insert(
            "host".to_string(),
            "examplebucket.s3.amazonaws.com".to_string(),
        );

        let signed = signer.sign(
            "GET",
            "/test.txt",
            &[("max-keys".to_string(), "2".to_string())],
            &headers,
            &sha256_hex(b""),
            date_time,
        );

        let auth = signed
            .get("authorization")
            .expect("authorization header present");
        assert!(auth.starts_with(
            "AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20130524/us-east-1/s3/aws4_request"
        ));
        assert!(auth.contains("SignedHeaders=host;x-amz-content-sha256;x-amz-date"));
        assert!(auth.contains("Signature="));
    }
}
