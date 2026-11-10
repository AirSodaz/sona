use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliStatus {
    pub installed: bool,
    pub in_path: bool,
    pub binary_path: Option<String>,
    pub bin_dir: Option<String>,
    pub version: String,
    pub platform: String,
    pub install_type: String,
    pub can_modify_path: bool,
}

fn resolve_cli_binary_and_bin_dir() -> (Option<PathBuf>, Option<PathBuf>, String) {
    let binary_name = if cfg!(windows) {
        "sona-cli.exe"
    } else {
        "sona-cli"
    };

    let current_exe = std::env::current_exe().ok();
    let current_dir = current_exe.as_deref().and_then(Path::parent);

    // 1. Check next to current exe (Canonical root sidecar binary)
    if let Some(dir) = current_dir {
        let bin_candidate = dir.join("bin");
        let cli_next_to_exe = dir.join(binary_name);
        if cli_next_to_exe.is_file() {
            return (
                Some(cli_next_to_exe),
                Some(bin_candidate),
                "installed_root".to_string(),
            );
        }

        // 2. Check bin/ next to current exe (Standard Windows/Linux layout fallback)
        let cli_in_bin = bin_candidate.join(binary_name);
        if cli_in_bin.is_file() {
            return (
                Some(cli_in_bin),
                Some(bin_candidate),
                "installed_with_bin".to_string(),
            );
        }

        // 3. Dev paths (target/debug or target/release or deps parent)
        let search_dirs = [
            dir.to_path_buf(),
            dir.join(".."),
            dir.join("../debug"),
            dir.join("../release"),
            dir.join("../../target/debug"),
            dir.join("../../target/release"),
        ];
        for candidate_dir in search_dirs {
            let candidate = candidate_dir.join(binary_name);
            if candidate.is_file() {
                let canon = candidate
                    .canonicalize()
                    .unwrap_or_else(|_| candidate.clone());
                let parent = canon.parent().map(Path::to_path_buf);
                let prospective_bin = parent.as_ref().map(|p| p.join("bin"));
                return (Some(canon), prospective_bin, "development".to_string());
            }
        }
    }

    // 4. Check registry paths on Windows or standard user/system paths on Unix
    #[cfg(windows)]
    {
        if let Some(found) = win_path::find_cli_in_registry_paths(binary_name) {
            let bin_dir = found.parent().map(Path::to_path_buf);
            return (Some(found), bin_dir, "system_path".to_string());
        }
    }

    #[cfg(unix)]
    {
        for standard_dir in &["/usr/local/bin", "/usr/bin"] {
            let p = Path::new(standard_dir).join(binary_name);
            if p.is_file() {
                return (
                    Some(p),
                    Some(PathBuf::from(standard_dir)),
                    "system_path".to_string(),
                );
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            let user_local = PathBuf::from(home).join(".local/bin");
            let p = user_local.join(binary_name);
            if p.is_file() {
                return (Some(p), Some(user_local), "system_path".to_string());
            }
        }
    }

    // Fallback: prospective bin dir next to current exe
    let fallback_bin_dir = current_dir.map(|d| d.join("bin"));
    (None, fallback_bin_dir, "not_found".to_string())
}

fn is_cli_currently_in_path(_bin_dir: Option<&Path>) -> bool {
    let binary_name = if cfg!(windows) {
        "sona-cli.exe"
    } else {
        "sona-cli"
    };

    // On Windows: check if bin_dir is in HKCU/HKLM Path, or if a global cli binary exists in registry paths
    #[cfg(windows)]
    {
        if _bin_dir.is_some_and(win_path::is_bin_in_user_or_system_path) {
            return true;
        }
        win_path::find_cli_in_registry_paths(binary_name).is_some()
    }

    // On Unix: check if cli is in PATH, or standard symlink exists in ~/.local/bin or /usr/local/bin
    #[cfg(unix)]
    {
        if let Ok(path_var) = std::env::var("PATH") {
            for dir in std::env::split_paths(&path_var) {
                let candidate = dir.join(binary_name);
                if candidate.exists() {
                    return true;
                }
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            let user_local = PathBuf::from(home).join(".local/bin").join(binary_name);
            if user_local.exists() {
                return true;
            }
        }
        for standard_dir in &["/usr/local/bin", "/usr/bin"] {
            let p = Path::new(standard_dir).join(binary_name);
            if p.exists() {
                return true;
            }
        }
        false
    }
}

#[cfg(windows)]
mod win_path {
    use std::path::{Path, PathBuf};
    use winreg::enums::*;
    use winreg::{RegKey, RegValue};

    fn expand_env_vars(raw: &str) -> String {
        let wide: Vec<u16> = raw.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            unsafe extern "system" {
                fn ExpandEnvironmentStringsW(lpSrc: *const u16, lpDst: *mut u16, nSize: u32)
                -> u32;
            }
            let needed = ExpandEnvironmentStringsW(wide.as_ptr(), std::ptr::null_mut(), 0);
            if needed == 0 {
                return raw.to_string();
            }
            let mut buf = vec![0u16; needed as usize];
            let written = ExpandEnvironmentStringsW(wide.as_ptr(), buf.as_mut_ptr(), needed);
            if written == 0 || written > needed {
                return raw.to_string();
            }
            let valid_len = if written > 0 && buf[written as usize - 1] == 0 {
                (written - 1) as usize
            } else {
                written as usize
            };
            String::from_utf16_lossy(&buf[..valid_len])
        }
    }

    fn strip_verbatim_prefix(p: &str) -> &str {
        if let Some(stripped) = p.strip_prefix(r"\\?\") {
            stripped
        } else {
            p
        }
    }

    fn normalize_path_cmp(p: &str) -> String {
        let expanded = expand_env_vars(p);
        let unquoted = expanded.trim().trim_matches('"');
        let stripped = strip_verbatim_prefix(unquoted);
        stripped.trim_end_matches(['\\', '/']).to_lowercase()
    }

    pub fn get_user_path() -> Result<String, String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let env = hkcu
            .open_subkey("Environment")
            .map_err(|e| format!("Failed to open HKCU\\Environment: {e}"))?;
        env.get_value("Path").map_err(|e| e.to_string())
    }

    pub fn get_system_path() -> Result<String, String> {
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let env = hklm
            .open_subkey("SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment")
            .map_err(|e| format!("Failed to open HKLM Environment: {e}"))?;
        env.get_value("Path").map_err(|e| e.to_string())
    }

    fn path_contains(path_str: &str, target: &str) -> bool {
        path_str.split(';').any(|p| normalize_path_cmp(p) == target)
    }

    pub fn is_bin_in_user_or_system_path(bin_dir: &Path) -> bool {
        let target = normalize_path_cmp(&bin_dir.to_string_lossy());
        if target.is_empty() {
            return false;
        }

        if get_user_path().is_ok_and(|p| path_contains(&p, &target)) {
            return true;
        }

        if get_system_path().is_ok_and(|p| path_contains(&p, &target)) {
            return true;
        }

        false
    }

    pub fn find_cli_in_registry_paths(binary_name: &str) -> Option<PathBuf> {
        let mut paths = Vec::new();
        if let Ok(user_path) = get_user_path() {
            for p in user_path.split(';') {
                let trimmed = p.trim().trim_matches('"');
                if !trimmed.is_empty() {
                    let expanded = expand_env_vars(trimmed);
                    paths.push(PathBuf::from(expanded));
                }
            }
        }
        if let Ok(sys_path) = get_system_path() {
            for p in sys_path.split(';') {
                let trimmed = p.trim().trim_matches('"');
                if !trimmed.is_empty() {
                    let expanded = expand_env_vars(trimmed);
                    paths.push(PathBuf::from(expanded));
                }
            }
        }

        for dir in paths {
            let candidate = dir.join(binary_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        None
    }

    pub fn set_bin_in_user_path(bin_dir: &Path, enable: bool) -> Result<(), String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (env, _) = hkcu
            .create_subkey("Environment")
            .map_err(|e| format!("Failed to open HKCU\\Environment: {e}"))?;

        let (current_path, vtype) = match env.get_raw_value("Path") {
            Ok(raw) => {
                let s = env.get_value::<String, _>("Path").unwrap_or_default();
                (s, raw.vtype)
            }
            Err(_) => (String::new(), REG_EXPAND_SZ),
        };

        let raw_display = bin_dir.to_string_lossy();
        let clean_display = strip_verbatim_prefix(&raw_display);
        let target = normalize_path_cmp(clean_display);
        let target_display = clean_display.to_string();

        let mut parts: Vec<String> = current_path
            .split(';')
            .map(str::trim)
            .filter(|s| !s.is_empty() && normalize_path_cmp(s) != target)
            .map(String::from)
            .collect();

        if enable {
            parts.push(target_display);
        }

        let new_path = parts.join(";");

        // Preserve REG_EXPAND_SZ so user environment variables like %USERPROFILE% continue to expand
        let mut utf16: Vec<u8> = new_path
            .encode_utf16()
            .flat_map(|c| c.to_le_bytes())
            .collect();
        utf16.extend_from_slice(&[0, 0]); // null terminator
        let reg_val = RegValue {
            vtype: if vtype == REG_SZ {
                REG_SZ
            } else {
                REG_EXPAND_SZ
            },
            bytes: utf16.into(),
        };
        env.set_raw_value("Path", &reg_val)
            .map_err(|e| format!("Failed to update HKCU\\Environment\\Path: {e}"))?;

        // Broadcast WM_SETTINGCHANGE
        unsafe {
            use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
            use windows::Win32::UI::WindowsAndMessaging::{
                HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE,
            };
            let param = "Environment\0".encode_utf16().collect::<Vec<u16>>();
            let mut result: usize = 0;
            let _ = SendMessageTimeoutW(
                HWND(HWND_BROADCAST.0),
                WM_SETTINGCHANGE,
                WPARAM(0),
                LPARAM(param.as_ptr() as isize),
                SMTO_ABORTIFHUNG,
                3000,
                Some(&mut result),
            );
        }

        Ok(())
    }
}

#[cfg(unix)]
mod unix_path {
    use std::os::unix::fs::symlink;
    use std::path::Path;

    pub fn set_cli_symlinks(cli_bin: &Path, enable: bool) -> Result<(), String> {
        let home = std::env::var("HOME").map_err(|_| "HOME environment variable not set")?;
        let user_bin = std::path::PathBuf::from(home).join(".local/bin");
        let _ = std::fs::create_dir_all(&user_bin);

        let target_symlink = user_bin.join("sona-cli");

        if enable {
            if target_symlink.exists() || target_symlink.is_symlink() {
                let _ = std::fs::remove_file(&target_symlink);
            }
            symlink(cli_bin, &target_symlink).map_err(|e| {
                format!(
                    "Failed to create symlink at {}: {e}",
                    target_symlink.display()
                )
            })?;

            // Also symlink sona-mcp if it exists alongside cli_bin
            if let Some(parent) = cli_bin.parent() {
                let mcp_bin = parent.join("sona-mcp");
                let mcp_symlink = user_bin.join("sona-mcp");
                if mcp_bin.is_file() {
                    let _ = std::fs::remove_file(&mcp_symlink);
                    let _ = symlink(&mcp_bin, &mcp_symlink);
                }
            }
        } else {
            if target_symlink.exists() || target_symlink.is_symlink() {
                let _ = std::fs::remove_file(&target_symlink);
            }
            let mcp_symlink = user_bin.join("sona-mcp");
            if mcp_symlink.exists() || mcp_symlink.is_symlink() {
                let _ = std::fs::remove_file(&mcp_symlink);
            }
        }

        Ok(())
    }
}

fn clean_path_display(p: &Path) -> String {
    let s = p.to_string_lossy().to_string();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        stripped.to_string()
    } else {
        s
    }
}

#[tauri::command]
pub async fn cli_get_status() -> Result<CliStatus, String> {
    let (cli_path, bin_dir, install_type) = resolve_cli_binary_and_bin_dir();
    let installed = cli_path.is_some();
    let in_path = is_cli_currently_in_path(bin_dir.as_deref());

    let platform = if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
    .to_string();

    let can_modify_path = if cfg!(windows) {
        install_type != "system_path"
            && bin_dir.is_some()
            && (cli_path.is_some() || install_type != "not_found")
    } else if cfg!(unix) {
        install_type != "system_path" && cli_path.is_some()
    } else {
        false
    };

    Ok(CliStatus {
        installed,
        in_path,
        binary_path: cli_path.as_deref().map(clean_path_display),
        bin_dir: bin_dir.as_deref().map(clean_path_display),
        version: env!("CARGO_PKG_VERSION").to_string(),
        platform,
        install_type,
        can_modify_path,
    })
}

#[tauri::command]
pub async fn cli_set_path_enabled(enabled: bool) -> Result<CliStatus, String> {
    let (_cli_path, _bin_dir, _) = resolve_cli_binary_and_bin_dir();

    #[cfg(windows)]
    {
        let bin = _bin_dir
            .as_deref()
            .ok_or_else(|| "Cannot determine Sona bin directory".to_string())?;
        // Ensure bin directory exists
        if enabled {
            if !bin.exists() {
                std::fs::create_dir_all(bin).map_err(|e| {
                    format!("Failed to create bin directory at {}: {e}", bin.display())
                })?;
            }
            // If bin/sona-cli.exe doesn't exist, try to locate sona-bin-shim.exe and copy it as shims
            let cli_shim = bin.join("sona-cli.exe");
            let mcp_shim = bin.join("sona-mcp.exe");
            if !cli_shim.exists() || !mcp_shim.exists() {
                let shim_candidates = [
                    bin.parent().map(|p| p.join("sona-bin-shim.exe")),
                    std::env::current_exe()
                        .ok()
                        .as_deref()
                        .and_then(Path::parent)
                        .map(|p| p.join("sona-bin-shim.exe")),
                    bin.parent()
                        .and_then(|p| p.parent())
                        .map(|p| p.join("target").join("release").join("sona-bin-shim.exe")),
                    bin.parent()
                        .and_then(|p| p.parent())
                        .map(|p| p.join("target").join("debug").join("sona-bin-shim.exe")),
                ];
                let mut copied = false;
                for candidate in shim_candidates.into_iter().flatten() {
                    if candidate.is_file() {
                        let _ = std::fs::copy(&candidate, &cli_shim);
                        let _ = std::fs::copy(&candidate, &mcp_shim);
                        copied = true;
                        break;
                    }
                }
                if !copied && !cli_shim.exists() {
                    return Err(format!(
                        "Cannot initialize CLI in {}: sona-bin-shim.exe was not found. The shim trampoline is required to run CLI tools without DLL search conflicts.",
                        bin.display()
                    ));
                }
            }
        }
        win_path::set_bin_in_user_path(bin, enabled)?;
    }

    #[cfg(unix)]
    {
        let cli = _cli_path
            .as_deref()
            .ok_or_else(|| "Sona CLI binary not found".to_string())?;
        unix_path::set_cli_symlinks(cli, enabled)?;
    }

    cli_get_status().await
}

#[tauri::command]
pub async fn cli_get_completion_script(shell: String) -> Result<String, String> {
    let (cli_path, _, _) = resolve_cli_binary_and_bin_dir();

    if let Some(cli) = cli_path {
        let output = std::process::Command::new(cli)
            .args(["completion", &shell])
            .output()
            .map_err(|e| format!("Failed to run sona-cli completion: {e}"))?;

        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).to_string());
        }
    }

    // Fallback if binary is not runnable directly (e.g. template)
    Err("Sona CLI executable not available to generate completions.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cli_status() {
        let status = cli_get_status().await.unwrap();
        println!("CLI STATUS DEBUG: {:#?}", status);
        assert!(!status.version.is_empty());
        assert!(!status.platform.is_empty());
        if status.installed {
            assert!(status.binary_path.is_some());
        }
    }
}
