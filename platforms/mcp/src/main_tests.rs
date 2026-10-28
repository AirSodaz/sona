use super::*;

fn offline_test_client() -> IpcClient {
    #[cfg(windows)]
    {
        IpcClient::with_endpoint(r"\\.\pipe\sona-offline-test-dummy".to_string())
    }
    #[cfg(not(windows))]
    {
        IpcClient::with_endpoint("/tmp/sona-offline-test-dummy.sock".to_string())
    }
}

#[tokio::test]
async fn test_handle_server_discover() {
    let client = offline_test_client();
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(1)),
        method: "server/discover".to_string(),
        params: Some(serde_json::json!({
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                "io.modelcontextprotocol/clientCapabilities": {},
                "io.modelcontextprotocol/clientInfo": {
                    "name": "TestClient",
                    "version": "1.0.0"
                }
            }
        })),
    };

    let initialized = AtomicBool::new(false);
    let resp = handle_request(req, &client, &initialized).await.unwrap();
    assert!(initialized.load(Ordering::SeqCst));
    assert_eq!(resp.id, serde_json::json!(1));
    assert!(resp.error.is_none());
    let result = resp.result.unwrap();
    assert_eq!(
        result.get("resultType").and_then(|r| r.as_str()),
        Some("complete")
    );
    let versions = result
        .get("supportedVersions")
        .and_then(|v| v.as_array())
        .unwrap();
    assert!(versions.iter().any(|v| v.as_str() == Some("2026-07-28")));
    assert!(result.get("capabilities").is_some());
    assert_eq!(result.get("ttlMs").and_then(|t| t.as_i64()), Some(3600000));
    assert_eq!(
        result.get("cacheScope").and_then(|s| s.as_str()),
        Some("public")
    );
    let meta = result.get("_meta").unwrap();
    assert_eq!(
        meta.get("io.modelcontextprotocol/serverInfo")
            .and_then(|s| s.get("name"))
            .and_then(|n| n.as_str()),
        Some("sona-mcp")
    );
}

#[tokio::test]
async fn test_handle_server_discover_missing_meta() {
    let client = offline_test_client();
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(1)),
        method: "server/discover".to_string(),
        params: None,
    };

    let initialized = AtomicBool::new(false);
    let resp = handle_request(req, &client, &initialized).await.unwrap();
    assert!(!initialized.load(Ordering::SeqCst));
    assert_eq!(resp.id, serde_json::json!(1));
    assert!(resp.error.is_some());
    assert_eq!(resp.error.unwrap().code, protocol::INVALID_PARAMS);
}

#[tokio::test]
async fn test_handle_unsupported_protocol_version() {
    let client = offline_test_client();
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(2)),
        method: "tools/list".to_string(),
        params: Some(serde_json::json!({
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": "2023-01-01",
                "io.modelcontextprotocol/clientCapabilities": {}
            }
        })),
    };

    let initialized = AtomicBool::new(true);
    let resp = handle_request(req, &client, &initialized).await.unwrap();
    assert_eq!(resp.id, serde_json::json!(2));
    assert!(resp.error.is_some());
    let err = resp.error.unwrap();
    assert_eq!(err.code, protocol::UNSUPPORTED_PROTOCOL_VERSION);
    assert_eq!(err.message, "Unsupported protocol version");
    let data = err.data.unwrap();
    let supported = data.get("supported").and_then(|s| s.as_array()).unwrap();
    assert!(supported.iter().any(|v| v.as_str() == Some("2026-07-28")));
    assert!(supported.iter().any(|v| v.as_str() == Some("2024-11-05")));
    assert_eq!(
        data.get("requested").and_then(|r| r.as_str()),
        Some("2023-01-01")
    );
}

#[tokio::test]
async fn test_handle_supported_protocol_version_2024_11_05() {
    let client = offline_test_client();
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(200)),
        method: "tools/list".to_string(),
        params: Some(serde_json::json!({
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": "2024-11-05",
                "io.modelcontextprotocol/clientCapabilities": {}
            }
        })),
    };

    let initialized = AtomicBool::new(true);
    let resp = handle_request(req, &client, &initialized).await.unwrap();
    assert_eq!(resp.id, serde_json::json!(200));
    assert!(resp.error.is_none());
    let result = resp.result.unwrap();
    assert_eq!(
        result.get("resultType").and_then(|r| r.as_str()),
        Some("complete")
    );
}

#[tokio::test]
async fn test_handle_missing_client_capabilities() {
    let client = offline_test_client();
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(3)),
        method: "tools/list".to_string(),
        params: Some(serde_json::json!({
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": "2026-07-28"
            }
        })),
    };

    let initialized = AtomicBool::new(true);
    let resp = handle_request(req, &client, &initialized).await.unwrap();
    assert_eq!(resp.id, serde_json::json!(3));
    assert!(resp.error.is_some());
    assert_eq!(resp.error.unwrap().code, protocol::INVALID_PARAMS);
}

#[tokio::test]
async fn test_handle_standard_mcp_meta_progress_token() {
    let client = offline_test_client();
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(1)),
        method: "tools/list".to_string(),
        params: Some(serde_json::json!({
            "_meta": {
                "progressToken": 123
            }
        })),
    };

    let initialized = AtomicBool::new(true);
    let resp = handle_request(req, &client, &initialized).await.unwrap();
    assert_eq!(resp.id, serde_json::json!(1));
    assert!(resp.error.is_none());
    assert!(resp.result.is_some());
}

#[tokio::test]
async fn test_handle_initialize() {
    let client = offline_test_client();
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(10)),
        method: "initialize".to_string(),
        params: Some(serde_json::json!({})),
    };

    let initialized = AtomicBool::new(false);
    let resp = handle_request(req, &client, &initialized).await.unwrap();
    assert!(initialized.load(Ordering::SeqCst));
    assert_eq!(resp.id, serde_json::json!(10));
    assert!(resp.error.is_none());
    let result = resp.result.unwrap();
    assert_eq!(
        result.get("protocolVersion").and_then(|v| v.as_str()),
        Some("2026-07-28")
    );
    assert_eq!(
        result
            .get("serverInfo")
            .and_then(|s| s.get("name"))
            .and_then(|n| n.as_str()),
        Some("sona-mcp")
    );
}

#[tokio::test]
async fn test_handle_initialize_negotiates_version() {
    let client = offline_test_client();
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(11)),
        method: "initialize".to_string(),
        params: Some(serde_json::json!({
            "protocolVersion": "2024-11-05"
        })),
    };

    let initialized = AtomicBool::new(false);
    let resp = handle_request(req, &client, &initialized).await.unwrap();
    assert!(initialized.load(Ordering::SeqCst));
    assert_eq!(resp.id, serde_json::json!(11));
    assert!(resp.error.is_none());
    let result = resp.result.unwrap();
    assert_eq!(
        result.get("protocolVersion").and_then(|v| v.as_str()),
        Some("2024-11-05")
    );
}

#[tokio::test]
async fn test_handle_tools_list() {
    let client = offline_test_client();
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(20)),
        method: "tools/list".to_string(),
        params: Some(serde_json::json!({
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                "io.modelcontextprotocol/clientCapabilities": {}
            }
        })),
    };

    let initialized = AtomicBool::new(true);
    let resp = handle_request(req, &client, &initialized).await.unwrap();
    assert_eq!(resp.id, serde_json::json!(20));
    let result = resp.result.unwrap();
    assert_eq!(
        result.get("resultType").and_then(|t| t.as_str()),
        Some("complete")
    );
    assert_eq!(result.get("ttlMs").and_then(|t| t.as_i64()), Some(300000));
    assert_eq!(
        result.get("cacheScope").and_then(|s| s.as_str()),
        Some("public")
    );
    let tools = result.get("tools").and_then(|t| t.as_array()).unwrap();
    assert_eq!(tools.len(), 40);
    assert_eq!(
        tools[0].get("title").and_then(|t| t.as_str()),
        Some("Get Client State")
    );
}

#[tokio::test]
async fn test_handle_resource_templates_list() {
    let client = offline_test_client();
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(301)),
        method: "resources/templates/list".to_string(),
        params: None,
    };

    let initialized = AtomicBool::new(true);
    let resp = handle_request(req, &client, &initialized).await.unwrap();
    assert_eq!(resp.id, serde_json::json!(301));
    let result = resp.result.unwrap();
    let templates = result
        .get("resourceTemplates")
        .and_then(|r| r.as_array())
        .unwrap();
    assert_eq!(templates.len(), 4);
    let uris: Vec<&str> = templates
        .iter()
        .filter_map(|t| t.get("uriTemplate").and_then(|u| u.as_str()))
        .collect();
    assert!(uris.contains(&"sona://history/{history_id}/translation"));
}

#[tokio::test]
async fn test_handle_resources_list() {
    let client = offline_test_client();
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(30)),
        method: "resources/list".to_string(),
        params: None,
    };

    let initialized = AtomicBool::new(true);
    let resp = handle_request(req, &client, &initialized).await.unwrap();
    assert_eq!(resp.id, serde_json::json!(30));
    let result = resp.result.unwrap();
    assert_eq!(
        result.get("resultType").and_then(|t| t.as_str()),
        Some("complete")
    );
    assert_eq!(result.get("ttlMs").and_then(|t| t.as_i64()), Some(300000));
    let resources = result.get("resources").and_then(|r| r.as_array()).unwrap();
    assert_eq!(resources.len(), 7);
    assert!(!resources.iter().any(|r| {
        r.get("uri")
            .and_then(|u| u.as_str())
            .is_some_and(|u| u.contains("{history_id}"))
    }));
    assert_eq!(
        resources[0].get("title").and_then(|t| t.as_str()),
        Some("Client Status")
    );
}

#[tokio::test]
async fn test_handle_resources_read_not_found() {
    let client = offline_test_client();
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(31)),
        method: "resources/read".to_string(),
        params: Some(serde_json::json!({
            "uri": "sona://unknown-resource"
        })),
    };

    let initialized = AtomicBool::new(true);
    let resp = handle_request(req, &client, &initialized).await.unwrap();
    assert_eq!(resp.id, serde_json::json!(31));
    assert!(resp.error.is_some());
    assert_eq!(resp.error.unwrap().code, protocol::INVALID_PARAMS);
}

#[tokio::test]
async fn test_handle_prompts_list() {
    let client = offline_test_client();
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(40)),
        method: "prompts/list".to_string(),
        params: None,
    };

    let initialized = AtomicBool::new(true);
    let resp = handle_request(req, &client, &initialized).await.unwrap();
    assert_eq!(resp.id, serde_json::json!(40));
    let result = resp.result.unwrap();
    assert_eq!(
        result.get("resultType").and_then(|t| t.as_str()),
        Some("complete")
    );
    assert_eq!(result.get("ttlMs").and_then(|t| t.as_i64()), Some(600000));
    let prompts = result.get("prompts").and_then(|p| p.as_array()).unwrap();
    assert_eq!(prompts.len(), 2);
    assert_eq!(
        prompts[0].get("title").and_then(|t| t.as_str()),
        Some("Summarize Meeting")
    );
}

#[tokio::test]
async fn test_handle_prompts_get_not_found() {
    let client = offline_test_client();
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(41)),
        method: "prompts/get".to_string(),
        params: Some(serde_json::json!({
            "name": "nonexistent_prompt",
            "arguments": { "history_id": "hist-1" }
        })),
    };

    let initialized = AtomicBool::new(true);
    let resp = handle_request(req, &client, &initialized).await.unwrap();
    assert_eq!(resp.id, serde_json::json!(41));
    assert!(resp.error.is_some());
    assert_eq!(resp.error.unwrap().code, protocol::INVALID_PARAMS);
}

#[tokio::test]
async fn test_handle_offline_get_client_state() {
    let client = offline_test_client();
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(50)),
        method: "tools/call".to_string(),
        params: Some(serde_json::json!({
            "name": "sona_get_client_state",
            "arguments": {}
        })),
    };

    let initialized = AtomicBool::new(true);
    let resp = handle_request(req, &client, &initialized).await.unwrap();
    assert_eq!(resp.id, serde_json::json!(50));
    let result = resp.result.unwrap();
    assert_eq!(
        result.get("resultType").and_then(|t| t.as_str()),
        Some("complete")
    );
    let content = result.get("content").and_then(|c| c.as_array()).unwrap();
    let text = content[0].get("text").and_then(|t| t.as_str()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed.get("online").and_then(|o| o.as_bool()), Some(false));
}

#[tokio::test]
async fn test_handle_notification_no_response() {
    let client = offline_test_client();
    let notification = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: None,
        method: "notifications/initialized".to_string(),
        params: None,
    };
    let initialized = AtomicBool::new(false);
    let resp = handle_request(notification, &client, &initialized).await;
    assert!(resp.is_none());
    assert!(initialized.load(Ordering::SeqCst));
}

#[tokio::test]
async fn test_handle_invalid_jsonrpc_version() {
    let client = offline_test_client();
    let invalid = JsonRpcRequest {
        jsonrpc: "1.0".to_string(),
        id: Some(serde_json::json!(99)),
        method: "ping".to_string(),
        params: None,
    };
    let initialized = AtomicBool::new(true);
    let resp = handle_request(invalid, &client, &initialized)
        .await
        .unwrap();
    assert_eq!(resp.id, serde_json::json!(99));
    assert!(resp.error.is_some());
    assert_eq!(resp.error.unwrap().code, protocol::INVALID_REQUEST);
}

#[tokio::test]
async fn test_uninitialized_request_rejected() {
    let client = offline_test_client();
    let initialized = AtomicBool::new(false);

    // Calling tools/list before initialize must return SERVER_NOT_INITIALIZED (-32002)
    let list_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(1)),
        method: "tools/list".to_string(),
        params: None,
    };
    let resp = handle_request(list_req, &client, &initialized)
        .await
        .unwrap();
    assert_eq!(resp.id, serde_json::json!(1));
    assert!(resp.error.is_some());
    let err = resp.error.unwrap();
    assert_eq!(err.code, protocol::SERVER_NOT_INITIALIZED);

    // Calling tools/call before initialize must return SERVER_NOT_INITIALIZED (-32002)
    let call_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(2)),
        method: "tools/call".to_string(),
        params: Some(serde_json::json!({
            "name": "sona_get_client_state",
            "arguments": {}
        })),
    };
    let resp = handle_request(call_req, &client, &initialized)
        .await
        .unwrap();
    assert_eq!(resp.id, serde_json::json!(2));
    assert!(resp.error.is_some());
    assert_eq!(resp.error.unwrap().code, protocol::SERVER_NOT_INITIALIZED);

    // Ping is allowed even when uninitialized
    let ping_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(3)),
        method: "ping".to_string(),
        params: None,
    };
    let resp = handle_request(ping_req, &client, &initialized)
        .await
        .unwrap();
    assert_eq!(resp.id, serde_json::json!(3));
    assert!(resp.error.is_none());

    // Initializing allows subsequent tool calls
    let init_notif = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: None,
        method: "notifications/initialized".to_string(),
        params: None,
    };
    assert!(
        handle_request(init_notif, &client, &initialized)
            .await
            .is_none()
    );
    assert!(initialized.load(Ordering::SeqCst));

    let list_req_after = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(4)),
        method: "tools/list".to_string(),
        params: None,
    };
    let resp = handle_request(list_req_after, &client, &initialized)
        .await
        .unwrap();
    assert_eq!(resp.id, serde_json::json!(4));
    assert!(resp.error.is_none());
    assert!(resp.result.is_some());
}

#[tokio::test]
async fn test_notification_cancellation_aborts_active_task() {
    use std::time::Duration;

    let active_tasks: ActiveTaskRegistry = Arc::new(tokio::sync::Mutex::new(HashMap::new()));

    let task_started = Arc::new(tokio::sync::Notify::new());
    let task_started_clone = task_started.clone();

    let req_id = serde_json::json!(42);
    let id_key = req_id.clone();
    let active_tasks_clone = active_tasks.clone();

    let handle = tokio::spawn(async move {
        task_started_clone.notify_waiters();
        tokio::time::sleep(Duration::from_secs(30)).await;
        active_tasks_clone.lock().await.remove(&id_key);
    });

    active_tasks.lock().await.insert(
        req_id.clone(),
        ActiveTaskInfo {
            abort_handle: handle.abort_handle(),
            batch_instance_id: None,
        },
    );

    task_started.notified().await;

    let cancelled = cancel_active_task(&active_tasks, &req_id, None).await;
    assert!(cancelled.is_some(), "Task should be found and cancelled");

    assert!(active_tasks.lock().await.get(&req_id).is_none());

    let join_res = handle.await;
    assert!(join_res.is_err());
    assert!(join_res.unwrap_err().is_cancelled());

    let second_cancel = cancel_active_task(&active_tasks, &req_id, None).await;
    assert!(second_cancel.is_none());
}

#[tokio::test]
async fn test_notification_cancelled_parsing() {
    use std::time::Duration;

    let active_tasks: ActiveTaskRegistry = Arc::new(tokio::sync::Mutex::new(HashMap::new()));

    let req_id = serde_json::json!("task-100");
    let handle = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(30)).await;
    });
    active_tasks.lock().await.insert(
        req_id.clone(),
        ActiveTaskInfo {
            abort_handle: handle.abort_handle(),
            batch_instance_id: Some("batch-100".to_string()),
        },
    );

    let notification_json = r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":"task-100","reason":"user requested"}}"#;
    let req: JsonRpcRequest = serde_json::from_str(notification_json).unwrap();
    assert!(req.id.is_none());

    let target_id = req
        .params
        .as_ref()
        .and_then(|p| p.get("requestId").or_else(|| p.get("id")).cloned());
    assert_eq!(target_id, Some(req_id.clone()));

    let cancelled = cancel_active_task(&active_tasks, &target_id.unwrap(), None).await;
    assert!(cancelled.is_some());
    let info = cancelled.unwrap();
    assert_eq!(info.batch_instance_id.as_deref(), Some("batch-100"));
    assert!(handle.await.unwrap_err().is_cancelled());
}

#[tokio::test]
async fn test_cancellation_jsonrpc_error_response() {
    let resp = JsonRpcResponse::error(
        serde_json::json!(101),
        protocol::REQUEST_CANCELLED,
        "Request was cancelled by client",
    );
    assert_eq!(resp.id, serde_json::json!(101));
    assert!(resp.result.is_none());
    let err = resp.error.unwrap();
    assert_eq!(err.code, -32800);
    assert_eq!(err.message, "Request was cancelled by client");
}

#[test]
fn test_batch_instance_id_injection() {
    for tool_name in [
        "sona_transcribe_file",
        "transcribe_file",
        "agent_transcribe_file",
    ] {
        let mut req: JsonRpcRequest = serde_json::from_str(&format!(
            r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"{tool_name}","arguments":{{"file_path":"test.wav"}}}}}}"#
        ))
        .unwrap();

        let batch_instance_id = maybe_inject_transcribe_instance_id(&mut req);

        assert!(batch_instance_id.is_some(), "Should inject for {tool_name}");
        let id = batch_instance_id.unwrap();
        assert!(id.starts_with("mcp-transcribe-"));
        let injected = req.params.unwrap()["arguments"]["instance_id"]
            .as_str()
            .unwrap()
            .to_string();
        assert_eq!(injected, id);
    }
    // Verify input alias normalizes to file_path when file_path is missing, null, or empty
    for payload in [
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"sona_transcribe_file","arguments":{"input":"https://example.com/stream.mp3"}}}"#,
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"sona_transcribe_file","arguments":{"file_path":null,"input":"https://example.com/stream.mp3"}}}"#,
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"sona_transcribe_file","arguments":{"file_path":"","input":"https://example.com/stream.mp3"}}}"#,
    ] {
        let mut req: JsonRpcRequest = serde_json::from_str(payload).unwrap();
        let id = maybe_inject_transcribe_instance_id(&mut req);
        assert!(id.is_some());
        let args = &req.params.unwrap()["arguments"];
        assert_eq!(
            args["file_path"].as_str(),
            Some("https://example.com/stream.mp3")
        );
    }

    // Verify non-transcribe tool does not get injected
    let mut non_transcribe: JsonRpcRequest = serde_json::from_str(
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"query_history","arguments":{}}}"#,
    )
    .unwrap();
    assert!(maybe_inject_transcribe_instance_id(&mut non_transcribe).is_none());
}
