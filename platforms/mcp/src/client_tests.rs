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
        "focus_window",
        "sona_focus_window",
        "agent_focus_window",
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

#[test]
fn test_build_desktop_command_custom_endpoint_and_silent() {
    let exe = std::path::Path::new("dummy-sona.exe");
    let cmd = build_desktop_command(exe, Some("custom_endpoint_value"), true);
    let envs: Vec<_> = cmd
        .get_envs()
        .map(|(k, v)| {
            (
                k.to_string_lossy().to_string(),
                v.map(|s| s.to_string_lossy().to_string()),
            )
        })
        .collect();
    assert!(
        envs.iter()
            .any(|(k, v)| k == "SONA_IPC_ENDPOINT"
                && v.as_deref() == Some("custom_endpoint_value")),
        "SONA_IPC_ENDPOINT should be set to custom_endpoint_value"
    );
    let args: Vec<_> = cmd
        .get_args()
        .map(|a| a.to_string_lossy().to_string())
        .collect();
    assert!(args.contains(&"--silent".to_string()));
}

#[test]
fn test_build_desktop_command_no_custom_endpoint() {
    let exe = std::path::Path::new("dummy-sona.exe");
    let cmd = build_desktop_command(exe, None, false);
    let envs: Vec<_> = cmd
        .get_envs()
        .map(|(k, v)| {
            (
                k.to_string_lossy().to_string(),
                v.map(|s| s.to_string_lossy().to_string()),
            )
        })
        .collect();
    assert!(
        !envs.iter().any(|(k, _)| k == "SONA_IPC_ENDPOINT"),
        "SONA_IPC_ENDPOINT should not be set when custom_endpoint is None"
    );
    let args: Vec<_> = cmd
        .get_args()
        .map(|a| a.to_string_lossy().to_string())
        .collect();
    assert!(!args.contains(&"--silent".to_string()));
}

#[test]
fn test_build_desktop_command_isolates_stdio() {
    #[cfg(windows)]
    {
        let mut cmd = build_desktop_command(std::path::Path::new("cmd.exe"), None, false);
        cmd.args(["/c", "echo", "leak_check"]);
        let mut child = cmd.spawn().expect("failed to spawn cmd.exe");
        let status = child.wait().expect("failed to wait on child");
        assert!(status.success());
        assert!(child.stdout.is_none());
        assert!(child.stdin.is_none());
    }
    #[cfg(not(windows))]
    {
        let mut cmd = build_desktop_command(std::path::Path::new("true"), None, false);
        let mut child = cmd.spawn().expect("failed to spawn true");
        let status = child.wait().expect("failed to wait on child");
        assert!(status.success());
        assert!(child.stdout.is_none());
        assert!(child.stdin.is_none());
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

#[test]
fn test_resolve_rpc_timeout() {
    assert_eq!(
        resolve_rpc_timeout("sona_transcribe_file"),
        Duration::from_secs(300)
    );
    assert_eq!(
        resolve_rpc_timeout("transcribe_file"),
        Duration::from_secs(300)
    );
    assert_eq!(
        resolve_rpc_timeout("sona_download_preset_model"),
        Duration::from_secs(300)
    );
    assert_eq!(
        resolve_rpc_timeout("download_preset_model"),
        Duration::from_secs(300)
    );
    assert_eq!(
        resolve_rpc_timeout("sona_trigger_sync"),
        Duration::from_secs(120)
    );
    assert_eq!(
        resolve_rpc_timeout("trigger_sync"),
        Duration::from_secs(120)
    );
    assert_eq!(
        resolve_rpc_timeout("sona_export_transcript"),
        Duration::from_secs(120)
    );
    assert_eq!(
        resolve_rpc_timeout("export_transcript"),
        Duration::from_secs(120)
    );
    assert_eq!(
        resolve_rpc_timeout("sona_get_client_state"),
        Duration::from_secs(30)
    );
    assert_eq!(
        resolve_rpc_timeout("unknown_rpc_method"),
        Duration::from_secs(30)
    );
}

#[tokio::test]
async fn test_mcp_transcribe_file_async_stream_and_read_transcript() {
    let (client, server) = tokio::io::duplex(4096);

    tokio::spawn(async move {
        let (srv_read, mut srv_write) = tokio::io::split(server);
        let mut reader = tokio::io::BufReader::new(srv_read);
        let mut line = String::new();

        // 1. Handle transcribe_file
        reader.read_line(&mut line).await.unwrap();
        let req1: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(req1["method"], "sona_transcribe_file");
        let params = &req1["params"];
        assert_eq!(params["file_path"], "https://example.com/audio.mp3");

        let resp1 = serde_json::json!({
            "jsonrpc": "2.0",
            "id": req1["id"],
            "result": {
                "historyId": "hist-mcp-1",
                "taskId": "task-mcp-1",
                "status": "started",
                "isStream": true,
                "message": "Stream transcription task started in background"
            }
        });
        srv_write
            .write_all(format!("{}\n", serde_json::to_string(&resp1).unwrap()).as_bytes())
            .await
            .unwrap();
        srv_write.flush().await.unwrap();

        // 2. Handle read_transcript with limit: 1 and since_segment_id
        line.clear();
        reader.read_line(&mut line).await.unwrap();
        let req2: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(req2["method"], "sona_read_transcript");
        let params2 = &req2["params"];
        assert_eq!(params2["history_id"], "hist-mcp-1");
        assert_eq!(params2["limit"], 1);
        assert_eq!(params2["since_segment_id"], "seg-1");

        let resp2 = serde_json::json!({
            "jsonrpc": "2.0",
            "id": req2["id"],
            "result": {
                "historyId": "hist-mcp-1",
                "taskStatus": "running",
                "segments": [
                    { "id": "seg-2", "start": 2.0, "end": 4.0, "text": "next segment", "isFinal": true }
                ],
                "text": "next segment",
                "totalSegmentsInHistory": 5,
                "returnedSegments": 1,
                "hasMore": true
            }
        });
        srv_write
            .write_all(format!("{}\n", serde_json::to_string(&resp2).unwrap()).as_bytes())
            .await
            .unwrap();
        srv_write.flush().await.unwrap();
    });

    let (read_half, mut write_half) = tokio::io::split(client);
    let mut buf_reader = tokio::io::BufReader::new(read_half);

    let start_instant = std::time::Instant::now();
    let res1 = IpcClient::send_receive_io(
        &mut buf_reader,
        &mut write_half,
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"sona_transcribe_file\",\"params\":{\"file_path\":\"https://example.com/audio.mp3\"}}\n",
        1,
        Duration::from_secs(300),
    )
    .await
    .unwrap();
    let elapsed = start_instant.elapsed();
    assert!(
        elapsed < Duration::from_millis(100),
        "Latency must be < 100ms, was {elapsed:?}"
    );
    assert_eq!(res1["status"], "started");
    assert_eq!(res1["isStream"], true);
    assert_eq!(res1["taskId"], "task-mcp-1");
    assert_eq!(res1["historyId"], "hist-mcp-1");

    let res2 = IpcClient::send_receive_io(
        &mut buf_reader,
        &mut write_half,
        "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"sona_read_transcript\",\"params\":{\"history_id\":\"hist-mcp-1\",\"limit\":1,\"since_segment_id\":\"seg-1\"}}\n",
        2,
        Duration::from_secs(30),
    )
    .await
    .unwrap();
    assert_eq!(res2["taskStatus"], "running");
    assert_eq!(res2["returnedSegments"], 1);
    assert_eq!(res2["totalSegmentsInHistory"], 5);
    assert_eq!(res2["hasMore"], true);
    assert_eq!(res2["segments"][0]["id"], "seg-2");
}
