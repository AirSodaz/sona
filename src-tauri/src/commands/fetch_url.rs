use tauri::command;

/// Fetch a URL and return its body as a string.
/// Used by the frontend to fetch updater JSON without webview CSP restrictions.
#[command]
pub async fn fetch_url(url: String) -> Result<String, String> {
  reqwest::get(&url)
    .await
    .map_err(|e| format!("HTTP request failed: {e}"))?
    .text()
    .await
    .map_err(|e| format!("Failed to read response body: {e}"))
}
