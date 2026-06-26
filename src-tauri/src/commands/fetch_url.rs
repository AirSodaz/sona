use std::time::Duration;
use tauri::command;
use futures_util::StreamExt;

const ALLOWED_HOSTS: &[&str] = &[
  "github.com",
  "objects.githubusercontent.com",
];

const MAX_RESPONSE_SIZE: usize = 1_048_576; // 1 MB
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Fetch a URL and return its body as a string.
/// Used by the frontend to fetch updater JSON without webview CSP restrictions.
/// Restricted to GitHub-hosted URLs for security.
#[command]
pub async fn fetch_url(url: String) -> Result<String, String> {
  let parsed = reqwest::Url::parse(&url).map_err(|e| format!("Invalid URL: {e}"))?;

  let host = parsed.host_str().ok_or("URL has no host")?;
  if !ALLOWED_HOSTS.iter().any(|allowed| host == *allowed || host.ends_with(&format!(".{allowed}"))) {
    return Err(format!("Host '{host}' is not allowed"));
  }

  if parsed.scheme() != "https" {
    return Err("Only HTTPS URLs are allowed".to_string());
  }

  let client = reqwest::Client::builder()
    .timeout(REQUEST_TIMEOUT)
    .build()
    .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

  let response = client
    .get(&url)
    .send()
    .await
    .map_err(|e| format!("HTTP request failed: {e}"))?;

  if let Some(content_length) = response.content_length() {
    if content_length > MAX_RESPONSE_SIZE as u64 {
      return Err("Response too large".to_string());
    }
  }

  let mut body_bytes = Vec::new();
  let mut stream = response.bytes_stream();
  while let Some(chunk_result) = stream.next().await {
    let chunk = chunk_result.map_err(|e| format!("Failed to read response chunk: {e}"))?;
    if body_bytes.len() + chunk.len() > MAX_RESPONSE_SIZE {
      return Err("Response too large".to_string());
    }
    body_bytes.extend_from_slice(&chunk);
  }

  let body = String::from_utf8(body_bytes)
    .map_err(|e| format!("Response body is not valid UTF-8: {e}"))?;

  Ok(body)
}
