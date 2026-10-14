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
            | "focus_window"
    )
}

pub(crate) fn resolve_rpc_timeout(method: &str) -> Duration {
    match method {
        "sona_transcribe_file" | "transcribe_file" => Duration::from_secs(300),
        "sona_download_preset_model" | "download_preset_model" => Duration::from_secs(300),
        "sona_trigger_sync" | "trigger_sync" => Duration::from_secs(120),
        "sona_export_transcript" | "export_transcript" => Duration::from_secs(120),
        _ => Duration::from_secs(30),
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
            let timeout_duration = resolve_rpc_timeout(method);

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
                    if err.kind() == std::io::ErrorKind::ConnectionRefused && attempts < 2 =>
                {
                    attempts += 1;
                    tokio::time::sleep(Duration::from_millis(50)).await;
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
        let mut cmd = build_desktop_command(&exe_path, self.custom_endpoint.as_deref(), silent);
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

pub(crate) fn build_desktop_command(
    exe_path: &std::path::Path,
    custom_endpoint: Option<&str>,
    silent: bool,
) -> std::process::Command {
    let mut cmd = std::process::Command::new(exe_path);
    if let Some(endpoint) = custom_endpoint {
        cmd.env("SONA_IPC_ENDPOINT", endpoint);
    }
    if silent {
        cmd.arg("--silent");
    }
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::inherit());
    cmd
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
#[path = "client_tests.rs"]
mod tests;
