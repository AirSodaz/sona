use std::path::PathBuf;

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

pub fn get_ipc_endpoint() -> String {
    resolve_ipc_endpoint(|k| std::env::var(k).ok())
}

pub fn resolve_ipc_endpoint<F: Fn(&str) -> Option<String>>(env_lookup: F) -> String {
    if let Some(endpoint) = env_lookup("SONA_IPC_ENDPOINT") {
        let trimmed = endpoint.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    if let Some(pipe) = env_lookup("SONA_IPC_PIPE") {
        let trimmed = pipe.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    #[cfg(windows)]
    {
        WINDOWS_PIPE_NAME.to_string()
    }
    #[cfg(unix)]
    {
        get_unix_socket_path().to_string_lossy().to_string()
    }
    #[cfg(not(any(windows, unix)))]
    {
        "unsupported".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_ipc_endpoint_overrides() {
        assert_eq!(
            resolve_ipc_endpoint(|k| if k == "SONA_IPC_ENDPOINT" {
                Some("custom-pipe".into())
            } else {
                None
            }),
            "custom-pipe"
        );
        assert_eq!(
            resolve_ipc_endpoint(|k| if k == "SONA_IPC_PIPE" {
                Some("legacy-pipe".into())
            } else {
                None
            }),
            "legacy-pipe"
        );
    }
}
