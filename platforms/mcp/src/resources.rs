use crate::client::IpcClient;
use crate::protocol::{ResourceContent, ResourceDefinition, ResourceTemplateDefinition};

pub fn list_resources() -> Vec<ResourceDefinition> {
    vec![
        ResourceDefinition {
            uri: "sona://client/status",
            name: "Sona Client Status",
            description: Some(
                "Current Sona desktop online status, recording state, and active project view",
            ),
            mime_type: Some("application/json"),
        },
        ResourceDefinition {
            uri: "sona://projects",
            name: "Sona Workspace Projects",
            description: Some("List of workspace projects and their configurations"),
            mime_type: Some("application/json"),
        },
        ResourceDefinition {
            uri: "sona://history/{history_id}",
            name: "Sona History Transcript",
            description: Some("Read-only transcript content for a specific history record by ID"),
            mime_type: Some("text/plain"),
        },
    ]
}

pub fn list_resource_templates() -> Vec<ResourceTemplateDefinition> {
    vec![ResourceTemplateDefinition {
        uri_template: "sona://history/{history_id}",
        name: "Sona History Transcript",
        description: Some("Read-only transcript content for a specific history record by ID"),
        mime_type: Some("text/plain"),
    }]
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
    } else if let Some(history_id) = uri.strip_prefix("sona://history/") {
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
    } else {
        Err(format!("Resource '{uri}' not found"))
    }
}
