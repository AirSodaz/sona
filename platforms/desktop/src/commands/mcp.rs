use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::platform::agent_control::{
    AgentControlFacade, AgentControlServerController, McpActivityLogEntry, McpToolCatalogItem,
    get_tools_catalog,
};

pub use crate::platform::agent_control::get_ipc_endpoint;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpStatus {
    pub enabled: bool,
    pub running: bool,
    pub endpoint: String,
    pub active_connections: usize,
    pub binary_path: Option<String>,
    pub binary_exists: bool,
    pub permission_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpClientGuide {
    pub id: String,
    pub name: String,
    pub config_path: String,
    pub config_content: String,
    pub format: String, // "json" | "bash"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpClientConfigMap {
    pub binary_path: String,
    pub binary_exists: bool,
    pub endpoint: String,
    pub guides: Vec<McpClientGuide>,
}

pub fn resolve_sona_mcp_binary_path() -> (Option<String>, bool) {
    let binary_name = if cfg!(windows) {
        "sona-mcp.exe"
    } else {
        "sona-mcp"
    };

    if let Some(parent) = std::env::current_exe()
        .ok()
        .as_deref()
        .and_then(std::path::Path::parent)
    {
        let candidate = parent.join(binary_name);
        if candidate.exists() {
            return (Some(candidate.to_string_lossy().to_string()), true);
        }
        for rel in &[
            "../debug",
            "../release",
            "../../target/debug",
            "../../target/release",
        ] {
            let candidate = parent.join(rel).join(binary_name);
            if let Ok(canon) = candidate.canonicalize() {
                return (Some(canon.to_string_lossy().to_string()), true);
            }
        }
    }

    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(binary_name);
            if candidate.exists() {
                return (Some(candidate.to_string_lossy().to_string()), true);
            }
        }
    }

    let fallback = if let Ok(current_exe) = std::env::current_exe() {
        current_exe
            .parent()
            .map(|p| p.join(binary_name).to_string_lossy().to_string())
    } else {
        None
    };

    (fallback, false)
}

#[tauri::command]
pub async fn mcp_get_status(
    facade: State<'_, AgentControlFacade>,
    controller: State<'_, AgentControlServerController>,
) -> Result<McpStatus, String> {
    let config = facade.load_app_config();
    let enabled = config.mcp_enabled.unwrap_or(true);
    let permission_mode = config
        .mcp_permission_mode
        .unwrap_or_else(|| "full".to_string());
    let (binary_path, binary_exists) = resolve_sona_mcp_binary_path();

    Ok(McpStatus {
        enabled,
        running: controller.is_running(),
        endpoint: get_ipc_endpoint(),
        active_connections: controller.tracker().active_count(),
        binary_path,
        binary_exists,
        permission_mode,
    })
}

#[tauri::command]
pub async fn mcp_set_enabled(
    app: AppHandle,
    facade: State<'_, AgentControlFacade>,
    controller: State<'_, AgentControlServerController>,
    enabled: bool,
) -> Result<(), String> {
    facade
        .services()
        .config
        .set_setting("mcpEnabled", &serde_json::Value::Bool(enabled))?;

    if enabled {
        controller.start(Some(app.clone()), facade.inner().clone());
    } else {
        controller.stop(Some(&app));
    }

    let _ = app.emit("mcp-status-changed", ());
    Ok(())
}

#[tauri::command]
pub async fn mcp_disconnect_all(
    app: AppHandle,
    controller: State<'_, AgentControlServerController>,
) -> Result<(), String> {
    controller.disconnect_all(Some(&app));
    let _ = app.emit("mcp-status-changed", ());
    Ok(())
}

#[tauri::command]
pub async fn mcp_get_activity_logs(
    controller: State<'_, AgentControlServerController>,
) -> Result<Vec<McpActivityLogEntry>, String> {
    Ok(controller.get_activity_logs().await)
}

#[tauri::command]
pub async fn mcp_clear_activity_logs(
    controller: State<'_, AgentControlServerController>,
) -> Result<(), String> {
    controller.clear_activity_logs().await;
    Ok(())
}

#[tauri::command]
pub async fn mcp_get_tools_catalog(
    facade: State<'_, AgentControlFacade>,
) -> Result<Vec<McpToolCatalogItem>, String> {
    let config = facade.load_app_config();
    Ok(get_tools_catalog(&config))
}

#[tauri::command]
pub async fn mcp_get_client_configs() -> Result<McpClientConfigMap, String> {
    let (binary_path_opt, binary_exists) = resolve_sona_mcp_binary_path();
    let binary_path = binary_path_opt.unwrap_or_else(|| {
        if cfg!(windows) {
            r"C:\Program Files\Sona\sona-mcp.exe".to_string()
        } else {
            "/usr/local/bin/sona-mcp".to_string()
        }
    });

    let endpoint = get_ipc_endpoint();

    let claude_desktop_config_path = if cfg!(windows) {
        r"%APPDATA%\Claude\claude_desktop_config.json".to_string()
    } else if cfg!(target_os = "macos") {
        "~/Library/Application Support/Claude/claude_desktop_config.json".to_string()
    } else {
        "~/.config/Claude/claude_desktop_config.json".to_string()
    };

    let cursor_config_path = if cfg!(windows) {
        r"%USERPROFILE%\.cursor\mcp.json".to_string()
    } else {
        "~/.cursor/mcp.json".to_string()
    };

    let windsurf_config_path = if cfg!(windows) {
        r"%USERPROFILE%\.codeium\windsurf\mcp_config.json".to_string()
    } else {
        "~/.codeium/windsurf/mcp_config.json".to_string()
    };

    let stdio_snippet = serde_json::json!({
        "mcpServers": {
            "sona": {
                "command": &binary_path,
                "args": []
            }
        }
    });
    let stdio_json_str = serde_json::to_string_pretty(&stdio_snippet).unwrap_or_default();

    let claude_code_cli = format!("claude mcp add sona \"{binary_path}\"");

    let guides = vec![
        McpClientGuide {
            id: "claude-desktop".to_string(),
            name: "Claude Desktop".to_string(),
            config_path: claude_desktop_config_path,
            config_content: stdio_json_str.clone(),
            format: "json".to_string(),
        },
        McpClientGuide {
            id: "cursor".to_string(),
            name: "Cursor".to_string(),
            config_path: cursor_config_path,
            config_content: stdio_json_str.clone(),
            format: "json".to_string(),
        },
        McpClientGuide {
            id: "windsurf".to_string(),
            name: "Windsurf".to_string(),
            config_path: windsurf_config_path,
            config_content: stdio_json_str.clone(),
            format: "json".to_string(),
        },
        McpClientGuide {
            id: "claude-code".to_string(),
            name: "Claude Code CLI".to_string(),
            config_path: "Terminal CLI Command".to_string(),
            config_content: claude_code_cli,
            format: "bash".to_string(),
        },
        McpClientGuide {
            id: "generic".to_string(),
            name: "Generic / Stdio".to_string(),
            config_path: "Custom Agent Client MCP Config".to_string(),
            config_content: stdio_json_str,
            format: "json".to_string(),
        },
    ];

    Ok(McpClientConfigMap {
        binary_path,
        binary_exists,
        endpoint,
        guides,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipc_endpoint_format() {
        let endpoint = get_ipc_endpoint();
        if cfg!(windows) {
            assert!(endpoint.contains(r"pipe\sona-agent-ipc"));
        } else if cfg!(unix) {
            assert!(endpoint.contains("sock"));
        }
    }

    #[tokio::test]
    async fn test_mcp_get_client_configs_structure() {
        let configs = mcp_get_client_configs().await.unwrap();
        assert!(!configs.binary_path.is_empty());
        assert_eq!(configs.guides.len(), 5);
        assert!(configs.guides.iter().any(|g| g.id == "claude-desktop"));
        assert!(configs.guides.iter().any(|g| g.id == "cursor"));
        assert!(configs.guides.iter().any(|g| g.id == "windsurf"));
        assert!(configs.guides.iter().any(|g| g.id == "claude-code"));
        assert!(configs.guides.iter().any(|g| g.id == "generic"));

        let claude_desktop = configs
            .guides
            .iter()
            .find(|g| g.id == "claude-desktop")
            .unwrap();
        assert!(claude_desktop.config_content.contains("mcpServers"));
        assert!(claude_desktop.config_content.contains("sona"));
    }
}
