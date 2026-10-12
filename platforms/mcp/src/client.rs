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
#[cfg(any(windows, unix))]
impl IpcConnection {
    pub fn is_stale(&mut self) -> bool {
        use std::task::{Context, Poll, Waker};
        use tokio::io::AsyncRead;
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);
        let mut buf = [0u8; 1];
        let mut read_buf = tokio::io::ReadBuf::new(&mut buf);
        match std::pin::Pin::new(&mut self.reader).poll_read(&mut cx, &mut read_buf) {
            Poll::Ready(Ok(())) if read_buf.filled().is_empty() => true,
            Poll::Ready(Err(_)) => true,
            Poll::Pending => false,
            Poll::Ready(Ok(())) => true,
        }
    }
}
pub const MAX_IDLE_CONNECTIONS: usize = 4;

#[cfg(any(windows, unix))]
#[derive(Clone, Default)]
pub struct IpcConnectionPool {
    idle: Arc<tokio::sync::Mutex<Vec<IpcConnection>>>,
}

#[cfg(any(windows, unix))]
impl IpcConnectionPool {
    pub fn new() -> Self {
        Self {
            idle: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        }
    }
}

#[derive(Debug)]
enum IpcCallError {
    Application(String),
    SendFailed(String),
    Transport(String),
}

#[derive(Clone)]
pub struct IpcClient {
    custom_endpoint: Option<String>,
    request_counter: Arc<std::sync::atomic::AtomicU64>,
    #[cfg(any(windows, unix))]
    pool: IpcConnectionPool,
}

impl Default for IpcClient {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for IpcClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IpcClient")
            .field("custom_endpoint", &self.custom_endpoint)
            .finish()
    }
}

pub fn is_idempotent_method(method: &str) -> bool {
    let clean = method
        .strip_prefix("sona_")
        .or_else(|| method.strip_prefix("agent_"))
        .unwrap_or(method);
    matches!(
        clean,
        "get_client_state"
            | "read_transcript"
            | "query_history"
            | "query_trash"
            | "list_projects"
            | "list_audio_devices"
            | "list_transcript_snapshots"
            | "list_sync_conflicts"
            | "get_sync_status"
            | "get_model_catalog"
            | "get_settings"
            | "load_summary"
    )
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
            request_counter: Arc::new(std::sync::atomic::AtomicU64::new(1)),
            #[cfg(any(windows, unix))]
            pool: IpcConnectionPool::new(),
        }
    }

    pub fn with_endpoint(endpoint: String) -> Self {
        Self {
            custom_endpoint: Some(endpoint),
            request_counter: Arc::new(std::sync::atomic::AtomicU64::new(1)),
            #[cfg(any(windows, unix))]
            pool: IpcConnectionPool::new(),
        }
    }
    pub fn next_request_id(&self) -> u64 {
        self.request_counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }
    pub fn is_idempotent_method(method: &str) -> bool {
        is_idempotent_method(method)
    }
    #[cfg(any(windows, unix))]
    pub async fn acquire_connection(&self) -> Result<IpcConnection, String> {
        let (conn, _) = self.acquire_connection_tracked().await?;
        Ok(conn)
    }

    #[cfg(any(windows, unix))]
    pub async fn acquire_connection_tracked(&self) -> Result<(IpcConnection, bool), String> {
        loop {
            let pooled = {
                let mut idle = self.pool.idle.lock().await;
                idle.pop()
            };
            if let Some(mut conn) = pooled {
                if conn.is_stale() {
                    self.clear_pool().await;
                    continue;
                }
                return Ok((conn, true));
            } else {
                let conn = self.connect_ipc().await?;
                return Ok((conn, false));
            }
        }
    }

    #[cfg(any(windows, unix))]
    pub async fn release_connection(&self, conn: IpcConnection) {
        let mut idle = self.pool.idle.lock().await;
        if idle.len() < MAX_IDLE_CONNECTIONS {
            idle.push(conn);
        }
    }

    #[cfg(any(windows, unix))]
    pub async fn clear_pool(&self) {
        let mut idle = self.pool.idle.lock().await;
        idle.clear();
    }

    #[cfg(any(windows, unix))]
    pub async fn idle_connection_count(&self) -> usize {
        self.pool.idle.lock().await.len()
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
        let req_id = self.next_request_id();
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": req_id,
            "method": method,
            "params": params,
        });
        let req_line = format!(
            "{}\n",
            serde_json::to_string(&req).map_err(|e| e.to_string())?
        );

        #[cfg(any(windows, unix))]
        {
            let is_batch_transcribe =
                method == "sona_transcribe_file" || method == "transcribe_file";
            let timeout_duration = if is_batch_transcribe {
                Duration::from_secs(300)
            } else {
                Duration::from_secs(30)
            };

            let (mut conn, _was_pooled) = self.acquire_connection_tracked().await?;
            match Self::send_receive_on_conn(&mut conn, &req_line, req_id, timeout_duration).await {
                Ok(val) => {
                    self.release_connection(conn).await;
                    Ok(val)
                }
                Err(IpcCallError::Application(app_err)) => {
                    self.release_connection(conn).await;
                    Err(app_err)
                }
                Err(IpcCallError::SendFailed(_)) => {
                    drop(conn);
                    self.clear_pool().await;
                    let mut fresh_conn = self.connect_ipc().await?;
                    match Self::send_receive_on_conn(
                        &mut fresh_conn,
                        &req_line,
                        req_id,
                        timeout_duration,
                    )
                    .await
                    {
                        Ok(val) => {
                            self.release_connection(fresh_conn).await;
                            Ok(val)
                        }
                        Err(IpcCallError::Application(app_err)) => {
                            self.release_connection(fresh_conn).await;
                            Err(app_err)
                        }
                        Err(IpcCallError::SendFailed(err)) | Err(IpcCallError::Transport(err)) => {
                            Err(err)
                        }
                    }
                }
                Err(IpcCallError::Transport(err)) => {
                    drop(conn);
                    self.clear_pool().await;
                    if !Self::is_idempotent_method(method) {
                        return Err(format!(
                            "Transport error communicating with desktop client: {err} (non-idempotent operation '{method}' will not retry)"
                        ));
                    }
                    let mut fresh_conn = self.connect_ipc().await?;
                    match Self::send_receive_on_conn(
                        &mut fresh_conn,
                        &req_line,
                        req_id,
                        timeout_duration,
                    )
                    .await
                    {
                        Ok(val) => {
                            self.release_connection(fresh_conn).await;
                            Ok(val)
                        }
                        Err(IpcCallError::Application(app_err)) => {
                            self.release_connection(fresh_conn).await;
                            Err(app_err)
                        }
                        Err(IpcCallError::SendFailed(retry_err))
                        | Err(IpcCallError::Transport(retry_err)) => Err(retry_err),
                    }
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
        req_id: u64,
        timeout_duration: Duration,
    ) -> Result<serde_json::Value, IpcCallError>
    where
        R: tokio::io::AsyncRead + Unpin,
        W: tokio::io::AsyncWrite + Unpin,
    {
        writer.write_all(req_line.as_bytes()).await.map_err(|e| {
            IpcCallError::SendFailed(format!("Failed to send request to desktop: {e}"))
        })?;
        writer.flush().await.map_err(|e| {
            IpcCallError::SendFailed(format!("Failed to flush request to desktop: {e}"))
        })?;

        let mut resp_line = String::new();
        tokio::time::timeout(timeout_duration, reader.read_line(&mut resp_line))
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
        if let Some(resp_id) = resp.get("id")
            && resp_id != &serde_json::json!(req_id)
        {
            return Err(IpcCallError::Transport(format!(
                "Response ID mismatch: expected {req_id}, got {resp_id}"
            )));
        }

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
        req_id: u64,
        timeout_duration: Duration,
    ) -> Result<serde_json::Value, IpcCallError> {
        Self::send_receive_io(
            &mut conn.reader,
            &mut conn.writer,
            req_line,
            req_id,
            timeout_duration,
        )
        .await
    }

    #[cfg(windows)]
    async fn connect_ipc(&self) -> Result<IpcConnection, String> {
        let pipe_name = self
            .custom_endpoint
            .as_deref()
            .unwrap_or(WINDOWS_PIPE_NAME)
            .to_string();

        let mut pipe_busy_attempts = 0;
        let client = loop {
            let pipe_name_clone = pipe_name.clone();
            let open_res = tokio::task::spawn_blocking(move || {
                tokio::net::windows::named_pipe::ClientOptions::new().open(&pipe_name_clone)
            })
            .await
            .map_err(|e| format!("Join error: {e}"))?;

            match open_res {
                Ok(client) => break client,
                Err(err) if err.raw_os_error() == Some(231) && pipe_busy_attempts < 10 => {
                    pipe_busy_attempts += 1;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(_) => {
                    return Err("DESKTOP_OFFLINE: Sona desktop client is not running. Call sona_launch_desktop to start it.".to_string());
                }
            }
        };

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

        let mut attempts = 0;
        let stream = loop {
            match tokio::time::timeout(
                Duration::from_secs(2),
                tokio::net::UnixStream::connect(&socket_path),
            )
            .await
            {
                Ok(Ok(stream)) => break stream,
                Ok(Err(err))
                    if err.kind() == std::io::ErrorKind::ConnectionRefused && attempts < 5 =>
                {
                    attempts += 1;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                _ => {
                    return Err("DESKTOP_OFFLINE: Sona desktop client is not running. Call sona_launch_desktop to start it.".to_string());
                }
            }
        };

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
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const DETACHED_PROCESS: u32 = 0x0000_0008;
            cmd.creation_flags(DETACHED_PROCESS);
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

        let mut cur = parent.as_path();
        while let Some(anc) = cur.parent() {
            for rel in &[
                "target/debug/sona.exe",
                "target/release/sona.exe",
                "target/debug/sona",
                "target/release/sona",
            ] {
                let candidate = anc.join(rel);
                if candidate.exists() {
                    return Some(candidate);
                }
            }
            cur = anc;
        }
    }

    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let m_path = PathBuf::from(manifest_dir);
        let workspace_root = m_path.parent().and_then(|p| p.parent()).unwrap_or(&m_path);
        for rel in &[
            "target/debug/sona.exe",
            "target/release/sona.exe",
            "target/debug/sona",
            "target/release/sona",
        ] {
            let candidate = workspace_root.join(rel);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    if std::path::Path::new("Cargo.toml").exists() {
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
        if let Ok(home) = std::env::var("HOME") {
            let candidate = PathBuf::from(home).join(".local").join("bin").join("sona");
            if candidate.exists() {
                return Some(candidate);
            }
        }
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
            1,
            Duration::from_secs(30),
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
            1,
            Duration::from_secs(30),
        )
        .await
        .unwrap_err();

        match err {
            IpcCallError::Application(msg) => assert_eq!(msg, "Method not found"),
            other => panic!("Expected application error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_send_receive_batch_transcribe_call() {
        let (client, mut server) = tokio::io::duplex(4096);

        tokio::spawn(async move {
            let mut reader = tokio::io::BufReader::new(&mut server);
            let mut line = String::new();
            reader.read_line(&mut line).await.unwrap();

            let resp = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": { "status": "completed", "segments": [] }
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
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"sona_transcribe_file\",\"params\":{\"file_path\":\"test.wav\"}}\n",
            1,
            Duration::from_secs(300),
        )
        .await
        .unwrap();
        assert_eq!(
            result.get("status").and_then(|v| v.as_str()),
            Some("completed")
        );
    }
    #[tokio::test]
    async fn test_response_id_mismatch() {
        let (client, mut server) = tokio::io::duplex(4096);

        tokio::spawn(async move {
            let mut reader = tokio::io::BufReader::new(&mut server);
            let mut line = String::new();
            reader.read_line(&mut line).await.unwrap();

            // Return response with mismatched id: 999 instead of expected 42
            let resp = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 999,
                "result": { "status": "ok" }
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
            "{\"jsonrpc\":\"2.0\",\"id\":42,\"method\":\"test\",\"params\":{}}\n",
            42,
            Duration::from_secs(30),
        )
        .await
        .unwrap_err();

        match err {
            IpcCallError::Transport(msg) => {
                assert!(
                    msg.contains("Response ID mismatch: expected 42, got 999"),
                    "Expected id mismatch message, got: {msg}"
                );
            }
            other => panic!("Expected Transport error, got {other:?}"),
        }
    }

    #[test]
    fn test_request_counter_increment() {
        let client = IpcClient::new();
        assert_eq!(client.next_request_id(), 1);
        assert_eq!(client.next_request_id(), 2);
        assert_eq!(client.next_request_id(), 3);
        let client_clone = client.clone();
        assert_eq!(client_clone.next_request_id(), 4);
        assert_eq!(client.next_request_id(), 5);
    }
    #[test]
    fn test_is_idempotent_method() {
        let idempotent = [
            "get_client_state",
            "sona_get_client_state",
            "agent_get_client_state",
            "read_transcript",
            "sona_read_transcript",
            "query_history",
            "sona_query_history",
            "query_trash",
            "list_projects",
            "list_audio_devices",
            "list_transcript_snapshots",
            "list_sync_conflicts",
            "get_sync_status",
            "get_model_catalog",
            "get_settings",
            "load_summary",
        ];
        for method in idempotent {
            assert!(
                is_idempotent_method(method),
                "Method {method} should be classified as idempotent"
            );
        }

        let non_idempotent = [
            "start_recording",
            "sona_start_recording",
            "stop_recording",
            "sona_stop_recording",
            "edit_transcript",
            "patch_segments",
            "update_translations",
            "delete_history",
            "sona_delete_history",
            "create_project",
            "download_preset_model",
            "transcribe_file",
            "sona_transcribe_file",
            "unknown_method",
        ];
        for method in non_idempotent {
            assert!(
                !is_idempotent_method(method),
                "Method {method} should NOT be classified as idempotent"
            );
        }
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn test_non_idempotent_operation_does_not_retry_on_transport_error() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use tokio::net::windows::named_pipe::ServerOptions;

        let seq = TEST_PIPE_SEQ.fetch_add(1, Ordering::Relaxed);
        let pipe_name = format!(
            r"\\.\pipe\sona-mcp-test-idempotent-{}-{}",
            std::process::id(),
            seq
        );
        let endpoint = pipe_name.clone();
        let connection_count = Arc::new(AtomicUsize::new(0));
        let connection_count_srv = connection_count.clone();
        let server_shutdown = Arc::new(tokio::sync::Notify::new());
        let server_shutdown_clone = server_shutdown.clone();

        let srv_handle = tokio::spawn(async move {
            let mut server = match ServerOptions::new()
                .first_pipe_instance(true)
                .create(&pipe_name)
            {
                Ok(s) => s,
                Err(_) => return,
            };

            loop {
                tokio::select! {
                    _ = server_shutdown_clone.notified() => break,
                    conn_res = server.connect() => {
                        if conn_res.is_err() {
                            break;
                        }
                        connection_count_srv.fetch_add(1, Ordering::SeqCst);
                        let connected = server;

                        tokio::spawn(async move {
                            let (read_half, mut write_half) = tokio::io::split(connected);
                            let mut reader = tokio::io::BufReader::new(read_half);
                            let mut line = String::new();
                            while reader.read_line(&mut line).await.is_ok() {
                                if line.trim().is_empty() {
                                    break;
                                }
                                if line.contains("sona_delete_history") {
                                    // Simulate in-flight transport error: drop without replying
                                    drop(reader);
                                    drop(write_half);
                                    return;
                                }
                                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                                    let id = val.get("id").cloned().unwrap_or(serde_json::json!(1));
                                    let resp = serde_json::json!({
                                        "jsonrpc": "2.0",
                                        "id": id,
                                        "result": { "status": "ok" }
                                    });
                                    let _ = write_half.write_all(format!("{resp}\n").as_bytes()).await;
                                    let _ = write_half.flush().await;
                                }
                                line.clear();
                            }
                        });

                        match ServerOptions::new().create(&pipe_name) {
                            Ok(next_s) => server = next_s,
                            Err(_) => break,
                        }
                    }
                }
            }
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        let client = IpcClient::with_endpoint(endpoint);

        // First call populates the cached connection
        let first_res = client
            .call("sona_get_client_state", serde_json::json!({}))
            .await;
        assert!(
            first_res.is_ok(),
            "First call should succeed: {first_res:?}"
        );
        assert_eq!(connection_count.load(Ordering::SeqCst), 1);

        // Wait a moment for server to drop connection 1
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Second call with mutating method should fail on transport and NOT retry
        let mutating_res = client
            .call(
                "sona_delete_history",
                serde_json::json!({ "id": "entry-1" }),
            )
            .await;
        assert!(
            mutating_res.is_err(),
            "Mutating call on dead connection must fail"
        );
        let err_msg = mutating_res.unwrap_err();
        assert!(
            err_msg.contains("non-idempotent operation 'sona_delete_history' will not retry"),
            "Error should explain non-idempotent operation will not retry: {err_msg}"
        );
        assert_eq!(
            connection_count.load(Ordering::SeqCst),
            1,
            "Mutating method must NOT open a second connection on transport failure"
        );

        // Subsequent idempotent call CAN reconnect and succeed
        let third_res = client
            .call("sona_get_client_state", serde_json::json!({}))
            .await;
        assert!(
            third_res.is_ok(),
            "Idempotent call should reconnect and succeed: {third_res:?}"
        );
        assert_eq!(
            connection_count.load(Ordering::SeqCst),
            2,
            "Idempotent method should have opened connection 2"
        );

        server_shutdown.notify_waiters();
        let _ = srv_handle.await;
    }
    #[cfg(windows)]
    #[tokio::test]
    async fn test_non_idempotent_operation_retries_on_stale_idle_connection() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use tokio::net::windows::named_pipe::ServerOptions;

        let seq = TEST_PIPE_SEQ.fetch_add(1, Ordering::Relaxed);
        let pipe_name = format!(
            r"\\.\pipe\sona-mcp-test-stale-{}-{}",
            std::process::id(),
            seq
        );
        let endpoint = pipe_name.clone();
        let connection_count = Arc::new(AtomicUsize::new(0));
        let connection_count_srv = connection_count.clone();
        let server_shutdown = Arc::new(tokio::sync::Notify::new());
        let server_shutdown_clone = server_shutdown.clone();

        let srv_handle = tokio::spawn(async move {
            let mut server = match ServerOptions::new()
                .first_pipe_instance(true)
                .create(&pipe_name)
            {
                Ok(s) => s,
                Err(_) => return,
            };

            loop {
                tokio::select! {
                    _ = server_shutdown_clone.notified() => break,
                    conn_res = server.connect() => {
                        if conn_res.is_err() {
                            break;
                        }
                        connection_count_srv.fetch_add(1, Ordering::SeqCst);
                        let connected = server;
                        let count = connection_count_srv.load(Ordering::SeqCst);

                        tokio::spawn(async move {
                            let (read_half, mut write_half) = tokio::io::split(connected);
                            let mut reader = tokio::io::BufReader::new(read_half);
                            let mut line = String::new();
                            if reader.read_line(&mut line).await.is_ok()
                                && let Ok(val) = serde_json::from_str::<serde_json::Value>(&line)
                            {
                                    let id = val.get("id").cloned().unwrap_or(serde_json::json!(1));
                                    let resp = serde_json::json!({
                                        "jsonrpc": "2.0",
                                        "id": id,
                                        "result": { "status": "ok" }
                                    });
                                    let _ = write_half.write_all(format!("{resp}\n").as_bytes()).await;
                                    let _ = write_half.flush().await;
                                }
                            if count == 1 {
                                drop(reader);
                                drop(write_half);
                            }
                        });

                        match ServerOptions::new().create(&pipe_name) {
                            Ok(next_s) => server = next_s,
                            Err(_) => break,
                        }
                    }
                }
            }
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        let client = IpcClient::with_endpoint(endpoint);

        let first_res = client
            .call("sona_get_client_state", serde_json::json!({}))
            .await;
        assert!(first_res.is_ok());
        assert_eq!(connection_count.load(Ordering::SeqCst), 1);

        tokio::time::sleep(Duration::from_millis(100)).await;

        let mutating_res = client
            .call(
                "sona_delete_history",
                serde_json::json!({ "id": "entry-1" }),
            )
            .await;
        assert!(
            mutating_res.is_ok(),
            "Non-idempotent operation should reconnect and succeed on stale idle connection: {mutating_res:?}"
        );
        assert_eq!(
            connection_count.load(Ordering::SeqCst),
            2,
            "Should have opened connection 2 after stale connection send failure"
        );

        server_shutdown.notify_waiters();
        let _ = srv_handle.await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_non_idempotent_operation_does_not_retry_on_transport_error() {
        use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
        static TEST_SOCK_SEQ_IDEMP: AtomicU64 = AtomicU64::new(200);

        let seq = TEST_SOCK_SEQ_IDEMP.fetch_add(1, Ordering::Relaxed);
        let socket_path = std::env::temp_dir().join(format!(
            "sona-test-idemp-{}-{}.sock",
            std::process::id(),
            seq
        ));
        let _ = std::fs::remove_file(&socket_path);
        let endpoint = socket_path.to_string_lossy().to_string();
        let connection_count = Arc::new(AtomicUsize::new(0));
        let connection_count_srv = connection_count.clone();
        let server_shutdown = Arc::new(tokio::sync::Notify::new());
        let server_shutdown_clone = server_shutdown.clone();

        let listener = tokio::net::UnixListener::bind(&socket_path).unwrap();

        let srv_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = server_shutdown_clone.notified() => break,
                    accept_res = listener.accept() => {
                        let (stream, _) = match accept_res {
                            Ok(s) => s,
                            Err(_) => break,
                        };
                        connection_count_srv.fetch_add(1, Ordering::SeqCst);
                        tokio::spawn(async move {
                            let (read_half, mut write_half) = tokio::io::split(stream);
                            let mut reader = tokio::io::BufReader::new(read_half);
                            let mut line = String::new();
                            while reader.read_line(&mut line).await.is_ok() {
                                if line.trim().is_empty() {
                                    break;
                                }
                                if line.contains("sona_delete_history") {
                                    // Simulate in-flight transport error: drop without replying
                                    drop(reader);
                                    drop(write_half);
                                    return;
                                }
                                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                                    let id = val.get("id").cloned().unwrap_or(serde_json::json!(1));
                                    let resp = serde_json::json!({
                                        "jsonrpc": "2.0",
                                        "id": id,
                                        "result": { "status": "ok" }
                                    });
                                    let _ = write_half.write_all(format!("{resp}\n").as_bytes()).await;
                                    let _ = write_half.flush().await;
                                }
                                line.clear();
                            }
                        });
                    }
                }
            }
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        let client = IpcClient::with_endpoint(endpoint);

        let first_res = client
            .call("sona_get_client_state", serde_json::json!({}))
            .await;
        assert!(
            first_res.is_ok(),
            "First call should succeed: {first_res:?}"
        );
        assert_eq!(connection_count.load(Ordering::SeqCst), 1);

        tokio::time::sleep(Duration::from_millis(100)).await;

        let mutating_res = client
            .call(
                "sona_delete_history",
                serde_json::json!({ "id": "entry-1" }),
            )
            .await;
        assert!(
            mutating_res.is_err(),
            "Mutating call on dead connection must fail"
        );
        let err_msg = mutating_res.unwrap_err();
        assert!(
            err_msg.contains("non-idempotent operation 'sona_delete_history' will not retry"),
            "Error should explain non-idempotent operation will not retry: {err_msg}"
        );
        assert_eq!(
            connection_count.load(Ordering::SeqCst),
            1,
            "Mutating method must NOT open a second connection on transport failure"
        );

        let third_res = client
            .call("sona_get_client_state", serde_json::json!({}))
            .await;
        assert!(
            third_res.is_ok(),
            "Idempotent call should reconnect and succeed: {third_res:?}"
        );
        assert_eq!(
            connection_count.load(Ordering::SeqCst),
            2,
            "Idempotent method should have opened connection 2"
        );

        server_shutdown.notify_waiters();
        let _ = srv_handle.await;
        let _ = std::fs::remove_file(&socket_path);
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn test_non_idempotent_operation_retries_on_stale_idle_connection() {
        use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
        static TEST_SOCK_SEQ_STALE: AtomicU64 = AtomicU64::new(300);

        let seq = TEST_SOCK_SEQ_STALE.fetch_add(1, Ordering::Relaxed);
        let socket_path = std::env::temp_dir().join(format!(
            "sona-test-stale-{}-{}.sock",
            std::process::id(),
            seq
        ));
        let _ = std::fs::remove_file(&socket_path);
        let endpoint = socket_path.to_string_lossy().to_string();
        let connection_count = Arc::new(AtomicUsize::new(0));
        let connection_count_srv = connection_count.clone();
        let server_shutdown = Arc::new(tokio::sync::Notify::new());
        let server_shutdown_clone = server_shutdown.clone();

        let listener = tokio::net::UnixListener::bind(&socket_path).unwrap();

        let srv_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = server_shutdown_clone.notified() => break,
                    accept_res = listener.accept() => {
                        let (stream, _) = match accept_res {
                            Ok(s) => s,
                            Err(_) => break,
                        };
                        connection_count_srv.fetch_add(1, Ordering::SeqCst);
                        let count = connection_count_srv.load(Ordering::SeqCst);
                        tokio::spawn(async move {
                            let (read_half, mut write_half) = tokio::io::split(stream);
                            let mut reader = tokio::io::BufReader::new(read_half);
                            let mut line = String::new();
                            if reader.read_line(&mut line).await.is_ok()
                                && let Ok(val) = serde_json::from_str::<serde_json::Value>(&line)
                            {
                                    let id = val.get("id").cloned().unwrap_or(serde_json::json!(1));
                                    let resp = serde_json::json!({
                                        "jsonrpc": "2.0",
                                        "id": id,
                                        "result": { "status": "ok" }
                                    });
                                    let _ = write_half.write_all(format!("{resp}\n").as_bytes()).await;
                                    let _ = write_half.flush().await;
                                }
                            if count == 1 {
                                drop(reader);
                                drop(write_half);
                            }
                        });
                    }
                }
            }
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        let client = IpcClient::with_endpoint(endpoint);

        let first_res = client
            .call("sona_get_client_state", serde_json::json!({}))
            .await;
        assert!(first_res.is_ok());
        assert_eq!(connection_count.load(Ordering::SeqCst), 1);

        tokio::time::sleep(Duration::from_millis(100)).await;

        let mutating_res = client
            .call(
                "sona_delete_history",
                serde_json::json!({ "id": "entry-1" }),
            )
            .await;
        assert!(
            mutating_res.is_ok(),
            "Non-idempotent operation should reconnect and succeed on stale idle connection: {mutating_res:?}"
        );
        assert_eq!(
            connection_count.load(Ordering::SeqCst),
            2,
            "Should have opened connection 2 after stale connection send failure"
        );

        server_shutdown.notify_waiters();
        let _ = srv_handle.await;
        let _ = std::fs::remove_file(&socket_path);
    }

    #[cfg(windows)]
    static TEST_PIPE_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(100);

    #[cfg(windows)]
    #[tokio::test]
    async fn test_concurrent_batch_transcribe_and_cancel() {
        use tokio::net::windows::named_pipe::ServerOptions;

        let seq = TEST_PIPE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let pipe_name = format!(
            r"\\.\pipe\sona-mcp-test-cancel-{}-{}",
            std::process::id(),
            seq
        );
        let endpoint = pipe_name.clone();

        let cancel_received = Arc::new(tokio::sync::Notify::new());
        let cancel_received_server = cancel_received.clone();
        let server_shutdown = Arc::new(tokio::sync::Notify::new());
        let server_shutdown_clone = server_shutdown.clone();

        let srv_handle = tokio::spawn(async move {
            let mut server = match ServerOptions::new()
                .first_pipe_instance(true)
                .create(&pipe_name)
            {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Failed to create pipe: {e}");
                    return;
                }
            };

            loop {
                tokio::select! {
                    _ = server_shutdown_clone.notified() => break,
                    conn_res = server.connect() => {
                        if conn_res.is_err() {
                            break;
                        }
                        let connected = server;
                        let cancel_notif = cancel_received_server.clone();
                        tokio::spawn(async move {
                            let (read_half, mut write_half) = tokio::io::split(connected);
                            let mut reader = tokio::io::BufReader::new(read_half);
                            let mut line = String::new();
                            if reader.read_line(&mut line).await.is_ok()
                                && let Ok(val) = serde_json::from_str::<serde_json::Value>(&line)
                            {
                                    let method = val.get("method").and_then(|m| m.as_str()).unwrap_or("");
                                    let id = val.get("id").cloned().unwrap_or(serde_json::json!(1));
                                    if method == "sona_transcribe_file" {
                                        tokio::select! {
                                            _ = cancel_notif.notified() => {}
                                            _ = tokio::time::sleep(Duration::from_secs(3)) => {}
                                        }
                                        let resp = serde_json::json!({
                                            "jsonrpc": "2.0",
                                            "id": id,
                                            "result": { "status": "cancelled" }
                                        });
                                        let _ = write_half.write_all(format!("{resp}\n").as_bytes()).await;
                                    } else if method == "sona_cancel_batch_task" {
                                        cancel_notif.notify_waiters();
                                        let resp = serde_json::json!({
                                            "jsonrpc": "2.0",
                                            "id": id,
                                            "result": { "status": "ok", "task_id": "batch-123" }
                                        });
                                        let _ = write_half.write_all(format!("{resp}\n").as_bytes()).await;
                                    }
                                }
                        });

                        match ServerOptions::new().create(&pipe_name) {
                            Ok(next_s) => server = next_s,
                            Err(_) => break,
                        }
                    }
                }
            }
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        let client = IpcClient::with_endpoint(endpoint);

        let client_clone = client.clone();
        let transcribe_task = tokio::spawn(async move {
            client_clone
                .call(
                    "sona_transcribe_file",
                    serde_json::json!({ "file_path": "audio.wav" }),
                )
                .await
        });

        tokio::time::sleep(Duration::from_millis(80)).await;

        let start = std::time::Instant::now();
        let cancel_res = tokio::time::timeout(
            Duration::from_secs(1),
            client.call(
                "sona_cancel_batch_task",
                serde_json::json!({ "task_id": "batch-123" }),
            ),
        )
        .await
        .expect("Cancel call must not time out");

        assert!(cancel_res.is_ok(), "Cancel call failed: {cancel_res:?}");
        let cancel_val = cancel_res.unwrap();
        assert_eq!(
            cancel_val.get("status").and_then(|s| s.as_str()),
            Some("ok")
        );
        assert!(
            start.elapsed() < Duration::from_millis(500),
            "Cancel must complete promptly without waiting for transcribe"
        );

        let transcribe_res = transcribe_task.await.unwrap();
        assert!(transcribe_res.is_ok());

        server_shutdown.notify_waiters();
        let _ = srv_handle.await;
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn test_connection_pool_reuses_idle() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use tokio::net::windows::named_pipe::ServerOptions;

        let seq = TEST_PIPE_SEQ.fetch_add(1, Ordering::Relaxed);
        let pipe_name = format!(
            r"\\.\pipe\sona-mcp-test-pool-reuse-{}-{}",
            std::process::id(),
            seq
        );
        let endpoint = pipe_name.clone();
        let connection_count = Arc::new(AtomicUsize::new(0));
        let connection_count_srv = connection_count.clone();
        let server_shutdown = Arc::new(tokio::sync::Notify::new());
        let server_shutdown_clone = server_shutdown.clone();

        let srv_handle = tokio::spawn(async move {
            let mut server = match ServerOptions::new()
                .first_pipe_instance(true)
                .create(&pipe_name)
            {
                Ok(s) => s,
                Err(_) => return,
            };

            loop {
                tokio::select! {
                    _ = server_shutdown_clone.notified() => break,
                    conn_res = server.connect() => {
                        if conn_res.is_err() {
                            break;
                        }
                        connection_count_srv.fetch_add(1, Ordering::SeqCst);
                        let connected = server;

                        tokio::spawn(async move {
                            let (read_half, mut write_half) = tokio::io::split(connected);
                            let mut reader = tokio::io::BufReader::new(read_half);
                            let mut line = String::new();
                            while reader.read_line(&mut line).await.is_ok() {
                                if line.trim().is_empty() {
                                    break;
                                }
                                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                                    let id = val.get("id").cloned().unwrap_or(serde_json::json!(1));
                                    let resp = serde_json::json!({
                                        "jsonrpc": "2.0",
                                        "id": id,
                                        "result": { "status": "ok" }
                                    });
                                    let _ = write_half.write_all(format!("{resp}\n").as_bytes()).await;
                                    let _ = write_half.flush().await;
                                }
                                line.clear();
                            }
                        });

                        match ServerOptions::new().create(&pipe_name) {
                            Ok(next_s) => server = next_s,
                            Err(_) => break,
                        }
                    }
                }
            }
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        let client = IpcClient::with_endpoint(endpoint);

        let r1 = client
            .call("sona_get_client_state", serde_json::json!({}))
            .await;
        assert!(r1.is_ok());
        assert_eq!(connection_count.load(Ordering::SeqCst), 1);
        assert_eq!(client.idle_connection_count().await, 1);

        let r2 = client
            .call("sona_get_client_state", serde_json::json!({}))
            .await;
        assert!(r2.is_ok());
        assert_eq!(connection_count.load(Ordering::SeqCst), 1);
        assert_eq!(client.idle_connection_count().await, 1);

        let r3 = client
            .call("sona_get_client_state", serde_json::json!({}))
            .await;
        assert!(r3.is_ok());
        assert_eq!(connection_count.load(Ordering::SeqCst), 1);
        assert_eq!(client.idle_connection_count().await, 1);

        server_shutdown.notify_waiters();
        let _ = srv_handle.await;
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn test_connection_pool_concurrent_calls() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use tokio::net::windows::named_pipe::ServerOptions;

        let seq = TEST_PIPE_SEQ.fetch_add(1, Ordering::Relaxed);
        let pipe_name = format!(
            r"\\.\pipe\sona-mcp-test-pool-concurrent-{}-{}",
            std::process::id(),
            seq
        );
        let endpoint = pipe_name.clone();
        let connection_count = Arc::new(AtomicUsize::new(0));
        let connection_count_srv = connection_count.clone();
        let server_shutdown = Arc::new(tokio::sync::Notify::new());
        let server_shutdown_clone = server_shutdown.clone();

        let srv_handle = tokio::spawn(async move {
            let mut server = match ServerOptions::new()
                .first_pipe_instance(true)
                .create(&pipe_name)
            {
                Ok(s) => s,
                Err(_) => return,
            };

            loop {
                tokio::select! {
                    _ = server_shutdown_clone.notified() => break,
                    conn_res = server.connect() => {
                        if conn_res.is_err() {
                            break;
                        }
                        connection_count_srv.fetch_add(1, Ordering::SeqCst);
                        let connected = server;

                        tokio::spawn(async move {
                            let (read_half, mut write_half) = tokio::io::split(connected);
                            let mut reader = tokio::io::BufReader::new(read_half);
                            let mut line = String::new();
                            while reader.read_line(&mut line).await.is_ok() {
                                if line.trim().is_empty() {
                                    break;
                                }
                                tokio::time::sleep(Duration::from_millis(80)).await;
                                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                                    let id = val.get("id").cloned().unwrap_or(serde_json::json!(1));
                                    let resp = serde_json::json!({
                                        "jsonrpc": "2.0",
                                        "id": id,
                                        "result": { "status": "ok" }
                                    });
                                    let _ = write_half.write_all(format!("{resp}\n").as_bytes()).await;
                                    let _ = write_half.flush().await;
                                }
                                line.clear();
                            }
                        });

                        match ServerOptions::new().create(&pipe_name) {
                            Ok(next_s) => server = next_s,
                            Err(_) => break,
                        }
                    }
                }
            }
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        let client = IpcClient::with_endpoint(endpoint);

        let c1 = client.clone();
        let c2 = client.clone();
        let (res1, res2) = tokio::join!(
            tokio::spawn(async move {
                c1.call("sona_get_client_state", serde_json::json!({}))
                    .await
            }),
            tokio::spawn(async move {
                c2.call("sona_get_client_state", serde_json::json!({}))
                    .await
            })
        );

        assert!(res1.unwrap().is_ok());
        assert!(res2.unwrap().is_ok());

        assert_eq!(connection_count.load(Ordering::SeqCst), 2);
        assert_eq!(client.idle_connection_count().await, 2);

        server_shutdown.notify_waiters();
        let _ = srv_handle.await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_concurrent_batch_transcribe_and_cancel() {
        use std::sync::atomic::{AtomicU64, Ordering};
        static TEST_SOCK_SEQ: AtomicU64 = AtomicU64::new(100);

        let seq = TEST_SOCK_SEQ.fetch_add(1, Ordering::Relaxed);
        let socket_path = std::env::temp_dir().join(format!(
            "sona-test-cancel-{}-{}.sock",
            std::process::id(),
            seq
        ));
        let _ = std::fs::remove_file(&socket_path);
        let endpoint = socket_path.to_string_lossy().to_string();

        let cancel_received = Arc::new(tokio::sync::Notify::new());
        let cancel_received_server = cancel_received.clone();
        let server_shutdown = Arc::new(tokio::sync::Notify::new());
        let server_shutdown_clone = server_shutdown.clone();

        let listener = tokio::net::UnixListener::bind(&socket_path).unwrap();

        let srv_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = server_shutdown_clone.notified() => break,
                    accept_res = listener.accept() => {
                        let (stream, _) = match accept_res {
                            Ok(s) => s,
                            Err(_) => break,
                        };
                        let cancel_notif = cancel_received_server.clone();
                        tokio::spawn(async move {
                            let (read_half, mut write_half) = tokio::io::split(stream);
                            let mut reader = tokio::io::BufReader::new(read_half);
                            let mut line = String::new();
                            if reader.read_line(&mut line).await.is_ok()
                                && let Ok(val) = serde_json::from_str::<serde_json::Value>(&line)
                            {
                                    let method = val.get("method").and_then(|m| m.as_str()).unwrap_or("");
                                    let id = val.get("id").cloned().unwrap_or(serde_json::json!(1));
                                    if method == "sona_transcribe_file" {
                                        tokio::select! {
                                            _ = cancel_notif.notified() => {}
                                            _ = tokio::time::sleep(Duration::from_secs(3)) => {}
                                        }
                                        let resp = serde_json::json!({
                                            "jsonrpc": "2.0",
                                            "id": id,
                                            "result": { "status": "cancelled" }
                                        });
                                        let _ = write_half.write_all(format!("{resp}\n").as_bytes()).await;
                                    } else if method == "sona_cancel_batch_task" {
                                        cancel_notif.notify_waiters();
                                        let resp = serde_json::json!({
                                            "jsonrpc": "2.0",
                                            "id": id,
                                            "result": { "status": "ok", "task_id": "batch-123" }
                                        });
                                        let _ = write_half.write_all(format!("{resp}\n").as_bytes()).await;
                                    }
                                }
                        });
                    }
                }
            }
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        let client = IpcClient::with_endpoint(endpoint);

        let client_clone = client.clone();
        let transcribe_task = tokio::spawn(async move {
            client_clone
                .call(
                    "sona_transcribe_file",
                    serde_json::json!({ "file_path": "audio.wav" }),
                )
                .await
        });

        tokio::time::sleep(Duration::from_millis(80)).await;

        let start = std::time::Instant::now();
        let cancel_res = tokio::time::timeout(
            Duration::from_secs(1),
            client.call(
                "sona_cancel_batch_task",
                serde_json::json!({ "task_id": "batch-123" }),
            ),
        )
        .await
        .expect("Cancel call must not time out");

        assert!(cancel_res.is_ok(), "Cancel call failed: {cancel_res:?}");
        let cancel_val = cancel_res.unwrap();
        assert_eq!(
            cancel_val.get("status").and_then(|s| s.as_str()),
            Some("ok")
        );
        assert!(
            start.elapsed() < Duration::from_millis(500),
            "Cancel must complete promptly without waiting for transcribe"
        );

        let transcribe_res = transcribe_task.await.unwrap();
        assert!(transcribe_res.is_ok());

        server_shutdown.notify_waiters();
        let _ = srv_handle.await;
        let _ = std::fs::remove_file(&socket_path);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_connection_pool_reuses_idle() {
        use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
        static TEST_SOCK_SEQ_POOL: AtomicU64 = AtomicU64::new(400);

        let seq = TEST_SOCK_SEQ_POOL.fetch_add(1, Ordering::Relaxed);
        let socket_path = std::env::temp_dir().join(format!(
            "sona-test-pool-reuse-{}-{}.sock",
            std::process::id(),
            seq
        ));
        let _ = std::fs::remove_file(&socket_path);
        let endpoint = socket_path.to_string_lossy().to_string();
        let connection_count = Arc::new(AtomicUsize::new(0));
        let connection_count_srv = connection_count.clone();
        let server_shutdown = Arc::new(tokio::sync::Notify::new());
        let server_shutdown_clone = server_shutdown.clone();

        let listener = tokio::net::UnixListener::bind(&socket_path).unwrap();

        let srv_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = server_shutdown_clone.notified() => break,
                    accept_res = listener.accept() => {
                        let (stream, _) = match accept_res {
                            Ok(s) => s,
                            Err(_) => break,
                        };
                        connection_count_srv.fetch_add(1, Ordering::SeqCst);
                        tokio::spawn(async move {
                            let (read_half, mut write_half) = tokio::io::split(stream);
                            let mut reader = tokio::io::BufReader::new(read_half);
                            let mut line = String::new();
                            while reader.read_line(&mut line).await.is_ok() {
                                if line.trim().is_empty() {
                                    break;
                                }
                                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                                    let id = val.get("id").cloned().unwrap_or(serde_json::json!(1));
                                    let resp = serde_json::json!({
                                        "jsonrpc": "2.0",
                                        "id": id,
                                        "result": { "status": "ok" }
                                    });
                                    let _ = write_half.write_all(format!("{resp}\n").as_bytes()).await;
                                    let _ = write_half.flush().await;
                                }
                                line.clear();
                            }
                        });
                    }
                }
            }
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        let client = IpcClient::with_endpoint(endpoint);

        let r1 = client
            .call("sona_get_client_state", serde_json::json!({}))
            .await;
        assert!(r1.is_ok());
        assert_eq!(connection_count.load(Ordering::SeqCst), 1);
        assert_eq!(client.idle_connection_count().await, 1);

        let r2 = client
            .call("sona_get_client_state", serde_json::json!({}))
            .await;
        assert!(r2.is_ok());
        assert_eq!(connection_count.load(Ordering::SeqCst), 1);
        assert_eq!(client.idle_connection_count().await, 1);

        let r3 = client
            .call("sona_get_client_state", serde_json::json!({}))
            .await;
        assert!(r3.is_ok());
        assert_eq!(connection_count.load(Ordering::SeqCst), 1);
        assert_eq!(client.idle_connection_count().await, 1);

        server_shutdown.notify_waiters();
        let _ = srv_handle.await;
        let _ = std::fs::remove_file(&socket_path);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_connection_pool_concurrent_calls() {
        use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
        static TEST_SOCK_SEQ_CONC: AtomicU64 = AtomicU64::new(500);

        let seq = TEST_SOCK_SEQ_CONC.fetch_add(1, Ordering::Relaxed);
        let socket_path = std::env::temp_dir().join(format!(
            "sona-test-pool-conc-{}-{}.sock",
            std::process::id(),
            seq
        ));
        let _ = std::fs::remove_file(&socket_path);
        let endpoint = socket_path.to_string_lossy().to_string();
        let connection_count = Arc::new(AtomicUsize::new(0));
        let connection_count_srv = connection_count.clone();
        let server_shutdown = Arc::new(tokio::sync::Notify::new());
        let server_shutdown_clone = server_shutdown.clone();

        let listener = tokio::net::UnixListener::bind(&socket_path).unwrap();

        let srv_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = server_shutdown_clone.notified() => break,
                    accept_res = listener.accept() => {
                        let (stream, _) = match accept_res {
                            Ok(s) => s,
                            Err(_) => break,
                        };
                        connection_count_srv.fetch_add(1, Ordering::SeqCst);
                        tokio::spawn(async move {
                            let (read_half, mut write_half) = tokio::io::split(stream);
                            let mut reader = tokio::io::BufReader::new(read_half);
                            let mut line = String::new();
                            while reader.read_line(&mut line).await.is_ok() {
                                if line.trim().is_empty() {
                                    break;
                                }
                                tokio::time::sleep(Duration::from_millis(80)).await;
                                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                                    let id = val.get("id").cloned().unwrap_or(serde_json::json!(1));
                                    let resp = serde_json::json!({
                                        "jsonrpc": "2.0",
                                        "id": id,
                                        "result": { "status": "ok" }
                                    });
                                    let _ = write_half.write_all(format!("{resp}\n").as_bytes()).await;
                                    let _ = write_half.flush().await;
                                }
                                line.clear();
                            }
                        });
                    }
                }
            }
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        let client = IpcClient::with_endpoint(endpoint);

        let c1 = client.clone();
        let c2 = client.clone();
        let (res1, res2) = tokio::join!(
            tokio::spawn(async move {
                c1.call("sona_get_client_state", serde_json::json!({}))
                    .await
            }),
            tokio::spawn(async move {
                c2.call("sona_get_client_state", serde_json::json!({}))
                    .await
            })
        );

        assert!(res1.unwrap().is_ok());
        assert!(res2.unwrap().is_ok());

        assert_eq!(connection_count.load(Ordering::SeqCst), 2);
        assert_eq!(client.idle_connection_count().await, 2);

        server_shutdown.notify_waiters();
        let _ = srv_handle.await;
        let _ = std::fs::remove_file(&socket_path);
    }
    #[tokio::test]
    async fn test_fast_offline_fail_latency() {
        let dummy_endpoint = if cfg!(windows) {
            r"\\.\pipe\sona-offline-test-fast-fail".to_string()
        } else {
            "/tmp/sona-offline-test-fast-fail.sock".to_string()
        };
        let client = IpcClient::with_endpoint(dummy_endpoint);
        let start = std::time::Instant::now();
        let res = client
            .call("sona_get_client_state", serde_json::json!({}))
            .await;
        let elapsed = start.elapsed();
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("DESKTOP_OFFLINE"));
        assert!(
            elapsed < Duration::from_millis(150),
            "Offline state should fail immediately without retry sleeps; took {elapsed:?}"
        );
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn test_stale_pooled_connection_recovery() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use tokio::net::windows::named_pipe::ServerOptions;

        let seq = TEST_PIPE_SEQ.fetch_add(1, Ordering::Relaxed);
        let pipe_name = format!(
            r"\\.\pipe\sona-mcp-test-stale-pool-{}-{}",
            std::process::id(),
            seq
        );
        let endpoint = pipe_name.clone();
        let connection_count = Arc::new(AtomicUsize::new(0));
        let connection_count_srv = connection_count.clone();
        let server_shutdown = Arc::new(tokio::sync::Notify::new());
        let server_shutdown_clone = server_shutdown.clone();
        let close_first_conn = Arc::new(tokio::sync::Notify::new());
        let close_first_conn_clone = close_first_conn.clone();

        let srv_handle = tokio::spawn(async move {
            let mut server = match ServerOptions::new()
                .first_pipe_instance(true)
                .create(&pipe_name)
            {
                Ok(s) => s,
                Err(_) => return,
            };

            loop {
                tokio::select! {
                    _ = server_shutdown_clone.notified() => break,
                    conn_res = server.connect() => {
                        if conn_res.is_err() {
                            break;
                        }
                        let conn_idx = connection_count_srv.fetch_add(1, Ordering::SeqCst);
                        let connected = server;
                        let close_notif = close_first_conn_clone.clone();

                        tokio::spawn(async move {
                            let (read_half, mut write_half) = tokio::io::split(connected);
                            let mut reader = tokio::io::BufReader::new(read_half);
                            let mut line = String::new();

                            if conn_idx == 0 {
                                if reader.read_line(&mut line).await.is_ok() && !line.trim().is_empty() {
                                    let resp = serde_json::json!({
                                        "jsonrpc": "2.0",
                                        "id": 1,
                                        "result": { "status": "ok" }
                                    });
                                    let _ = write_half.write_all(format!("{resp}\n").as_bytes()).await;
                                    let _ = write_half.flush().await;
                                }
                                close_notif.notified().await;
                                drop(reader);
                                drop(write_half);
                            } else {
                                while reader.read_line(&mut line).await.is_ok() {
                                    if line.trim().is_empty() {
                                        break;
                                    }
                                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                                        let id = val.get("id").cloned().unwrap_or(serde_json::json!(1));
                                        let resp = serde_json::json!({
                                            "jsonrpc": "2.0",
                                            "id": id,
                                            "result": { "status": "fresh_conn_ok" }
                                        });
                                        let _ = write_half.write_all(format!("{resp}\n").as_bytes()).await;
                                        let _ = write_half.flush().await;
                                    }
                                    line.clear();
                                }
                            }
                        });

                        match ServerOptions::new().create(&pipe_name) {
                            Ok(next_s) => server = next_s,
                            Err(_) => break,
                        }
                    }
                }
            }
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        let client = IpcClient::with_endpoint(endpoint);

        let r1 = client
            .call("sona_get_client_state", serde_json::json!({}))
            .await;
        assert!(r1.is_ok());
        assert_eq!(client.idle_connection_count().await, 1);

        close_first_conn.notify_waiters();
        tokio::time::sleep(Duration::from_millis(50)).await;

        let r2 = client
            .call(
                "sona_transcribe_file",
                serde_json::json!({ "file_path": "audio.wav" }),
            )
            .await;
        assert!(r2.is_ok());
        let val = r2.unwrap();
        assert_eq!(
            val.get("status").and_then(|v| v.as_str()),
            Some("fresh_conn_ok")
        );
        assert_eq!(connection_count.load(Ordering::SeqCst), 2);

        server_shutdown.notify_waiters();
        let _ = srv_handle.await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_stale_pooled_connection_recovery_unix() {
        use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
        static TEST_SOCK_SEQ_STALE_POOL: AtomicU64 = AtomicU64::new(600);

        let seq = TEST_SOCK_SEQ_STALE_POOL.fetch_add(1, Ordering::Relaxed);
        let socket_path = std::env::temp_dir().join(format!(
            "sona-test-stale-pool-{}-{}.sock",
            std::process::id(),
            seq
        ));
        let _ = std::fs::remove_file(&socket_path);
        let endpoint = socket_path.to_string_lossy().to_string();
        let connection_count = Arc::new(AtomicUsize::new(0));
        let connection_count_srv = connection_count.clone();
        let server_shutdown = Arc::new(tokio::sync::Notify::new());
        let server_shutdown_clone = server_shutdown.clone();
        let close_first_conn = Arc::new(tokio::sync::Notify::new());
        let close_first_conn_clone = close_first_conn.clone();

        let listener = tokio::net::UnixListener::bind(&socket_path).unwrap();

        let srv_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = server_shutdown_clone.notified() => break,
                    accept_res = listener.accept() => {
                        let (stream, _) = match accept_res {
                            Ok(s) => s,
                            Err(_) => break,
                        };
                        let conn_idx = connection_count_srv.fetch_add(1, Ordering::SeqCst);
                        let close_notif = close_first_conn_clone.clone();

                        tokio::spawn(async move {
                            let (read_half, mut write_half) = tokio::io::split(stream);
                            let mut reader = tokio::io::BufReader::new(read_half);
                            let mut line = String::new();

                            if conn_idx == 0 {
                                if reader.read_line(&mut line).await.is_ok() && !line.trim().is_empty() {
                                    let resp = serde_json::json!({
                                        "jsonrpc": "2.0",
                                        "id": 1,
                                        "result": { "status": "ok" }
                                    });
                                    let _ = write_half.write_all(format!("{resp}\n").as_bytes()).await;
                                    let _ = write_half.flush().await;
                                }
                                close_notif.notified().await;
                                drop(reader);
                                drop(write_half);
                            } else {
                                while reader.read_line(&mut line).await.is_ok() {
                                    if line.trim().is_empty() {
                                        break;
                                    }
                                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                                        let id = val.get("id").cloned().unwrap_or(serde_json::json!(1));
                                        let resp = serde_json::json!({
                                            "jsonrpc": "2.0",
                                            "id": id,
                                            "result": { "status": "fresh_conn_ok" }
                                        });
                                        let _ = write_half.write_all(format!("{resp}\n").as_bytes()).await;
                                        let _ = write_half.flush().await;
                                    }
                                    line.clear();
                                }
                            }
                        });
                    }
                }
            }
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        let client = IpcClient::with_endpoint(endpoint);

        let r1 = client
            .call("sona_get_client_state", serde_json::json!({}))
            .await;
        assert!(r1.is_ok());
        assert_eq!(client.idle_connection_count().await, 1);

        close_first_conn.notify_waiters();
        tokio::time::sleep(Duration::from_millis(50)).await;

        let r2 = client
            .call(
                "sona_transcribe_file",
                serde_json::json!({ "file_path": "audio.wav" }),
            )
            .await;
        assert!(r2.is_ok());
        let val = r2.unwrap();
        assert_eq!(
            val.get("status").and_then(|v| v.as_str()),
            Some("fresh_conn_ok")
        );
        assert_eq!(connection_count.load(Ordering::SeqCst), 2);

        server_shutdown.notify_waiters();
        let _ = srv_handle.await;
        let _ = std::fs::remove_file(&socket_path);
    }
}
