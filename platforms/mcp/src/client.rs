use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

pub const WINDOWS_PIPE_NAME: &str = r"\\.\pipe\sona-agent-ipc";

pub fn get_unix_socket_path() -> PathBuf {
    if let Some(runtime_dir) = std::env::var("XDG_RUNTIME_DIR")
        .ok()
        .filter(|d| !d.trim().is_empty())
    {
        return PathBuf::from(runtime_dir).join("sona-agent.sock");
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".local")
        .join("share")
        .join("sona")
        .join("agent.sock")
}

#[derive(Clone, Debug, Default)]
pub struct IpcClient {
    custom_endpoint: Option<String>,
}

impl IpcClient {
    pub fn new() -> Self {
        let custom_endpoint = std::env::var("SONA_IPC_ENDPOINT")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| {
                std::env::var("SONA_IPC_PIPE")
                    .ok()
                    .filter(|s| !s.trim().is_empty())
            });
        Self { custom_endpoint }
    }

    pub fn with_endpoint(endpoint: String) -> Self {
        Self {
            custom_endpoint: Some(endpoint),
        }
    }

    pub async fn is_online(&self) -> bool {
        self.call("sona_get_client_state", serde_json::json!({}))
            .await
            .is_ok()
    }

    pub async fn call(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });
        let req_line = format!(
            "{}\n",
            serde_json::to_string(&req).map_err(|e| e.to_string())?
        );

        #[cfg(windows)]
        {
            let pipe_name = self
                .custom_endpoint
                .as_deref()
                .unwrap_or(WINDOWS_PIPE_NAME)
                .to_string();

            let client = tokio::task::spawn_blocking(move || {
                tokio::net::windows::named_pipe::ClientOptions::new().open(&pipe_name)
            })
            .await
            .map_err(|e| format!("Join error: {e}"))?
            .map_err(|_| "DESKTOP_OFFLINE: Sona desktop client is not running. Call sona_launch_desktop to start it.".to_string())?;

            Self::send_receive(client, req_line).await
        }

        #[cfg(unix)]
        {
            let socket_path = if let Some(endpoint) = &self.custom_endpoint {
                PathBuf::from(endpoint)
            } else {
                get_unix_socket_path()
            };

            let stream = tokio::time::timeout(
                Duration::from_secs(2),
                tokio::net::UnixStream::connect(&socket_path),
            )
            .await
            .map_err(|_| "DESKTOP_OFFLINE: Connection timed out connecting to Sona desktop".to_string())?
            .map_err(|_| "DESKTOP_OFFLINE: Sona desktop client is not running. Call sona_launch_desktop to start it.".to_string())?;

            Self::send_receive(stream, req_line).await
        }

        #[cfg(not(any(windows, unix)))]
        {
            Err("IPC transport is not supported on this operating system".to_string())
        }
    }

    pub async fn send_receive<S>(stream: S, req_line: String) -> Result<serde_json::Value, String>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        let (reader, mut writer) = tokio::io::split(stream);
        let mut buf_reader = tokio::io::BufReader::new(reader);

        writer
            .write_all(req_line.as_bytes())
            .await
            .map_err(|e| format!("Failed to send request to desktop: {e}"))?;
        writer
            .flush()
            .await
            .map_err(|e| format!("Failed to flush request to desktop: {e}"))?;

        let mut resp_line = String::new();
        tokio::time::timeout(
            Duration::from_secs(30),
            buf_reader.read_line(&mut resp_line),
        )
        .await
        .map_err(|_| "Timeout waiting for response from desktop client".to_string())?
        .map_err(|e| format!("Failed to read response from desktop: {e}"))?;

        if resp_line.trim().is_empty() {
            return Err("Empty response received from desktop client".to_string());
        }

        let resp: serde_json::Value = serde_json::from_str(resp_line.trim())
            .map_err(|e| format!("Invalid JSON response from desktop: {e}"))?;

        if let Some(err_obj) = resp.get("error") {
            let msg = err_obj
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error from desktop client");
            return Err(msg.to_string());
        }

        resp.get("result")
            .cloned()
            .ok_or_else(|| "Missing 'result' in response from desktop client".to_string())
    }

    pub async fn launch_desktop(&self, timeout_seconds: u64) -> Result<String, String> {
        if self.is_online().await {
            return Ok("Desktop client is already running".to_string());
        }

        let exe_path = find_desktop_binary().ok_or_else(|| {
            "Could not locate Sona desktop executable. Please ensure Sona is installed or set SONA_DESKTOP_PATH.".to_string()
        })?;

        eprintln!(
            "[sona-mcp] Launching desktop client from: {}",
            exe_path.display()
        );
        let mut cmd = std::process::Command::new(&exe_path);
        cmd.spawn()
            .map_err(|e| format!("Failed to spawn Sona desktop executable: {e}"))?;

        let deadline = std::time::Instant::now() + Duration::from_secs(timeout_seconds.max(1));
        while std::time::Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(300)).await;
            if self.is_online().await {
                return Ok("Desktop client launched successfully and IPC is ready".to_string());
            }
        }

        Err(format!(
            "Timed out after {timeout_seconds}s waiting for Sona desktop client to respond on IPC"
        ))
    }
}

pub fn find_desktop_binary() -> Option<PathBuf> {
    if let Ok(env_path) = std::env::var("SONA_DESKTOP_PATH") {
        let p = PathBuf::from(env_path);
        if p.exists() {
            return Some(p);
        }
    }

    if let Some(parent) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
    {
        for name in &["sona.exe", "Sona.exe", "sona"] {
            let candidate = parent.join(name);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    for rel in &[
        "target/debug/sona.exe",
        "target/release/sona.exe",
        "target/debug/sona",
        "target/release/sona",
    ] {
        let candidate = PathBuf::from(rel);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    #[cfg(windows)]
    {
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let candidate = PathBuf::from(local_app_data)
                .join("Programs")
                .join("Sona")
                .join("Sona.exe");
            if candidate.exists() {
                return Some(candidate);
            }
        }

        if let Ok(program_files) = std::env::var("ProgramFiles") {
            let candidate = PathBuf::from(program_files).join("Sona").join("Sona.exe");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        let candidate = PathBuf::from("/Applications/Sona.app/Contents/MacOS/Sona");
        if candidate.exists() {
            return Some(candidate);
        }
    }

    #[cfg(target_os = "linux")]
    {
        for candidate_str in &["/usr/bin/sona", "/usr/local/bin/sona"] {
            let candidate = PathBuf::from(candidate_str);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            for name in &["sona.exe", "Sona.exe", "sona"] {
                let candidate = dir.join(name);
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_send_receive_duplex() {
        let (client, mut server) = tokio::io::duplex(4096);

        tokio::spawn(async move {
            let mut reader = tokio::io::BufReader::new(&mut server);
            let mut line = String::new();
            reader.read_line(&mut line).await.unwrap();

            let resp = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": { "status": "ok" }
            });
            let resp_str = format!("{}\n", serde_json::to_string(&resp).unwrap());
            server.write_all(resp_str.as_bytes()).await.unwrap();
            server.flush().await.unwrap();
        });

        let result = IpcClient::send_receive(
            client,
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"test\",\"params\":{}}\n".to_string(),
        )
        .await
        .unwrap();

        assert_eq!(result.get("status").and_then(|v| v.as_str()), Some("ok"));
    }

    #[tokio::test]
    async fn test_send_receive_error() {
        let (client, mut server) = tokio::io::duplex(4096);

        tokio::spawn(async move {
            let resp = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "error": { "code": -32601, "message": "Method not found" }
            });
            let resp_str = format!("{}\n", serde_json::to_string(&resp).unwrap());
            server.write_all(resp_str.as_bytes()).await.unwrap();
            server.flush().await.unwrap();
        });

        let err = IpcClient::send_receive(
            client,
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"test\",\"params\":{}}\n".to_string(),
        )
        .await
        .unwrap_err();

        assert_eq!(err, "Method not found");
    }
}
