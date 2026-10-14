use crate::client::IpcClient;
use crate::protocol::{ResourceContent, ResourceDefinition, ResourceTemplateDefinition};

pub fn list_resources() -> Vec<ResourceDefinition> {
    vec![
        ResourceDefinition {
            uri: "sona://client/status",
            name: "Sona Client Status",
            title: Some("Client Status"),
            description: Some(
                "Current Sona desktop online status, recording state, and active project view",
            ),
            mime_type: Some("application/json"),
        },
        ResourceDefinition {
            uri: "sona://projects",
            name: "Sona Workspace Projects",
            title: Some("Workspace Projects"),
            description: Some("List of workspace projects and their configurations"),
            mime_type: Some("application/json"),
        },
        ResourceDefinition {
            uri: "sona://models",
            name: "Sona Model Catalog",
            title: Some("Model Catalog"),
            description: Some("Current snapshot of installed and available local models"),
            mime_type: Some("application/json"),
        },
        ResourceDefinition {
            uri: "sona://sync/status",
            name: "Sona Sync Status",
            title: Some("Sync Status"),
            description: Some("Current E2EE cloud sync configuration and operation status"),
            mime_type: Some("application/json"),
        },
        ResourceDefinition {
            uri: "sona://trash",
            name: "Sona Trash Items",
            title: Some("Trash Items"),
            description: Some("List of deleted history items currently in trash"),
            mime_type: Some("application/json"),
        },
        ResourceDefinition {
            uri: "sona://audio/devices",
            name: "Sona Audio Devices",
            title: Some("Audio Devices"),
            description: Some("Available microphone and system audio loopback capture devices"),
            mime_type: Some("application/json"),
        },
        ResourceDefinition {
            uri: "sona://sync/conflicts",
            name: "Sona Sync Conflicts",
            title: Some("Sync Conflicts"),
            description: Some("Unresolved E2EE sync conflicts requiring manual intervention"),
            mime_type: Some("application/json"),
        },
    ]
}

pub fn list_resource_templates() -> Vec<ResourceTemplateDefinition> {
    vec![
        ResourceTemplateDefinition {
            uri_template: "sona://history/{history_id}",
            name: "Sona History Transcript",
            title: Some("History Transcript"),
            description: Some("Read-only transcript content for a specific history record by ID"),
            mime_type: Some("text/plain"),
        },
        ResourceTemplateDefinition {
            uri_template: "sona://history/{history_id}/summary",
            name: "Sona History Summary",
            title: Some("History Summary"),
            description: Some("Persisted AI summary text for a specific history record by ID"),
            mime_type: Some("text/markdown"),
        },
        ResourceTemplateDefinition {
            uri_template: "sona://history/{history_id}/translation",
            name: "Sona History Translation",
            title: Some("History Translation"),
            description: Some("Read-only translation content for a specific history record by ID"),
            mime_type: Some("text/plain"),
        },
        ResourceTemplateDefinition {
            uri_template: "sona://history/{history_id}/snapshots",
            name: "Sona History Snapshots",
            title: Some("History Snapshots"),
            description: Some(
                "Version snapshot history and metadata for a specific history record by ID",
            ),
            mime_type: Some("application/json"),
        },
    ]
}

pub async fn read_resource(uri: &str, client: &IpcClient) -> Result<ResourceContent, String> {
    let clean_uri = uri.split('?').next().unwrap_or(uri).trim_end_matches('/');
    if clean_uri == "sona://client/status" {
        let val = client
            .call("sona_get_client_state", serde_json::json!({}))
            .await?;
        let text = serde_json::to_string_pretty(&val).unwrap_or_else(|_| val.to_string());
        Ok(ResourceContent {
            uri: uri.to_string(),
            mime_type: Some("application/json"),
            text,
        })
    } else if clean_uri == "sona://projects" {
        let val = client
            .call("sona_list_projects", serde_json::json!({}))
            .await?;
        let text = serde_json::to_string_pretty(&val).unwrap_or_else(|_| val.to_string());
        Ok(ResourceContent {
            uri: uri.to_string(),
            mime_type: Some("application/json"),
            text,
        })
    } else if clean_uri == "sona://models" {
        let val = client
            .call("sona_get_model_catalog", serde_json::json!({}))
            .await?;
        let text = serde_json::to_string_pretty(&val).unwrap_or_else(|_| val.to_string());
        Ok(ResourceContent {
            uri: uri.to_string(),
            mime_type: Some("application/json"),
            text,
        })
    } else if clean_uri == "sona://sync/status" {
        let val = client
            .call("sona_get_sync_status", serde_json::json!({}))
            .await?;
        let text = serde_json::to_string_pretty(&val).unwrap_or_else(|_| val.to_string());
        Ok(ResourceContent {
            uri: uri.to_string(),
            mime_type: Some("application/json"),
            text,
        })
    } else if clean_uri == "sona://trash" {
        let val = client
            .call("sona_query_trash", serde_json::json!({}))
            .await?;
        let text = serde_json::to_string_pretty(&val).unwrap_or_else(|_| val.to_string());
        Ok(ResourceContent {
            uri: uri.to_string(),
            mime_type: Some("application/json"),
            text,
        })
    } else if clean_uri == "sona://audio/devices" {
        let val = client
            .call("sona_list_audio_devices", serde_json::json!({}))
            .await?;
        let text = serde_json::to_string_pretty(&val).unwrap_or_else(|_| val.to_string());
        Ok(ResourceContent {
            uri: uri.to_string(),
            mime_type: Some("application/json"),
            text,
        })
    } else if clean_uri == "sona://sync/conflicts" {
        let val = client
            .call("sona_list_sync_conflicts", serde_json::json!({}))
            .await?;
        let text = serde_json::to_string_pretty(&val).unwrap_or_else(|_| val.to_string());
        Ok(ResourceContent {
            uri: uri.to_string(),
            mime_type: Some("application/json"),
            text,
        })
    } else if clean_uri == "sona://history" {
        Err("Invalid or empty history ID in resource URI".to_string())
    } else if let Some(rest) = clean_uri.strip_prefix("sona://history/") {
        if let Some(history_id) = rest.strip_suffix("/summary") {
            if history_id.trim().is_empty() {
                return Err("Invalid or empty history ID in resource URI".to_string());
            }
            let val = client
                .call(
                    "sona_load_summary",
                    serde_json::json!({ "history_id": history_id }),
                )
                .await?;
            let text = val
                .get("record")
                .and_then(|r| r.get("content"))
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .to_string();
            Ok(ResourceContent {
                uri: uri.to_string(),
                mime_type: Some("text/markdown"),
                text,
            })
        } else if let Some(history_id) = rest.strip_suffix("/snapshots") {
            if history_id.trim().is_empty() {
                return Err("Invalid or empty history ID in resource URI".to_string());
            }
            let val = client
                .call(
                    "sona_list_transcript_snapshots",
                    serde_json::json!({ "history_id": history_id }),
                )
                .await?;
            let text = serde_json::to_string_pretty(&val).unwrap_or_else(|_| val.to_string());
            Ok(ResourceContent {
                uri: uri.to_string(),
                mime_type: Some("application/json"),
                text,
            })
        } else if let Some(history_id) = rest.strip_suffix("/translation") {
            if history_id.trim().is_empty() {
                return Err("Invalid or empty history ID in resource URI".to_string());
            }
            let val = client
                .call(
                    "sona_read_transcript",
                    serde_json::json!({ "history_id": history_id }),
                )
                .await?;
            let text = val
                .get("translationText")
                .or_else(|| val.get("translation_text"))
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string();
            Ok(ResourceContent {
                uri: uri.to_string(),
                mime_type: Some("text/plain"),
                text,
            })
        } else {
            let history_id = rest;
            if history_id.trim().is_empty() {
                return Err("Invalid or empty history ID in resource URI".to_string());
            }
            let val = client
                .call(
                    "sona_read_transcript",
                    serde_json::json!({ "history_id": history_id }),
                )
                .await?;
            let text = val
                .get("text")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string();
            Ok(ResourceContent {
                uri: uri.to_string(),
                mime_type: Some("text/plain"),
                text,
            })
        }
    } else {
        Err(format!("Resource '{uri}' not found"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

    #[test]
    fn test_list_resources() {
        let res = list_resources();
        assert!(!res.is_empty());
        assert!(res.iter().any(|r| r.uri == "sona://models"));
        assert!(res.iter().any(|r| r.uri == "sona://client/status"));
    }

    #[test]
    fn test_list_resource_templates() {
        let templates = list_resource_templates();
        assert!(!templates.is_empty());
        assert!(
            templates
                .iter()
                .any(|t| t.uri_template == "sona://history/{history_id}")
        );
    }

    #[tokio::test]
    async fn test_read_resource_empty_history_id() {
        let client = IpcClient::with_endpoint("dummy".to_string());
        for uri in [
            "sona://history/",
            "sona://history",
            "sona://history/?cache=false",
            "sona://history//summary",
            "sona://history//snapshots",
            "sona://history//translation",
        ] {
            let res = read_resource(uri, &client).await;
            assert!(res.is_err(), "Expected error for URI {uri}");
            let err = res.unwrap_err();
            assert!(
                err.contains("Invalid or empty history ID"),
                "Expected invalid or empty history ID error for {uri}, got: {err}"
            );
        }
    }

    #[tokio::test]
    async fn test_read_resource_unknown() {
        let client = IpcClient::with_endpoint("dummy".to_string());
        let res = read_resource("sona://unknown?foo=bar", &client).await;
        assert!(res.is_err());
        assert!(
            res.unwrap_err()
                .contains("Resource 'sona://unknown?foo=bar' not found")
        );
    }

    #[tokio::test]
    async fn test_read_resource_with_queries_and_cleaning() {
        #[cfg(windows)]
        let (endpoint, shutdown_tx) = {
            use tokio::net::windows::named_pipe::ServerOptions;
            let pipe_name = format!(r"\\.\pipe\sona-mcp-test-res-{}", uuid::Uuid::new_v4());
            let srv_pipe = pipe_name.clone();
            let shutdown_tx = Arc::new(tokio::sync::Notify::new());
            let shutdown_rx = shutdown_tx.clone();

            tokio::spawn(async move {
                let mut server = match ServerOptions::new()
                    .first_pipe_instance(true)
                    .create(&srv_pipe)
                {
                    Ok(s) => s,
                    Err(_) => return,
                };
                loop {
                    tokio::select! {
                        _ = shutdown_rx.notified() => break,
                        conn = server.connect() => {
                            if conn.is_err() { break; }
                            let connected = server;
                            tokio::spawn(async move {
                                let (read_half, mut write_half) = tokio::io::split(connected);
                                let mut reader = tokio::io::BufReader::new(read_half);
                                let mut line = String::new();
                                while reader.read_line(&mut line).await.is_ok() {
                                    if line.trim().is_empty() { break; }
                                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                                        let id = val.get("id").cloned().unwrap_or(serde_json::json!(1));
                                        let method = val.get("method").and_then(|m| m.as_str()).unwrap_or("");
                                        let resp = match method {
                                            "sona_get_model_catalog" => serde_json::json!({
                                                "jsonrpc": "2.0",
                                                "id": id,
                                                "result": { "models": ["whisper-base"] }
                                            }),
                                            "sona_read_transcript" => serde_json::json!({
                                                "jsonrpc": "2.0",
                                                "id": id,
                                                "result": { "text": "transcription text", "translationText": "translated text" }
                                            }),
                                            "sona_load_summary" => serde_json::json!({
                                                "jsonrpc": "2.0",
                                                "id": id,
                                                "result": { "record": { "content": "summary content" } }
                                            }),
                                            _ => serde_json::json!({
                                                "jsonrpc": "2.0",
                                                "id": id,
                                                "error": { "code": -32601, "message": "Method not found" }
                                            }),
                                        };
                                        let _ = write_half.write_all(format!("{resp}\n").as_bytes()).await;
                                        let _ = write_half.flush().await;
                                    }
                                    line.clear();
                                }
                            });
                            match ServerOptions::new().create(&srv_pipe) {
                                Ok(next_s) => server = next_s,
                                Err(_) => break,
                            }
                        }
                    }
                }
            });
            (pipe_name, shutdown_tx)
        };

        #[cfg(unix)]
        let (endpoint, shutdown_tx) = {
            let socket_path = std::env::temp_dir()
                .join(format!("sona-mcp-test-res-{}.sock", uuid::Uuid::new_v4()));
            let _ = std::fs::remove_file(&socket_path);
            let listener = tokio::net::UnixListener::bind(&socket_path).unwrap();
            let ep = socket_path.to_string_lossy().to_string();
            let shutdown_tx = Arc::new(tokio::sync::Notify::new());
            let shutdown_rx = shutdown_tx.clone();

            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = shutdown_rx.notified() => break,
                        accept_res = listener.accept() => {
                            let (stream, _) = match accept_res {
                                Ok(s) => s,
                                Err(_) => break,
                            };
                            tokio::spawn(async move {
                                let (read_half, mut write_half) = tokio::io::split(stream);
                                let mut reader = tokio::io::BufReader::new(read_half);
                                let mut line = String::new();
                                while reader.read_line(&mut line).await.is_ok() {
                                    if line.trim().is_empty() { break; }
                                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                                        let id = val.get("id").cloned().unwrap_or(serde_json::json!(1));
                                        let method = val.get("method").and_then(|m| m.as_str()).unwrap_or("");
                                        let resp = match method {
                                            "sona_get_model_catalog" => serde_json::json!({
                                                "jsonrpc": "2.0",
                                                "id": id,
                                                "result": { "models": ["whisper-base"] }
                                            }),
                                            "sona_read_transcript" => serde_json::json!({
                                                "jsonrpc": "2.0",
                                                "id": id,
                                                "result": { "text": "transcription text", "translationText": "translated text" }
                                            }),
                                            "sona_load_summary" => serde_json::json!({
                                                "jsonrpc": "2.0",
                                                "id": id,
                                                "result": { "record": { "content": "summary content" } }
                                            }),
                                            _ => serde_json::json!({
                                                "jsonrpc": "2.0",
                                                "id": id,
                                                "error": { "code": -32601, "message": "Method not found" }
                                            }),
                                        };
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
            (ep, shutdown_tx)
        };

        let client = IpcClient::with_endpoint(endpoint);

        // Test sona://models with query parameter
        let model_res = read_resource("sona://models?cache=false", &client)
            .await
            .unwrap();
        assert_eq!(model_res.uri, "sona://models?cache=false");
        assert_eq!(model_res.mime_type, Some("application/json"));
        assert!(model_res.text.contains("whisper-base"));

        // Test sona://history/h-1 with query parameter
        let hist_res = read_resource("sona://history/h-1?format=txt", &client)
            .await
            .unwrap();
        assert_eq!(hist_res.uri, "sona://history/h-1?format=txt");
        assert_eq!(hist_res.mime_type, Some("text/plain"));
        assert_eq!(hist_res.text, "transcription text");

        // Test sona://history/h-1/summary with query parameter
        let summary_res = read_resource("sona://history/h-1/summary?cached=true", &client)
            .await
            .unwrap();
        assert_eq!(summary_res.uri, "sona://history/h-1/summary?cached=true");
        assert_eq!(summary_res.mime_type, Some("text/markdown"));
        assert_eq!(summary_res.text, "summary content");

        // Test trailing slash on static resource
        let static_slash = read_resource("sona://models/", &client).await.unwrap();
        assert_eq!(static_slash.uri, "sona://models/");
        assert!(static_slash.text.contains("whisper-base"));

        shutdown_tx.notify_waiters();
    }
}
