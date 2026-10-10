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
            uri: "sona://history/{history_id}",
            name: "Sona History Transcript",
            title: Some("History Transcript"),
            description: Some("Read-only transcript content for a specific history record by ID"),
            mime_type: Some("text/plain"),
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
    ]
}

pub async fn read_resource(uri: &str, client: &IpcClient) -> Result<ResourceContent, String> {
    if uri == "sona://client/status" {
        let val = client
            .call("sona_get_client_state", serde_json::json!({}))
            .await?;
        let text = serde_json::to_string_pretty(&val).unwrap_or_else(|_| val.to_string());
        Ok(ResourceContent {
            uri: uri.to_string(),
            mime_type: Some("application/json"),
            text,
        })
    } else if uri == "sona://projects" {
        let val = client
            .call("sona_list_projects", serde_json::json!({}))
            .await?;
        let text = serde_json::to_string_pretty(&val).unwrap_or_else(|_| val.to_string());
        Ok(ResourceContent {
            uri: uri.to_string(),
            mime_type: Some("application/json"),
            text,
        })
    } else if uri == "sona://models" {
        let val = client
            .call("sona_get_model_catalog", serde_json::json!({}))
            .await?;
        let text = serde_json::to_string_pretty(&val).unwrap_or_else(|_| val.to_string());
        Ok(ResourceContent {
            uri: uri.to_string(),
            mime_type: Some("application/json"),
            text,
        })
    } else if uri == "sona://sync/status" {
        let val = client
            .call("sona_get_sync_status", serde_json::json!({}))
            .await?;
        let text = serde_json::to_string_pretty(&val).unwrap_or_else(|_| val.to_string());
        Ok(ResourceContent {
            uri: uri.to_string(),
            mime_type: Some("application/json"),
            text,
        })
    } else if uri == "sona://trash" {
        let val = client
            .call("sona_query_trash", serde_json::json!({}))
            .await?;
        let text = serde_json::to_string_pretty(&val).unwrap_or_else(|_| val.to_string());
        Ok(ResourceContent {
            uri: uri.to_string(),
            mime_type: Some("application/json"),
            text,
        })
    } else if let Some(rest) = uri.strip_prefix("sona://history/") {
        if let Some(history_id) = rest.strip_suffix("/summary") {
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
        } else if let Some(history_id) = rest.strip_suffix("/translation") {
            let val = client
                .call(
                    "sona_read_transcript",
                    serde_json::json!({ "history_id": history_id }),
                )
                .await?;
            let text = val
                .get("translation_text")
                .or_else(|| val.get("translationText"))
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string();
            Ok(ResourceContent {
                uri: uri.to_string(),
                mime_type: Some("text/plain"),
                text,
            })
        } else {
            let val = client
                .call(
                    "sona_read_transcript",
                    serde_json::json!({ "history_id": rest }),
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
