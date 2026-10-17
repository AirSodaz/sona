use crate::registry::{
    Channel, TargetRole, detect_active_ipc_channel, find_executable, get_default_channel,
    is_channel_installed,
};
use std::path::{Path, PathBuf};

/// Identifies the role of the invoked binary from its file stem.
pub fn detect_role(program_name: &str) -> TargetRole {
    let path = Path::new(program_name);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(program_name)
        .to_lowercase();

    if stem.contains("sona-mcp") {
        TargetRole::Mcp
    } else if stem.contains("sona-shim") {
        TargetRole::Shim
    } else {
        TargetRole::Cli
    }
}

/// Arbitrates which channel (Stable or Nightly) should be targeted.
/// Modifies `args` if a `+channel` argument was stripped.
pub fn resolve_target_channel(
    program_name: &str,
    args: &mut Vec<String>,
    current_dir: &Path,
    role: TargetRole,
) -> Channel {
    let path = Path::new(program_name);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(program_name)
        .to_lowercase();

    // Rule 1: Command name contains -nightly or -stable
    let name_channel = if stem.contains("-nightly") {
        Some(Channel::Nightly)
    } else if stem.contains("-stable") {
        Some(Channel::Stable)
    } else {
        None
    };

    // Rule 2: First CLI argument is +nightly or +stable
    let flag_channel = if let Some(first_arg) = args.first() {
        if first_arg.eq_ignore_ascii_case("+nightly") {
            args.remove(0);
            Some(Channel::Nightly)
        } else if first_arg.eq_ignore_ascii_case("+stable") {
            args.remove(0);
            Some(Channel::Stable)
        } else {
            None
        }
    } else {
        None
    };

    if let Some(c) = name_channel {
        return c;
    }
    if let Some(c) = flag_channel {
        return c;
    }

    // Rule 3: Environment variable SONA_CHANNEL
    if let Some(c) = std::env::var("SONA_CHANNEL")
        .ok()
        .and_then(|v| Channel::parse(&v))
    {
        return c;
    }

    // Rule 4: .sona-channel file in current directory or ancestor directories
    if let Some(c) = find_channel_in_ancestors(current_dir) {
        return c;
    }

    // Rule 6a: For MCP (or adaptive dispatching), probe running desktop IPC pipe instances
    if role == TargetRole::Mcp
        && let Some(active) = detect_active_ipc_channel()
    {
        return active;
    }
    // Rule 5: Global default preference from registry
    let default_channel = get_default_channel();

    // Rule 6b: Adaptive installation fallback:
    // If the default channel is not installed, but the other channel is installed, fall back!
    let other_channel = match default_channel {
        Channel::Stable => Channel::Nightly,
        Channel::Nightly => Channel::Stable,
    };

    let default_installed = is_channel_installed(default_channel);
    let other_installed = is_channel_installed(other_channel);

    if !default_installed && other_installed {
        return other_channel;
    }

    default_channel
}

/// Locates a `.sona-channel` file in `dir` or any parent directory.
pub fn find_channel_in_ancestors(dir: &Path) -> Option<Channel> {
    let mut current = dir;
    loop {
        let candidate = current.join(".sona-channel");
        if candidate.is_file()
            && let Some(c) = std::fs::read_to_string(&candidate)
                .ok()
                .and_then(|s| Channel::parse(&s))
        {
            return Some(c);
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => break,
        }
    }
    None
}

/// Resolves the absolute path to the target executable for the given role and channel.
pub fn resolve_target_executable(role: TargetRole, channel: Channel) -> Option<PathBuf> {
    find_executable(channel, role)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_role() {
        assert_eq!(detect_role("sona-cli"), TargetRole::Cli);
        assert_eq!(detect_role("sona-cli.exe"), TargetRole::Cli);
        assert_eq!(detect_role("sona-cli-nightly.exe"), TargetRole::Cli);
        assert_eq!(detect_role("sona-mcp"), TargetRole::Mcp);
        assert_eq!(detect_role("sona-mcp-stable.exe"), TargetRole::Mcp);
        assert_eq!(detect_role("sona-shim.exe"), TargetRole::Shim);
        assert_eq!(detect_role("sona"), TargetRole::Cli);
    }

    #[test]
    fn test_channel_resolution_by_name() {
        let mut args = vec!["arg1".to_string()];
        let cwd = Path::new(".");

        assert_eq!(
            resolve_target_channel("sona-cli-nightly", &mut args, cwd, TargetRole::Cli),
            Channel::Nightly
        );
        assert_eq!(args.len(), 1);

        assert_eq!(
            resolve_target_channel("sona-mcp-stable.exe", &mut args, cwd, TargetRole::Mcp),
            Channel::Stable
        );
    }

    #[test]
    fn test_channel_resolution_by_plus_flag() {
        let mut args = vec!["+nightly".to_string(), "status".to_string()];
        let cwd = Path::new(".");

        let ch = resolve_target_channel("sona-cli", &mut args, cwd, TargetRole::Cli);
        assert_eq!(ch, Channel::Nightly);
        assert_eq!(args, vec!["status".to_string()]);
    }

    #[test]
    fn test_channel_resolution_precedence() {
        // Name takes precedence over +flag, but +flag is still consumed
        let mut args = vec!["+stable".to_string(), "run".to_string()];
        let cwd = Path::new(".");
        let ch = resolve_target_channel("sona-cli-nightly", &mut args, cwd, TargetRole::Cli);
        assert_eq!(ch, Channel::Nightly);
        assert_eq!(args, vec!["run".to_string()]);
    }

    #[test]
    fn test_ancestor_channel_discovery() {
        let temp_dir = std::env::temp_dir().join(format!("sona-test-{}", std::process::id()));
        let nested = temp_dir.join("a").join("b").join("c");
        std::fs::create_dir_all(&nested).unwrap();

        // Place .sona-channel in root of temp_dir
        let marker = temp_dir.join(".sona-channel");
        std::fs::write(&marker, "nightly\n").unwrap();

        let found = find_channel_in_ancestors(&nested);
        assert_eq!(found, Some(Channel::Nightly));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
