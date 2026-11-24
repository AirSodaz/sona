use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const CLI_PROBE_TIMEOUT: Duration = Duration::from_millis(1500);
const CLI_GENERATE_TIMEOUT: Duration = Duration::from_millis(4000);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CliPathState {
    NotInPath,
    ActiveCurrent,
    ActiveOther,
    ShadowedByOther,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CliPathEntrySource {
    SystemPath,
    UserPath,
    UnixSymlink,
    UnixPath,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliPathEntry {
    pub path_dir: String,
    pub entry_binary_path: String,
    pub resolved_binary_path: String,
    pub version: Option<String>,
    pub source: CliPathEntrySource,
    pub is_active: bool,
    pub is_current_app: bool,
    pub can_remove: bool,
    pub is_broken: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliStatus {
    pub installed: bool,
    pub in_path: bool,
    pub path_state: CliPathState,
    pub binary_path: Option<String>,
    pub bin_dir: Option<String>,
    pub version: String,
    pub platform: String,
    pub install_type: String,
    pub can_modify_path: bool,
    pub blocked_by_system_path: bool,
    pub active_entry: Option<CliPathEntry>,
    pub path_entries: Vec<CliPathEntry>,
}

pub(crate) fn cli_binary_name() -> &'static str {
    if cfg!(windows) {
        "sona-cli.exe"
    } else {
        "sona-cli"
    }
}

pub(crate) fn clean_path_display(p: &Path) -> String {
    let s = p.to_string_lossy().to_string();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        stripped.to_string()
    } else {
        s
    }
}

pub(crate) fn strip_verbatim_prefix(p: &str) -> &str {
    p.strip_prefix(r"\\?\").unwrap_or(p)
}

#[cfg(windows)]
pub(crate) fn expand_env_vars(raw: &str) -> String {
    let wide: Vec<u16> = raw.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        unsafe extern "system" {
            fn ExpandEnvironmentStringsW(lpSrc: *const u16, lpDst: *mut u16, nSize: u32) -> u32;
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

pub(crate) fn normalize_path_cmp(p: &str) -> String {
    #[cfg(windows)]
    let expanded = expand_env_vars(p);
    #[cfg(not(windows))]
    let expanded = p.to_string();

    let unquoted = expanded.trim().trim_matches('"');
    let stripped = strip_verbatim_prefix(unquoted);
    let trimmed = stripped.trim_end_matches(['\\', '/']);
    #[cfg(windows)]
    {
        trimmed.to_lowercase()
    }
    #[cfg(not(windows))]
    {
        trimmed.to_string()
    }
}

fn normalize_pathbuf_cmp(p: &Path) -> String {
    normalize_path_cmp(&p.to_string_lossy())
}

pub(crate) fn resolve_current_app_cli() -> (Option<PathBuf>, Option<PathBuf>, String) {
    let binary_name = cli_binary_name();
    let current_exe = std::env::current_exe().ok();
    let current_dir = current_exe.as_deref().and_then(Path::parent);

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

        let cli_in_bin = bin_candidate.join(binary_name);
        if cli_in_bin.is_file() {
            return (
                Some(cli_in_bin),
                Some(bin_candidate),
                "installed_with_bin".to_string(),
            );
        }

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
                let canon = candidate.canonicalize().unwrap_or(candidate);
                let parent = canon.parent().map(Path::to_path_buf);
                let prospective_bin = parent.as_ref().map(|p| p.join("bin"));
                return (Some(canon), prospective_bin, "development".to_string());
            }
        }
    }

    let fallback_bin_dir = current_dir.map(|d| d.join("bin"));
    (None, fallback_bin_dir, "not_found".to_string())
}

pub(crate) struct CommandOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

pub(crate) fn run_command_with_timeout(
    executable: &Path,
    args: &[&str],
    timeout: Duration,
) -> Result<CommandOutput, String> {
    let mut cmd = Command::new(executable);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn {}: {e}", executable.display()))?;

    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();

    let stdout_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(ref mut pipe) = stdout_pipe {
            let _ = pipe.read_to_end(&mut buf);
        }
        buf
    });
    let stderr_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(ref mut pipe) = stderr_pipe {
            let _ = pipe.read_to_end(&mut buf);
        }
        buf
    });

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = stdout_handle.join();
                    let _ = stderr_handle.join();
                    return Err(format!("Timed out waiting for {}", executable.display()));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => {
                let _ = child.kill();
                let _ = stdout_handle.join();
                let _ = stderr_handle.join();
                return Err(format!("Failed waiting for {}: {e}", executable.display()));
            }
        }
    };

    let stdout_bytes = stdout_handle.join().unwrap_or_default();
    let stderr_bytes = stderr_handle.join().unwrap_or_default();

    Ok(CommandOutput {
        success: status.success(),
        stdout: String::from_utf8_lossy(&stdout_bytes).to_string(),
        stderr: String::from_utf8_lossy(&stderr_bytes).to_string(),
    })
}

pub(crate) fn parse_cli_version_output(stdout: &str) -> Option<String> {
    let first_line = stdout.lines().map(str::trim).find(|l| !l.is_empty())?;
    let candidate = if let Some(rest) = first_line.strip_prefix("sona-cli ") {
        rest.trim()
    } else if let Some(rest) = first_line.strip_prefix("sona-cli.exe ") {
        rest.trim()
    } else if first_line
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_digit())
    {
        first_line
    } else {
        return None;
    };

    let token = candidate.split_whitespace().next()?.trim_start_matches('v');
    if token.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        Some(token.to_string())
    } else {
        None
    }
}

pub(crate) fn probe_cli_version(binary_path: &Path) -> (Option<String>, bool) {
    if !binary_path.exists() {
        return (None, true);
    }
    match run_command_with_timeout(binary_path, &["--version"], CLI_PROBE_TIMEOUT) {
        Ok(out) if out.success => (parse_cli_version_output(&out.stdout), false),
        Ok(out) => {
            let broken = out.stderr.contains("sona-bin-shim:")
                || out.stderr.contains("target executable not found");
            (None, broken)
        }
        Err(_) => (None, false),
    }
}

pub(crate) fn run_cli_completion_command(
    binary_path: &Path,
    shell: &str,
) -> Result<String, String> {
    let out = run_command_with_timeout(binary_path, &["completion", shell], CLI_GENERATE_TIMEOUT)?;
    if out.success {
        Ok(out.stdout)
    } else {
        let detail = out.stderr.trim();
        if detail.is_empty() {
            Err(format!("sona-cli completion {shell} failed"))
        } else {
            Err(format!("sona-cli completion {shell} failed: {detail}"))
        }
    }
}

#[cfg(windows)]
pub(crate) mod win_path {
    use super::*;
    use winreg::enums::*;
    use winreg::{RegKey, RegValue};

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

    pub fn get_windows_documents_dir() -> Option<PathBuf> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(folders) = hkcu.open_subkey(
            "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\User Shell Folders",
        ) && let Ok(personal) = folders.get_value::<String, _>("Personal")
        {
            let expanded = expand_env_vars(personal.trim());
            if !expanded.is_empty() {
                return Some(PathBuf::from(strip_verbatim_prefix(&expanded)));
            }
        }
        std::env::var_os("USERPROFILE")
            .filter(|s| !s.is_empty())
            .map(|p| PathBuf::from(p).join("Documents"))
    }

    pub fn split_raw_path_entries(raw: &str) -> Vec<PathBuf> {
        raw.split(';')
            .map(|s| s.trim().trim_matches('"'))
            .filter(|s| !s.is_empty())
            .map(|s| PathBuf::from(strip_verbatim_prefix(&expand_env_vars(s))))
            .collect()
    }

    fn write_user_path_parts(env: &RegKey, parts: &[String], vtype: RegType) -> Result<(), String> {
        let new_path = parts.join(";");
        let mut utf16: Vec<u8> = new_path
            .encode_utf16()
            .flat_map(|c| c.to_le_bytes())
            .collect();
        utf16.extend_from_slice(&[0, 0]);
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

    pub fn update_user_path(remove_dirs: &[&Path], add_dir: Option<&Path>) -> Result<(), String> {
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

        let mut remove_targets: HashSet<String> = remove_dirs
            .iter()
            .map(|p| normalize_pathbuf_cmp(p))
            .filter(|s| !s.is_empty())
            .collect();
        if let Some(dir) = add_dir {
            let norm = normalize_pathbuf_cmp(dir);
            if !norm.is_empty() {
                remove_targets.insert(norm);
            }
        }

        let mut parts: Vec<String> = current_path
            .split(';')
            .map(str::trim)
            .filter(|s| !s.is_empty() && !remove_targets.contains(&normalize_path_cmp(s)))
            .map(String::from)
            .collect();

        if let Some(dir) = add_dir {
            let clean_display = clean_path_display(dir);
            if !clean_display.is_empty() {
                parts.push(clean_display);
            }
        }

        write_user_path_parts(&env, &parts, vtype)
    }

    pub fn user_path_contains(target_dir: &Path) -> bool {
        let norm = normalize_pathbuf_cmp(target_dir);
        if norm.is_empty() {
            return false;
        }
        get_user_path()
            .map(|raw| raw.split(';').any(|p| normalize_path_cmp(p) == norm))
            .unwrap_or(false)
    }
}

#[cfg(unix)]
mod unix_path {
    use super::*;
    use std::os::unix::fs::symlink;

    pub fn user_local_bin() -> Option<PathBuf> {
        std::env::var_os("HOME")
            .filter(|s| !s.is_empty())
            .map(|h| PathBuf::from(h).join(".local").join("bin"))
    }

    pub fn set_cli_symlinks(cli_bin: &Path, enable: bool) -> Result<(), String> {
        let user_bin =
            user_local_bin().ok_or_else(|| "HOME environment variable not set".to_string())?;
        let target_symlink = user_bin.join("sona-cli");
        let mcp_symlink = user_bin.join("sona-mcp");

        if enable {
            let _ = std::fs::create_dir_all(&user_bin);
            if target_symlink.exists() || target_symlink.is_symlink() {
                let _ = std::fs::remove_file(&target_symlink);
            }
            symlink(cli_bin, &target_symlink).map_err(|e| {
                format!(
                    "Failed to create symlink at {}: {e}",
                    target_symlink.display()
                )
            })?;

            if let Some(parent) = cli_bin.parent() {
                let mcp_bin = parent.join("sona-mcp");
                if mcp_bin.is_file() {
                    let _ = std::fs::remove_file(&mcp_symlink);
                    let _ = symlink(&mcp_bin, &mcp_symlink);
                }
            }
        } else {
            if target_symlink.exists() || target_symlink.is_symlink() {
                let _ = std::fs::remove_file(&target_symlink);
            }
            if mcp_symlink.exists() || mcp_symlink.is_symlink() {
                let _ = std::fs::remove_file(&mcp_symlink);
            }
        }

        Ok(())
    }

    pub fn remove_user_symlink_if_matches(path_dir: &Path) -> Result<(), String> {
        let Some(user_bin) = user_local_bin() else {
            return Err("HOME environment variable not set".to_string());
        };
        if normalize_pathbuf_cmp(path_dir) != normalize_pathbuf_cmp(&user_bin) {
            return Err(format!(
                "Cannot remove system-managed PATH entry without elevated privileges: {}",
                path_dir.display()
            ));
        }
        let target_symlink = user_bin.join("sona-cli");
        if target_symlink.is_symlink() {
            std::fs::remove_file(&target_symlink).map_err(|e| {
                format!("Failed to remove symlink {}: {e}", target_symlink.display())
            })?;
        } else if target_symlink.exists() {
            return Err(format!(
                "Refusing to remove non-symlink binary at {}",
                target_symlink.display()
            ));
        }
        let mcp_symlink = user_bin.join("sona-mcp");
        if mcp_symlink.is_symlink() {
            let _ = std::fs::remove_file(&mcp_symlink);
        }
        Ok(())
    }
}

fn resolve_shim_or_symlink_target(_dir: &Path, entry_binary: &Path) -> (PathBuf, bool) {
    #[cfg(windows)]
    {
        if _dir
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.eq_ignore_ascii_case("bin"))
            && let Some(parent) = _dir.parent()
        {
            let target = parent.join(cli_binary_name());
            if target.is_file() {
                return (target, false);
            }
            // If entry_binary is a small shim trampoline without a parent sona-cli.exe, it is broken
            let looks_like_shim = entry_binary
                .metadata()
                .map(|m| m.len() < 2 * 1024 * 1024)
                .unwrap_or(false);
            if looks_like_shim {
                return (target, true);
            }
        }
        (entry_binary.to_path_buf(), false)
    }

    #[cfg(not(windows))]
    {
        if entry_binary.is_symlink() {
            match entry_binary.canonicalize() {
                Ok(target) => (target, false),
                Err(_) => {
                    let unresolved = std::fs::read_link(entry_binary)
                        .unwrap_or_else(|_| entry_binary.to_path_buf());
                    (unresolved, true)
                }
            }
        } else {
            (entry_binary.to_path_buf(), false)
        }
    }
}

fn entry_matches_current_app(
    path_dir: &Path,
    resolved_binary: &Path,
    current_cli: Option<&Path>,
    current_bin_dir: Option<&Path>,
) -> bool {
    let dir_norm = normalize_pathbuf_cmp(path_dir);
    let resolved_norm = normalize_pathbuf_cmp(resolved_binary);

    if let Some(bin_dir) = current_bin_dir
        && !dir_norm.is_empty()
        && dir_norm == normalize_pathbuf_cmp(bin_dir)
    {
        return true;
    }

    if let Some(cli) = current_cli {
        let cli_canon = cli.canonicalize().unwrap_or_else(|_| cli.to_path_buf());
        let resolved_canon = resolved_binary
            .canonicalize()
            .unwrap_or_else(|_| resolved_binary.to_path_buf());
        if normalize_pathbuf_cmp(&cli_canon) == normalize_pathbuf_cmp(&resolved_canon) {
            return true;
        }
        if let Some(cli_dir) = cli.parent()
            && dir_norm == normalize_pathbuf_cmp(cli_dir)
        {
            return true;
        }
        if !resolved_norm.is_empty() && resolved_norm == normalize_pathbuf_cmp(cli) {
            return true;
        }
    }

    false
}

pub(crate) fn scan_path_entries(
    current_cli: Option<&Path>,
    current_bin_dir: Option<&Path>,
) -> Vec<CliPathEntry> {
    let binary_name = cli_binary_name();
    let mut candidates: Vec<(PathBuf, CliPathEntrySource, bool)> = Vec::new();

    #[cfg(windows)]
    {
        // Windows resolves HKLM (System) Path BEFORE HKCU (User) Path.
        if let Ok(sys_raw) = win_path::get_system_path() {
            for dir in win_path::split_raw_path_entries(&sys_raw) {
                candidates.push((dir, CliPathEntrySource::SystemPath, false));
            }
        }
        if let Ok(user_raw) = win_path::get_user_path() {
            for dir in win_path::split_raw_path_entries(&user_raw) {
                candidates.push((dir, CliPathEntrySource::UserPath, true));
            }
        }
    }

    #[cfg(unix)]
    {
        let user_bin = unix_path::user_local_bin();
        if let Ok(path_var) = std::env::var("PATH") {
            for dir in std::env::split_paths(&path_var) {
                let is_user_bin = user_bin
                    .as_ref()
                    .is_some_and(|ub| normalize_pathbuf_cmp(&dir) == normalize_pathbuf_cmp(ub));
                let entry_bin = dir.join(binary_name);
                if is_user_bin && entry_bin.is_symlink() {
                    candidates.push((dir, CliPathEntrySource::UnixSymlink, true));
                } else {
                    candidates.push((dir, CliPathEntrySource::UnixPath, false));
                }
            }
        }

        if let Some(ub) = user_bin {
            let entry_bin = ub.join(binary_name);
            let source = if entry_bin.is_symlink() {
                CliPathEntrySource::UnixSymlink
            } else {
                CliPathEntrySource::UnixPath
            };
            let can_remove = entry_bin.is_symlink();
            candidates.push((ub, source, can_remove));
        }

        for std_dir in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"] {
            candidates.push((PathBuf::from(std_dir), CliPathEntrySource::UnixPath, false));
        }
    }

    let mut seen_dirs = HashSet::new();
    let mut entries = Vec::new();

    for (dir, source, can_remove) in candidates {
        let norm = normalize_pathbuf_cmp(&dir);
        if norm.is_empty() || !seen_dirs.insert(norm) {
            continue;
        }

        let entry_binary = dir.join(binary_name);
        if !entry_binary.is_file() && !entry_binary.is_symlink() {
            continue;
        }

        let (resolved_binary, broken_target) = resolve_shim_or_symlink_target(&dir, &entry_binary);
        let (version, probe_broken) = if broken_target {
            (None, true)
        } else {
            probe_cli_version(&entry_binary)
        };
        let is_broken = broken_target || probe_broken;
        let is_current_app =
            entry_matches_current_app(&dir, &resolved_binary, current_cli, current_bin_dir);

        entries.push(CliPathEntry {
            path_dir: clean_path_display(&dir),
            entry_binary_path: clean_path_display(&entry_binary),
            resolved_binary_path: clean_path_display(&resolved_binary),
            version,
            source,
            is_active: false,
            is_current_app,
            can_remove,
            is_broken,
        });
    }

    if let Some(first) = entries.first_mut() {
        first.is_active = true;
    }

    entries
}

pub(crate) fn compute_path_state(entries: &[CliPathEntry]) -> (CliPathState, bool) {
    let Some(first) = entries.first() else {
        return (CliPathState::NotInPath, false);
    };

    if first.is_current_app && !first.is_broken {
        return (CliPathState::ActiveCurrent, false);
    }

    let current_idx = entries
        .iter()
        .position(|e| e.is_current_app && !e.is_broken);
    let state = if current_idx.is_some() {
        CliPathState::ShadowedByOther
    } else {
        CliPathState::ActiveOther
    };

    let check_end = current_idx.unwrap_or(0);
    let blocked_by_system = entries[..=check_end]
        .iter()
        .any(|e| !e.is_current_app && !e.can_remove);

    (state, blocked_by_system)
}

#[cfg(windows)]
pub(crate) fn ensure_windows_shims_ready(bin: &Path) -> Result<(), String> {
    if !bin.exists() {
        std::fs::create_dir_all(bin)
            .map_err(|e| format!("Failed to create bin directory at {}: {e}", bin.display()))?;
    }

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

    Ok(())
}

pub(crate) fn build_cli_status_sync() -> CliStatus {
    let (cli_path, bin_dir, install_type) = resolve_current_app_cli();
    let installed = cli_path.is_some();

    let path_entries = scan_path_entries(cli_path.as_deref(), bin_dir.as_deref());
    let (path_state, blocked_by_system_path) = compute_path_state(&path_entries);
    let in_path = path_state == CliPathState::ActiveCurrent;
    let active_entry = path_entries.first().cloned();

    let version = cli_path
        .as_deref()
        .and_then(|p| probe_cli_version(p).0)
        .unwrap_or_else(|| env!("SONA_DISPLAY_VERSION").to_string());

    let platform = if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
    .to_string();

    let can_modify_path = if cfg!(windows) {
        bin_dir.is_some() && (cli_path.is_some() || install_type != "not_found")
    } else if cfg!(unix) {
        cli_path.is_some()
    } else {
        false
    };

    CliStatus {
        installed,
        in_path,
        path_state,
        binary_path: cli_path.as_deref().map(clean_path_display),
        bin_dir: bin_dir.as_deref().map(clean_path_display),
        version,
        platform,
        install_type,
        can_modify_path,
        blocked_by_system_path,
        active_entry,
        path_entries,
    }
}

pub(crate) fn set_path_enabled_sync(enabled: bool) -> Result<(), String> {
    let (cli_path, bin_dir, _) = resolve_current_app_cli();

    #[cfg(windows)]
    {
        let bin = bin_dir
            .as_deref()
            .ok_or_else(|| "Cannot determine Sona bin directory".to_string())?;
        if enabled {
            if cli_path.is_none() {
                return Err("Sona CLI binary not found for current app".to_string());
            }
            ensure_windows_shims_ready(bin)?;
            win_path::update_user_path(&[], Some(bin))?;
        } else {
            win_path::update_user_path(&[bin], None)?;
        }
    }

    #[cfg(unix)]
    {
        let _ = bin_dir;
        let cli = cli_path
            .as_deref()
            .ok_or_else(|| "Sona CLI binary not found".to_string())?;
        unix_path::set_cli_symlinks(cli, enabled)?;
    }

    Ok(())
}

pub(crate) fn switch_path_to_current_sync() -> Result<(), String> {
    let (cli_path, bin_dir, _) = resolve_current_app_cli();
    let entries = scan_path_entries(cli_path.as_deref(), bin_dir.as_deref());
    let (_, blocked_by_system) = compute_path_state(&entries);

    if blocked_by_system {
        return Err(
            "A system-level PATH entry takes precedence and cannot be overridden without administrator privileges."
                .to_string(),
        );
    }

    #[cfg(windows)]
    {
        if cli_path.is_none() {
            return Err("Sona CLI binary not found for current app".to_string());
        }
        let bin = bin_dir
            .as_deref()
            .ok_or_else(|| "Cannot determine Sona bin directory".to_string())?;
        ensure_windows_shims_ready(bin)?;

        let removable_dirs: Vec<PathBuf> = entries
            .iter()
            .filter(|e| e.can_remove)
            .map(|e| PathBuf::from(&e.path_dir))
            .collect();
        let remove_refs: Vec<&Path> = removable_dirs.iter().map(PathBuf::as_path).collect();
        win_path::update_user_path(&remove_refs, Some(bin))?;
    }

    #[cfg(unix)]
    {
        let _ = bin_dir;
        let cli = cli_path
            .as_deref()
            .ok_or_else(|| "Sona CLI binary not found".to_string())?;
        unix_path::set_cli_symlinks(cli, true)?;
    }

    Ok(())
}

pub(crate) fn remove_path_entry_sync(path_dir: String) -> Result<(), String> {
    let target = PathBuf::from(path_dir.trim());
    if target.as_os_str().is_empty() {
        return Err("PATH directory cannot be empty".to_string());
    }

    #[cfg(windows)]
    {
        if !win_path::user_path_contains(&target) {
            return Err(format!(
                "Cannot remove system or unmanaged PATH entry without elevated privileges: {}",
                target.display()
            ));
        }
        win_path::update_user_path(&[target.as_path()], None)?;
    }

    #[cfg(unix)]
    {
        unix_path::remove_user_symlink_if_matches(&target)?;
    }

    Ok(())
}

pub(crate) fn best_available_cli_binary() -> Option<PathBuf> {
    let (current_cli, current_bin_dir, _) = resolve_current_app_cli();
    if let Some(cli) = current_cli {
        return Some(cli);
    }
    let entries = scan_path_entries(None, current_bin_dir.as_deref());
    entries
        .into_iter()
        .find(|e| !e.is_broken)
        .map(|e| PathBuf::from(e.entry_binary_path))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(
        dir: &str,
        is_current_app: bool,
        can_remove: bool,
        is_broken: bool,
    ) -> CliPathEntry {
        CliPathEntry {
            path_dir: dir.to_string(),
            entry_binary_path: format!("{dir}/sona-cli"),
            resolved_binary_path: format!("{dir}/sona-cli"),
            version: (!is_broken).then(|| "0.10.5".to_string()),
            source: if can_remove {
                CliPathEntrySource::UserPath
            } else {
                CliPathEntrySource::SystemPath
            },
            is_active: false,
            is_current_app,
            can_remove,
            is_broken,
        }
    }

    #[test]
    fn parses_cli_version_output_across_channels() {
        assert_eq!(
            parse_cli_version_output("sona-cli 0.10.5\n"),
            Some("0.10.5".to_string())
        );
        assert_eq!(
            parse_cli_version_output("sona-cli 0.10.5-nightly.42\r\n"),
            Some("0.10.5-nightly.42".to_string())
        );
        assert_eq!(
            parse_cli_version_output("sona-cli 0.10.5+dev.abc1234\n"),
            Some("0.10.5+dev.abc1234".to_string())
        );
        assert_eq!(parse_cli_version_output("unexpected output"), None);
    }

    #[test]
    fn compute_path_state_covers_all_four_states_and_system_block() {
        assert_eq!(compute_path_state(&[]), (CliPathState::NotInPath, false));

        let active_curr = vec![make_entry("C:\\Current\\bin", true, true, false)];
        assert_eq!(
            compute_path_state(&active_curr),
            (CliPathState::ActiveCurrent, false)
        );

        let other_user = vec![make_entry("C:\\Other\\bin", false, true, false)];
        assert_eq!(
            compute_path_state(&other_user),
            (CliPathState::ActiveOther, false)
        );

        let other_sys = vec![make_entry(
            "C:\\Program Files\\Sona\\bin",
            false,
            false,
            false,
        )];
        assert_eq!(
            compute_path_state(&other_sys),
            (CliPathState::ActiveOther, true)
        );

        let shadowed_by_user = vec![
            make_entry("C:\\Old\\bin", false, true, false),
            make_entry("C:\\Current\\bin", true, true, false),
        ];
        assert_eq!(
            compute_path_state(&shadowed_by_user),
            (CliPathState::ShadowedByOther, false)
        );

        let shadowed_by_sys = vec![
            make_entry("C:\\Program Files\\Sona\\bin", false, false, false),
            make_entry("C:\\Current\\bin", true, true, false),
        ];
        assert_eq!(
            compute_path_state(&shadowed_by_sys),
            (CliPathState::ShadowedByOther, true)
        );

        let broken_curr = vec![make_entry("C:\\Current\\bin", true, true, true)];
        assert_eq!(
            compute_path_state(&broken_curr),
            (CliPathState::ActiveOther, false)
        );
    }

    #[test]
    fn normalize_path_cmp_strips_quotes_verbatim_and_trailing_slashes() {
        let a = normalize_path_cmp(r#""\\?\C:\Users\Test\Sona\bin\""#);
        let b = normalize_path_cmp(r"C:\Users\Test\Sona\bin/");
        #[cfg(windows)]
        assert_eq!(a, r"c:\users\test\sona\bin");
        #[cfg(not(windows))]
        assert_eq!(a, r"C:\Users\Test\Sona\bin");
        assert_eq!(a, b);
    }
}
