use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::platform::agent_control::{
    AgentControlFacade, AgentControlServerController, McpActivityLogEntry, McpToolCatalogItem,
    get_tools_catalog,
};

pub use crate::platform::agent_control::{get_ipc_endpoint, get_ipc_endpoint_for_channel};

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
    resolve_sona_mcp_binary_path_with_env(|k| std::env::var(k).ok())
}

pub(crate) fn resolve_sona_mcp_binary_path_with_env<F: Fn(&str) -> Option<String>>(
    env_lookup: F,
) -> (Option<String>, bool) {
    let binary_name = if cfg!(windows) {
        "sona-mcp.exe"
    } else {
        "sona-mcp"
    };

    if let Some(env_path) = env_lookup("SONA_MCP_PATH") {
        let p = std::path::PathBuf::from(env_path);
        if p.is_file() {
            return (Some(p.to_string_lossy().to_string()), true);
        }
        if p.is_dir() {
            let candidate = p.join(binary_name);
            if candidate.is_file() {
                return (Some(candidate.to_string_lossy().to_string()), true);
            }
        }
    }
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

    if let Some(path_var) = env_lookup("PATH") {
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
pub async fn mcp_get_client_configs(app: AppHandle) -> Result<McpClientConfigMap, String> {
    let is_nightly =
        crate::commands::system::detect_app_channel(&app) == sona_shim::registry::Channel::Nightly;
    build_mcp_client_configs(is_nightly)
}

pub fn build_mcp_client_configs(is_nightly: bool) -> Result<McpClientConfigMap, String> {
    let (binary_path_opt, binary_exists) = resolve_sona_mcp_binary_path();
    let binary_path = binary_path_opt.unwrap_or_else(|| {
        if cfg!(windows) {
            r"C:\Program Files\Sona\sona-mcp.exe".to_string()
        } else {
            "/usr/local/bin/sona-mcp".to_string()
        }
    });

    let endpoint = get_ipc_endpoint_for_channel(is_nightly);

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

    let codex_config_path = if cfg!(windows) {
        r"%USERPROFILE%\.codex\config.toml".to_string()
    } else {
        "~/.codex/config.toml".to_string()
    };

    let antigravity_config_path = if cfg!(windows) {
        r"%USERPROFILE%\.gemini\antigravity\mcp_config.json".to_string()
    } else {
        "~/.gemini/antigravity/mcp_config.json".to_string()
    };

    let pi_omp_config_path = if cfg!(windows) {
        r"%USERPROFILE%\.omp\agent\mcp.json".to_string()
    } else {
        "~/.omp/agent/mcp.json".to_string()
    };

    let windsurf_config_path = if cfg!(windows) {
        r"%USERPROFILE%\.codeium\windsurf\mcp_config.json".to_string()
    } else {
        "~/.codeium/windsurf/mcp_config.json".to_string()
    };

    let path_status = sona_shim::installer::check_path_status();
    let target_mcp_name = if is_nightly {
        "sona-mcp-nightly"
    } else {
        "sona-mcp"
    };
    let target_installed = path_status
        .installed_shims
        .iter()
        .any(|name| name.strip_suffix(".exe").unwrap_or(name) == target_mcp_name);

    let (command_to_run, args_to_run) =
        if (path_status.in_current_path || path_status.in_user_path) && target_installed {
            let exe_name = if cfg!(windows) {
                format!("{}.exe", target_mcp_name)
            } else {
                target_mcp_name.to_string()
            };
            let shim_exe_path = path_status.shim_dir.join(exe_name);
            let cmd = if shim_exe_path.exists() {
                shim_exe_path.to_string_lossy().to_string()
            } else {
                target_mcp_name.to_string()
            };
            (cmd, vec![])
        } else {
            let args = if is_nightly {
                vec!["--endpoint".to_string(), endpoint.clone()]
            } else {
                vec![]
            };
            (binary_path.clone(), args)
        };
    let server_key = if is_nightly { "sona-nightly" } else { "sona" };

    let stdio_snippet = serde_json::json!({
        "mcpServers": {
            server_key: {
                "command": &command_to_run,
                "args": &args_to_run
            }
        }
    });
    let stdio_json_str = serde_json::to_string_pretty(&stdio_snippet).unwrap_or_default();

    let codex_command = serde_json::to_string(&command_to_run)
        .unwrap_or_else(|_| format!("\"{}\"", command_to_run));
    let codex_args = serde_json::to_string(&args_to_run).unwrap_or_else(|_| "[]".to_string());
    let codex_config_content =
        format!("[mcp_servers.{server_key}]\ncommand = {codex_command}\nargs = {codex_args}\n");

    let pi_omp_snippet = serde_json::json!({
        "mcpServers": {
            server_key: {
                "type": "stdio",
                "command": &command_to_run,
                "args": &args_to_run
            }
        }
    });
    let pi_omp_json_str = serde_json::to_string_pretty(&pi_omp_snippet).unwrap_or_default();

    let claude_code_cli = if args_to_run.is_empty() {
        format!("claude mcp add {server_key} \"{command_to_run}\"")
    } else {
        let args_str = args_to_run
            .iter()
            .map(|a| format!("\"{}\"", a))
            .collect::<Vec<_>>()
            .join(" ");
        format!("claude mcp add {server_key} \"{command_to_run}\" -- {args_str}")
    };
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
            id: "codex".to_string(),
            name: "Codex".to_string(),
            config_path: codex_config_path,
            config_content: codex_config_content,
            format: "toml".to_string(),
        },
        McpClientGuide {
            id: "antigravity".to_string(),
            name: "Antigravity".to_string(),
            config_path: antigravity_config_path,
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
            id: "pi-omp".to_string(),
            name: "pi / omp".to_string(),
            config_path: pi_omp_config_path,
            config_content: pi_omp_json_str,
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
        let configs = build_mcp_client_configs(false).unwrap();
        assert!(!configs.binary_path.is_empty());
        assert_eq!(configs.guides.len(), 8);
        assert!(configs.guides.iter().any(|g| g.id == "claude-desktop"));
        assert!(configs.guides.iter().any(|g| g.id == "cursor"));
        assert!(configs.guides.iter().any(|g| g.id == "codex"));
        assert!(configs.guides.iter().any(|g| g.id == "antigravity"));
        assert!(configs.guides.iter().any(|g| g.id == "claude-code"));
        assert!(configs.guides.iter().any(|g| g.id == "pi-omp"));
        assert!(configs.guides.iter().any(|g| g.id == "windsurf"));
        assert!(configs.guides.iter().any(|g| g.id == "generic"));

        assert_eq!(configs.guides[6].id, "windsurf");
        assert_eq!(configs.guides[7].id, "generic");

        let codex = configs.guides.iter().find(|g| g.id == "codex").unwrap();
        assert_eq!(codex.format, "toml");
        assert!(codex.config_content.contains("[mcp_servers.sona]"));

        let pi_omp = configs.guides.iter().find(|g| g.id == "pi-omp").unwrap();
        assert!(pi_omp.config_content.contains("\"type\": \"stdio\""));

        let claude_desktop = configs
            .guides
            .iter()
            .find(|g| g.id == "claude-desktop")
            .unwrap();
        assert!(claude_desktop.config_content.contains("mcpServers"));
        assert!(claude_desktop.config_content.contains("sona"));
    }
    #[tokio::test]
    async fn test_mcp_get_client_configs_nightly() {
        let configs = build_mcp_client_configs(true).unwrap();
        assert!(!configs.binary_path.is_empty());
        assert_eq!(configs.guides.len(), 8);
        if cfg!(windows) {
            assert!(configs.endpoint.contains("sona-agent-ipc-nightly"));
        } else if cfg!(unix) {
            assert!(configs.endpoint.contains("nightly"));
        }
        let codex = configs.guides.iter().find(|g| g.id == "codex").unwrap();
        assert!(codex.config_content.contains("[mcp_servers.sona-nightly]"));
    }

    #[test]
    fn test_resolve_sona_mcp_binary_path_with_env_var() {
        let current_exe = std::env::current_exe().expect("current exe should exist");
        let (resolved, exists) = resolve_sona_mcp_binary_path_with_env(|k| match k {
            "SONA_MCP_PATH" => Some(current_exe.to_string_lossy().to_string()),
            _ => None,
        });

        assert_eq!(resolved, Some(current_exe.to_string_lossy().to_string()));
        assert!(exists);
    }

    #[test]
    fn test_resolve_sona_mcp_binary_path_with_nonexistent_env_var() {
        let (resolved, _exists) = resolve_sona_mcp_binary_path_with_env(|k| match k {
            "SONA_MCP_PATH" => Some("nonexistent/path/to/sona-mcp".to_string()),
            _ => None,
        });

        if let Some(path) = resolved {
            assert_ne!(path, "nonexistent/path/to/sona-mcp");
        }
    }

    #[test]
    fn test_resolve_sona_mcp_binary_path_with_dir_env_var() {
        let temp_dir = std::env::temp_dir();
        let (resolved, _exists) = resolve_sona_mcp_binary_path_with_env(|k| match k {
            "SONA_MCP_PATH" => Some(temp_dir.to_string_lossy().to_string()),
            _ => None,
        });

        // Should not treat temp_dir itself as a binary file
        if let Some(path) = resolved {
            assert_ne!(path, temp_dir.to_string_lossy().to_string());
        }
    }
}
