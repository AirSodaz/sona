use crate::registry::{
    Channel, get_registered_channel, get_sona_dir, is_channel_installed, register_channel,
    set_default_channel, unregister_channel,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathStatus {
    pub shim_dir: PathBuf,
    pub in_current_path: bool,
    pub in_user_path: bool,
    pub installed_shims: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallReport {
    pub shim_dir: PathBuf,
    pub created_shims: Vec<String>,
    pub updated_path: bool,
    pub registered_channel: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UninstallReport {
    pub removed_channel: Option<String>,
    pub shims_removed: bool,
    pub path_removed: bool,
    pub active_remaining_channel: Option<String>,
}

pub fn get_shim_dir() -> PathBuf {
    get_sona_dir().join("bin")
}

pub fn shim_binary_names() -> &'static [&'static str] {
    &[
        "sona-shim",
        "sona-cli",
        "sona-mcp",
        "sona",
        "sona-cli-stable",
        "sona-cli-nightly",
        "sona-mcp-stable",
        "sona-mcp-nightly",
    ]
}

pub fn check_path_status() -> PathStatus {
    let shim_dir = get_shim_dir();
    let norm_shim = normalize_path(&shim_dir);

    // 1. Current process PATH
    let mut in_current_path = false;
    if let Ok(path_env) = std::env::var("PATH") {
        for entry in std::env::split_paths(&path_env) {
            if normalize_path(&entry) == norm_shim {
                in_current_path = true;
                break;
            }
        }
    }

    // 2. User environment PATH (Windows registry / Unix files)
    let in_user_path = check_user_env_has_path(&shim_dir);

    // 3. Existing shims
    let mut installed_shims = Vec::new();
    for name in shim_binary_names() {
        let exe_name = if cfg!(windows) {
            format!("{}.exe", name)
        } else {
            name.to_string()
        };
        let p = shim_dir.join(&exe_name);
        if p.is_file() {
            installed_shims.push(exe_name);
        }
    }

    PathStatus {
        shim_dir,
        in_current_path,
        in_user_path,
        installed_shims,
    }
}

pub fn install_shims(
    source_shim: &Path,
    channel_reg: Option<(Channel, &Path, Option<&str>, Option<&str>)>,
) -> Result<InstallReport, String> {
    if !source_shim.is_file() {
        return Err(format!(
            "Source shim binary not found at {}",
            source_shim.display()
        ));
    }

    let shim_dir = get_shim_dir();
    std::fs::create_dir_all(&shim_dir).map_err(|e| {
        format!(
            "Failed to create shim directory {}: {}",
            shim_dir.display(),
            e
        )
    })?;

    cleanup_orphan_shims(&shim_dir);
    let mut created_shims = Vec::new();

    #[cfg(windows)]
    {
        for name in shim_binary_names() {
            let exe_name = format!("{}.exe", name);
            let dest = shim_dir.join(&exe_name);

            // Copy with version precedence / change check
            let need_copy = if dest.is_file() {
                should_update_file(source_shim, &dest)
            } else {
                true
            };

            if need_copy {
                copy_file_atomic(source_shim, &dest)?;
            }
            created_shims.push(exe_name);
        }
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let main_shim = shim_dir.join("sona-shim");
        let need_copy = if main_shim.is_file() {
            source_shim != &main_shim && should_update_file(source_shim, &main_shim)
        } else {
            true
        };
        if need_copy && source_shim != &main_shim {
            let _ = std::fs::remove_file(&main_shim);
            std::fs::copy(source_shim, &main_shim)
                .map_err(|e| format!("Failed to copy to {}: {}", main_shim.display(), e))?;
            #[cfg(target_family = "unix")]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ =
                    std::fs::set_permissions(&main_shim, std::fs::Permissions::from_mode(0o755));
            }
        }
        created_shims.push("sona-shim".to_string());

        for name in shim_binary_names() {
            if *name == "sona-shim" {
                continue;
            }
            let link_path = shim_dir.join(name);
            let _ = std::fs::remove_file(&link_path);
            if symlink("sona-shim", &link_path).is_ok() {
                created_shims.push(name.to_string());
            }
        }
    }

    // Register channel if provided
    let mut reg_str = None;
    if let Some((ch, inst_path, ver, exe)) = channel_reg {
        register_channel(ch, inst_path, ver, exe)?;
        reg_str = Some(ch.as_str().to_string());
    }

    // Add shim_dir to user PATH
    let updated_path = add_to_user_path(&shim_dir)?;

    Ok(InstallReport {
        shim_dir,
        created_shims,
        updated_path,
        registered_channel: reg_str,
    })
}

pub fn uninstall_shims(
    channel_to_remove: Option<Channel>,
    force: bool,
) -> Result<UninstallReport, String> {
    let shim_dir = get_shim_dir();
    let mut removed_channel_name = None;

    if let Some(ch) = channel_to_remove {
        unregister_channel(ch)?;
        removed_channel_name = Some(ch.as_str().to_string());

        if !force {
            // Reference counting check
            let other_ch = match ch {
                Channel::Stable => Channel::Nightly,
                Channel::Nightly => Channel::Stable,
            };

            let other_exists =
                get_registered_channel(other_ch).is_some() || is_channel_installed(other_ch);

            if other_exists {
                // Smoothly switch default to the remaining channel if needed
                let _ = set_default_channel(other_ch);
                return Ok(UninstallReport {
                    removed_channel: removed_channel_name,
                    shims_removed: false,
                    path_removed: false,
                    active_remaining_channel: Some(other_ch.as_str().to_string()),
                });
            }
        }
    } else if !force {
        let stable_exists = get_registered_channel(Channel::Stable).is_some()
            || is_channel_installed(Channel::Stable);
        let nightly_exists = get_registered_channel(Channel::Nightly).is_some()
            || is_channel_installed(Channel::Nightly);
        if stable_exists || nightly_exists {
            return Err("Cannot remove shims while channels remain installed. Specify --channel <name> or use --force.".to_string());
        }
    }

    // Completely remove shims and PATH entry
    let mut shims_removed = false;
    if shim_dir.exists() {
        for name in shim_binary_names() {
            let exe_name = if cfg!(windows) {
                format!("{}.exe", name)
            } else {
                name.to_string()
            };
            let p = shim_dir.join(&exe_name);
            if p.is_symlink() || p.exists() {
                if std::fs::remove_file(&p).is_ok() {
                    shims_removed = true;
                } else {
                    #[cfg(windows)]
                    {
                        // Running/locked binary on Windows: rename to .old to vacate active name
                        let old_name = format!("{}.old.{}", exe_name, std::process::id());
                        let old_path = shim_dir.join(old_name);
                        if std::fs::rename(&p, &old_path).is_ok() {
                            let _ = std::fs::remove_file(&old_path);
                            shims_removed = true;
                        }
                    }
                }
            }
        }
        cleanup_orphan_shims(&shim_dir);
        // Remove shim_dir if empty
        let _ = std::fs::remove_dir(&shim_dir);
    }

    let path_removed = remove_from_user_path(&shim_dir)?;

    Ok(UninstallReport {
        removed_channel: removed_channel_name,
        shims_removed,
        path_removed,
        active_remaining_channel: None,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub fn normalize_path(p: &Path) -> String {
    let s = p.to_string_lossy().trim().trim_matches('"').to_lowercase();
    if cfg!(windows) {
        let unified = s.replace('/', "\\");
        unified.trim_end_matches('\\').to_string()
    } else {
        let unified = s.replace('\\', "/");
        unified.trim_end_matches('/').to_string()
    }
}

pub fn cleanup_orphan_shims(shim_dir: &Path) {
    if let Ok(entries) = std::fs::read_dir(shim_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(file_name) = path.file_name().and_then(|n| n.to_str())
                && (file_name.contains(".old.") || file_name.contains(".tmp."))
            {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
}

fn should_update_file(source: &Path, dest: &Path) -> bool {
    let Ok(src_meta) = source.metadata() else {
        return false;
    };
    let Ok(dst_meta) = dest.metadata() else {
        return true;
    };

    if src_meta.len() != dst_meta.len() {
        return true;
    }

    // Direct byte comparison: shims are compact binaries (~1-2MB), byte check takes <1ms and avoids mtime drift
    let Ok(src_bytes) = std::fs::read(source) else {
        return false;
    };
    let Ok(dst_bytes) = std::fs::read(dest) else {
        return true;
    };

    src_bytes != dst_bytes
}

#[cfg(windows)]
fn copy_file_atomic(source: &Path, dest: &Path) -> Result<(), String> {
    if source == dest {
        return Ok(());
    }
    if let Some(parent) = dest.parent() {
        cleanup_orphan_shims(parent);
    }
    let temp_name = format!(
        "{}.tmp.{}.{}",
        dest.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    let temp_path = dest.with_file_name(temp_name);

    std::fs::copy(source, &temp_path).map_err(|e| {
        format!(
            "Failed to copy to temporary file {}: {}",
            temp_path.display(),
            e
        )
    })?;

    if dest.exists() {
        let old_name = format!(
            "{}.old.{}.{}",
            dest.file_name().unwrap_or_default().to_string_lossy(),
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        let old_path = dest.with_file_name(old_name);
        if std::fs::rename(dest, &old_path).is_ok() {
            let _ = std::fs::remove_file(&old_path);
        } else {
            let _ = std::fs::remove_file(dest);
        }
    }
    let res = std::fs::rename(&temp_path, dest).or_else(|_| {
        std::fs::copy(&temp_path, dest)
            .map(|_| ())
            .map_err(|e| format!("Copy failed: {}", e))
    });

    let _ = std::fs::remove_file(&temp_path);
    res
}

fn check_user_env_has_path(dir: &Path) -> bool {
    let target = normalize_path(dir);

    #[cfg(windows)]
    {
        use crate::registry::{win_expand_environment_strings, win_read_registry_string_raw};
        if let Some(user_path) = win_read_registry_string_raw("Environment", "Path") {
            let expanded_path = win_expand_environment_strings(&user_path);
            for entry in expanded_path.split(';') {
                let trimmed = entry.trim();
                if !trimmed.is_empty() && normalize_path(Path::new(trimmed)) == target {
                    return true;
                }
            }
        }
    }

    #[cfg(unix)]
    {
        if let Ok(home) = std::env::var("HOME") {
            let home_path = PathBuf::from(&home);
            let dir_str = dir.to_string_lossy();
            let export_path = dir
                .strip_prefix(&home_path)
                .ok()
                .map(|rel| format!("$HOME/{}", rel.display()));
            for p in get_unix_shell_rc_paths(&home_path) {
                if let Ok(content) = std::fs::read_to_string(&p) {
                    if content.contains(".sona/bin")
                        || content.contains(&*dir_str)
                        || export_path
                            .as_deref()
                            .map(|ep| content.contains(ep))
                            .unwrap_or(false)
                    {
                        return true;
                    }
                }
            }
        }
    }

    false
}

#[cfg(unix)]
fn get_unix_shell_rc_paths(home: &Path) -> Vec<PathBuf> {
    let mut targets = Vec::new();
    let zshrc = home.join(".zshrc");
    let zprofile = home.join(".zprofile");
    let bashrc = home.join(".bashrc");
    let profile = home.join(".profile");

    let is_zsh = std::env::var("SHELL")
        .map(|s| s.ends_with("zsh"))
        .unwrap_or(cfg!(target_os = "macos"));

    if is_zsh {
        targets.push(zshrc);
        if cfg!(target_os = "macos") && zprofile.exists() {
            targets.push(zprofile);
        }
    }
    if bashrc.exists() {
        targets.push(bashrc);
    }
    if profile.exists() {
        targets.push(profile);
    }

    if targets.is_empty() {
        if cfg!(target_os = "macos") {
            targets.push(home.join(".zshrc"));
        } else {
            targets.push(home.join(".profile"));
        }
    }

    targets
}

fn add_to_user_path(dir: &Path) -> Result<bool, String> {
    if std::env::var("SONA_TEST_ISOLATED").ok().as_deref() == Some("1") {
        return Ok(true);
    }
    #[cfg(windows)]
    if check_user_env_has_path(dir) {
        return Ok(false);
    }

    #[cfg(windows)]
    {
        use crate::registry::{win_read_registry_string_raw, win_write_registry_expand_string};
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE,
        };

        // Read RAW unexpanded string to preserve dynamic variables like %USERPROFILE% or %JAVA_HOME%
        let current_raw_path =
            win_read_registry_string_raw("Environment", "Path").unwrap_or_default();
        let new_path = if current_raw_path.trim().is_empty() {
            dir.to_string_lossy().to_string()
        } else {
            format!(
                "{};{}",
                current_raw_path.trim_end_matches(';'),
                dir.to_string_lossy()
            )
        };
        win_write_registry_expand_string("Environment", "Path", &new_path)?;
        // Broadcast environment change notification
        let env_w: Vec<u16> = "Environment\0".encode_utf16().collect();
        let mut result: usize = 0;
        unsafe {
            SendMessageTimeoutW(
                HWND_BROADCAST,
                WM_SETTINGCHANGE,
                0,
                env_w.as_ptr() as isize,
                SMTO_ABORTIFHUNG,
                3000,
                &mut result,
            );
        }
        Ok(true)
    }
    #[cfg(unix)]
    {
        let home = std::env::var("HOME")
            .map_err(|_| "HOME environment variable is not set".to_string())?;
        let home_path = PathBuf::from(&home);
        let targets = get_unix_shell_rc_paths(&home_path);
        let export_path = if let Ok(rel) = dir.strip_prefix(&home_path) {
            format!("$HOME/{}", rel.display())
        } else {
            dir.display().to_string()
        };
        let export_block = format!(
            "\n# Sona CLI tools\nexport PATH=\"{}:$PATH\"\n",
            export_path
        );
        let dir_str = dir.to_string_lossy();
        use std::io::Write;
        let mut modified = false;

        for rc_path in targets {
            let already_has = std::fs::read_to_string(&rc_path)
                .map(|c| {
                    c.contains(&export_path) || c.contains(&*dir_str) || c.contains(".sona/bin")
                })
                .unwrap_or(false);
            if !already_has {
                if let Ok(mut file) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&rc_path)
                {
                    if file.write_all(export_block.as_bytes()).is_ok() {
                        modified = true;
                    }
                }
            }
        }
        Ok(modified)
    }
    #[cfg(not(any(windows, unix)))]
    {
        Ok(false)
    }
}
fn remove_from_user_path(dir: &Path) -> Result<bool, String> {
    if std::env::var("SONA_TEST_ISOLATED").ok().as_deref() == Some("1") {
        return Ok(true);
    }
    let target = normalize_path(dir);

    #[cfg(windows)]
    {
        use crate::registry::{
            win_expand_environment_strings, win_read_registry_string_raw,
            win_write_registry_expand_string,
        };
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE,
        };

        let Some(current_raw_path) = win_read_registry_string_raw("Environment", "Path") else {
            return Ok(false);
        };

        let mut changed = false;
        let mut new_entries = Vec::new();

        for entry in current_raw_path.split(';') {
            let trimmed = entry.trim();
            if trimmed.is_empty() {
                continue;
            }
            // Expand entry ONLY for comparison with target
            let expanded = win_expand_environment_strings(trimmed);
            if normalize_path(Path::new(trimmed)) == target
                || normalize_path(Path::new(&expanded)) == target
            {
                changed = true;
            } else {
                // Retain raw unexpanded entry to protect %USERPROFILE%, %JAVA_HOME%, etc.
                new_entries.push(trimmed);
            }
        }

        if changed {
            if new_entries.is_empty() {
                use crate::registry::win_delete_registry_value;
                let _ = win_delete_registry_value("Environment", "Path");
            } else {
                let new_path = new_entries.join(";");
                win_write_registry_expand_string("Environment", "Path", &new_path)?;
            }
            let env_w: Vec<u16> = "Environment\0".encode_utf16().collect();
            let mut result: usize = 0;
            unsafe {
                SendMessageTimeoutW(
                    HWND_BROADCAST,
                    WM_SETTINGCHANGE,
                    0,
                    env_w.as_ptr() as isize,
                    SMTO_ABORTIFHUNG,
                    3000,
                    &mut result,
                );
            }
            return Ok(true);
        }
        Ok(false)
    }

    #[cfg(unix)]
    {
        let _ = target;
        let mut modified = false;
        if let Ok(home) = std::env::var("HOME") {
            let home_path = PathBuf::from(&home);
            let dir_str = dir.to_string_lossy();
            let export_path = dir
                .strip_prefix(&home_path)
                .ok()
                .map(|rel| format!("$HOME/{}", rel.display()));
            let candidates = [
                home_path.join(".bashrc"),
                home_path.join(".zshrc"),
                home_path.join(".zprofile"),
                home_path.join(".profile"),
            ];
            for path in &candidates {
                if let Ok(content) = std::fs::read_to_string(path) {
                    let matches_path = |line: &str| {
                        line.contains(".sona/bin")
                            || line.contains(&*dir_str)
                            || export_path
                                .as_deref()
                                .map(|ep| line.contains(ep))
                                .unwrap_or(false)
                    };
                    if content.lines().any(matches_path) {
                        let cleaned_lines: Vec<&str> = content
                            .lines()
                            .filter(|line| {
                                !matches_path(line) && !line.contains("# Sona CLI tools")
                            })
                            .collect();
                        let mut new_content = cleaned_lines.join("\n");
                        if !new_content.is_empty() && content.ends_with('\n') {
                            new_content.push('\n');
                        }
                        if std::fs::write(path, new_content).is_ok() {
                            modified = true;
                        }
                    }
                }
            }
        }
        Ok(modified)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path() {
        if cfg!(windows) {
            assert_eq!(
                normalize_path(Path::new("C:/Users/test/bin/")),
                "c:\\users\\test\\bin"
            );
            assert_eq!(
                normalize_path(Path::new("\"C:\\Users\\test\\bin\\\\\"")),
                "c:\\users\\test\\bin"
            );
        } else {
            assert_eq!(
                normalize_path(Path::new("/home/test/bin/")),
                "/home/test/bin"
            );
        }
    }

    #[test]
    fn test_cleanup_orphan_shims() {
        let temp_dir = std::env::temp_dir().join(format!("sona_gc_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let live_file = temp_dir.join("sona-cli.exe");
        let old_file = temp_dir.join("sona-cli.exe.old.12345");
        let tmp_file = temp_dir.join("sona-mcp.exe.tmp.67890");
        let doc_file = temp_dir.join("readme.txt");

        std::fs::write(&live_file, b"live").unwrap();
        std::fs::write(&old_file, b"old").unwrap();
        std::fs::write(&tmp_file, b"tmp").unwrap();
        std::fs::write(&doc_file, b"doc").unwrap();

        cleanup_orphan_shims(&temp_dir);

        assert!(live_file.exists());
        assert!(doc_file.exists());
        assert!(!old_file.exists());
        assert!(!tmp_file.exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_should_update_file_byte_comparison() {
        let temp_dir =
            std::env::temp_dir().join(format!("sona_update_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let file_a = temp_dir.join("a.bin");
        let file_b = temp_dir.join("b.bin");
        let file_c = temp_dir.join("c.bin");

        std::fs::write(&file_a, b"identical content").unwrap();
        std::fs::write(&file_b, b"identical content").unwrap();
        std::fs::write(&file_c, b"different content").unwrap();

        // Identical content -> should NOT update
        assert!(!should_update_file(&file_a, &file_b));

        // Same length, different content -> SHOULD update
        assert!(should_update_file(&file_a, &file_c));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
