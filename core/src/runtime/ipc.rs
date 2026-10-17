use std::path::PathBuf;

pub const WINDOWS_PIPE_NAME: &str = r"\\.\pipe\sona-agent-ipc";
pub const WINDOWS_PIPE_NAME_NIGHTLY: &str = r"\\.\pipe\sona-agent-ipc-nightly";

pub fn get_unix_socket_path() -> PathBuf {
    get_unix_socket_path_with_env(|k| std::env::var(k).ok())
}

pub fn get_unix_socket_path_with_env<F: Fn(&str) -> Option<String>>(env_lookup: F) -> PathBuf {
    let is_nightly = env_lookup("SONA_CHANNEL")
        .map(|v| v.trim().eq_ignore_ascii_case("nightly"))
        .unwrap_or(false);

    let socket_name = if is_nightly {
        "sona-agent-nightly.sock"
    } else {
        "sona-agent.sock"
    };

    let xdg_dir = env_lookup("XDG_RUNTIME_DIR")
        .or_else(|| std::env::var("XDG_RUNTIME_DIR").ok())
        .filter(|d| !d.trim().is_empty());
    if let Some(runtime_dir) = xdg_dir {
        return PathBuf::from(runtime_dir).join(socket_name);
    }
    let home = env_lookup("HOME")
        .or_else(|| std::env::var("HOME").ok())
        .unwrap_or_else(|| ".".to_string());
    let dir_name = if is_nightly { "sona-nightly" } else { "sona" };
    PathBuf::from(home)
        .join(".local")
        .join("share")
        .join(dir_name)
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
        let is_nightly = env_lookup("SONA_CHANNEL")
            .map(|v| v.trim().eq_ignore_ascii_case("nightly"))
            .unwrap_or(false);
        if is_nightly {
            WINDOWS_PIPE_NAME_NIGHTLY.to_string()
        } else {
            WINDOWS_PIPE_NAME.to_string()
        }
    }
    #[cfg(unix)]
    {
        get_unix_socket_path_with_env(env_lookup)
            .to_string_lossy()
            .to_string()
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = env_lookup;
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

    #[test]
    fn test_resolve_ipc_endpoint_precedence() {
        // SONA_IPC_ENDPOINT takes highest precedence
        let ep = resolve_ipc_endpoint(|k| match k {
            "SONA_IPC_ENDPOINT" => Some("endpoint-wins".into()),
            "SONA_IPC_PIPE" => Some("pipe-loses".into()),
            "SONA_CHANNEL" => Some("nightly".into()),
            _ => None,
        });
        assert_eq!(ep, "endpoint-wins");

        // SONA_IPC_PIPE takes precedence over SONA_CHANNEL
        let ep = resolve_ipc_endpoint(|k| match k {
            "SONA_IPC_ENDPOINT" => Some("   ".into()), // whitespace falls through
            "SONA_IPC_PIPE" => Some("pipe-wins".into()),
            "SONA_CHANNEL" => Some("nightly".into()),
            _ => None,
        });
        assert_eq!(ep, "pipe-wins");

        // SONA_CHANNEL takes effect when higher overrides are absent or empty
        let ep = resolve_ipc_endpoint(|k| match k {
            "SONA_IPC_ENDPOINT" => Some("".into()),
            "SONA_IPC_PIPE" => Some("  ".into()),
            "SONA_CHANNEL" => Some("nightly".into()),
            _ => None,
        });
        #[cfg(windows)]
        assert_eq!(ep, WINDOWS_PIPE_NAME_NIGHTLY);
        #[cfg(unix)]
        assert!(ep.contains("nightly"));
    }

    #[test]
    fn test_resolve_ipc_endpoint_channels() {
        let stable_ep = resolve_ipc_endpoint(|k| {
            if k == "SONA_CHANNEL" {
                Some("stable".into())
            } else {
                None
            }
        });
        let nightly_ep = resolve_ipc_endpoint(|k| {
            if k == "SONA_CHANNEL" {
                Some("nightly".into())
            } else {
                None
            }
        });

        assert_ne!(stable_ep, nightly_ep);
        #[cfg(windows)]
        {
            assert_eq!(stable_ep, WINDOWS_PIPE_NAME);
            assert_eq!(nightly_ep, WINDOWS_PIPE_NAME_NIGHTLY);
        }
        #[cfg(unix)]
        {
            assert!(stable_ep.contains("sona"));
            assert!(nightly_ep.contains("sona-nightly") || nightly_ep.contains("nightly"));
        }
    }
}
