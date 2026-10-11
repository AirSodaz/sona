use std::path::PathBuf;
use std::sync::Arc;
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

#[cfg(windows)]
pub type IpcReadHalf = tokio::io::ReadHalf<tokio::net::windows::named_pipe::NamedPipeClient>;
#[cfg(windows)]
pub type IpcWriteHalf = tokio::io::WriteHalf<tokio::net::windows::named_pipe::NamedPipeClient>;

#[cfg(unix)]
pub type IpcReadHalf = tokio::io::ReadHalf<tokio::net::UnixStream>;
#[cfg(unix)]
pub type IpcWriteHalf = tokio::io::WriteHalf<tokio::net::UnixStream>;

#[cfg(any(windows, unix))]
pub struct IpcConnection {
    reader: tokio::io::BufReader<IpcReadHalf>,
    writer: IpcWriteHalf,
}
#[derive(Debug)]
enum IpcCallError {
    Transport(String),
    Application(String),
}

#[derive(Clone, Default)]
pub struct IpcClient {
    custom_endpoint: Option<String>,
    #[cfg(any(windows, unix))]
    connection: Arc<tokio::sync::Mutex<Option<IpcConnection>>>,
}

impl std::fmt::Debug for IpcClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IpcClient")
            .field("custom_endpoint", &self.custom_endpoint)
            .finish()
    }
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
        Self {
            custom_endpoint,
            #[cfg(any(windows, unix))]
            connection: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    pub fn with_endpoint(endpoint: String) -> Self {
        Self {
            custom_endpoint: Some(endpoint),
            #[cfg(any(windows, unix))]
            connection: Arc::new(tokio::sync::Mutex::new(None)),
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

        #[cfg(any(windows, unix))]
        {
            let mut conn_guard = self.connection.lock().await;
            if let Some(conn) = conn_guard.as_mut() {
                match Self::send_receive_on_conn(conn, &req_line).await {
                    Ok(val) => return Ok(val),
                    Err(IpcCallError::Application(app_err)) => return Err(app_err),
                    Err(IpcCallError::Transport(_)) => {
                        *conn_guard = None;
                    }
                }
            }

            let mut new_conn = self.connect_ipc().await?;
            match Self::send_receive_on_conn(&mut new_conn, &req_line).await {
                Ok(val) => {
                    *conn_guard = Some(new_conn);
                    Ok(val)
                }
                Err(IpcCallError::Application(app_err)) => {
                    *conn_guard = Some(new_conn);
                    Err(app_err)
                }
                Err(IpcCallError::Transport(err)) => {
                    *conn_guard = None;
                    Err(err)
                }
            }
        }

        #[cfg(not(any(windows, unix)))]
        {
            Err("IPC transport is not supported on this operating system".to_string())
        }
    }

    async fn send_receive_io<R, W>(
        reader: &mut tokio::io::BufReader<R>,
        writer: &mut W,
        req_line: &str,
    ) -> Result<serde_json::Value, IpcCallError>
    where
        R: tokio::io::AsyncRead + Unpin,
        W: tokio::io::AsyncWrite + Unpin,
    {
        writer.write_all(req_line.as_bytes()).await.map_err(|e| {
            IpcCallError::Transport(format!("Failed to send request to desktop: {e}"))
        })?;
        writer.flush().await.map_err(|e| {
            IpcCallError::Transport(format!("Failed to flush request to desktop: {e}"))
        })?;

        let mut resp_line = String::new();
        tokio::time::timeout(Duration::from_secs(30), reader.read_line(&mut resp_line))
            .await
            .map_err(|_| {
                IpcCallError::Transport(
                    "Timeout waiting for response from desktop client".to_string(),
                )
            })?
            .map_err(|e| {
                IpcCallError::Transport(format!("Failed to read response from desktop: {e}"))
            })?;

        if resp_line.trim().is_empty() {
            return Err(IpcCallError::Transport(
                "Empty response received from desktop client (disconnected)".to_string(),
            ));
        }

        let resp: serde_json::Value = serde_json::from_str(resp_line.trim()).map_err(|e| {
            IpcCallError::Transport(format!("Invalid JSON response from desktop: {e}"))
        })?;

        if let Some(err_obj) = resp.get("error") {
            let msg = err_obj
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error from desktop client");
            return Err(IpcCallError::Application(msg.to_string()));
        }

        resp.get("result").cloned().ok_or_else(|| {
            IpcCallError::Application(
                "Missing 'result' in response from desktop client".to_string(),
            )
        })
    }

    #[cfg(any(windows, unix))]
    async fn send_receive_on_conn(
        conn: &mut IpcConnection,
        req_line: &str,
    ) -> Result<serde_json::Value, IpcCallError> {
        Self::send_receive_io(&mut conn.reader, &mut conn.writer, req_line).await
    }

    #[cfg(windows)]
    async fn connect_ipc(&self) -> Result<IpcConnection, String> {
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

        let (reader, writer) = tokio::io::split(client);
        Ok(IpcConnection {
            reader: tokio::io::BufReader::new(reader),
            writer,
        })
    }

    #[cfg(unix)]
    async fn connect_ipc(&self) -> Result<IpcConnection, String> {
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

        let (reader, writer) = tokio::io::split(stream);
        Ok(IpcConnection {
            reader: tokio::io::BufReader::new(reader),
            writer,
        })
    }

    pub async fn launch_desktop(
        &self,
        timeout_seconds: u64,
        silent: bool,
    ) -> Result<String, String> {
        if self.is_online().await {
            return Ok("Desktop client is already running".to_string());
        }

        let exe_path = find_desktop_binary().ok_or_else(|| {
            "Could not locate Sona desktop executable. Please ensure Sona is installed or set SONA_DESKTOP_PATH.".to_string()
        })?;

        eprintln!(
            "[sona-mcp] Launching desktop client from: {}{}",
            exe_path.display(),
            if silent { " (--silent)" } else { "" }
        );
        let mut cmd = std::process::Command::new(&exe_path);
        if silent {
            cmd.arg("--silent");
        }
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

        let (read_half, mut write_half) = tokio::io::split(client);
        let mut buf_reader = tokio::io::BufReader::new(read_half);
        let result = IpcClient::send_receive_io(
            &mut buf_reader,
            &mut write_half,
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"test\",\"params\":{}}\n",
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

        let (read_half, mut write_half) = tokio::io::split(client);
        let mut buf_reader = tokio::io::BufReader::new(read_half);
        let err = IpcClient::send_receive_io(
            &mut buf_reader,
            &mut write_half,
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"test\",\"params\":{}}\n",
        )
        .await
        .unwrap_err();

        match err {
            IpcCallError::Application(msg) => assert_eq!(msg, "Method not found"),
            other => panic!("Expected application error, got {other:?}"),
        }
    }
}
