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
            should_update_file(source_shim, &main_shim)
        } else {
            true
        };
        if need_copy {
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
            if !link_path.exists() {
                let _ = symlink("sona-shim", &link_path);
            }
            created_shims.push(name.to_string());
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
                let _ = std::fs::remove_file(&p);
                shims_removed = true;
            }
        }
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

fn normalize_path(p: &Path) -> String {
    let s = p.to_string_lossy().to_lowercase();
    s.trim_end_matches(['\\', '/']).to_string()
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

    match (src_meta.modified(), dst_meta.modified()) {
        (Ok(s_mod), Ok(d_mod)) => s_mod > d_mod,
        _ => false,
    }
}

#[cfg(windows)]
fn copy_file_atomic(source: &Path, dest: &Path) -> Result<(), String> {
    if source == dest {
        return Ok(());
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

    let res = (|| {
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
        std::fs::rename(&temp_path, dest).or_else(|_| {
            std::fs::copy(&temp_path, dest)
                .map(|_| ())
                .map_err(|e| format!("Copy failed: {}", e))
        })
    })();

    let _ = std::fs::remove_file(&temp_path);
    res
}

fn check_user_env_has_path(dir: &Path) -> bool {
    let target = normalize_path(dir);

    #[cfg(windows)]
    {
        use crate::registry::{win_expand_environment_strings, win_read_registry_string};
        if let Some(user_path) = win_read_registry_string("Environment", "Path") {
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
            let files = [".bashrc", ".zshrc", ".profile"];
            for f in &files {
                let p = PathBuf::from(&home).join(f);
                if let Ok(content) = std::fs::read_to_string(&p) {
                    if content.contains(".sona/bin") {
                        return true;
                    }
                }
            }
        }
    }

    false
}

fn add_to_user_path(dir: &Path) -> Result<bool, String> {
    if std::env::var("SONA_TEST_ISOLATED").ok().as_deref() == Some("1") {
        return Ok(true);
    }
    if check_user_env_has_path(dir) {
        return Ok(false);
    }

    #[cfg(windows)]
    {
        use crate::registry::{win_read_registry_string, win_write_registry_expand_string};
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE,
        };

        let current_path = match win_read_registry_string("Environment", "Path") {
            Some(p) => p,
            None => {
                if crate::registry::win_registry_value_exists("Environment", "Path") {
                    return Err("Failed to read user PATH from Windows registry".to_string());
                }
                String::new()
            }
        };
        let new_path = if current_path.trim().is_empty() {
            dir.to_string_lossy().to_string()
        } else {
            format!(
                "{};{}",
                current_path.trim_end_matches(';'),
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
        if let Ok(home) = std::env::var("HOME") {
            let rc_path = PathBuf::from(&home).join(".profile");
            let line = "\nexport PATH=\"$HOME/.sona/bin:$PATH\"\n";
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&rc_path)
                .map_err(|e| format!("Failed to open {}: {}", rc_path.display(), e))?;
            file.write_all(line.as_bytes())
                .map_err(|e| format!("Failed to write {}: {}", rc_path.display(), e))?;
            return Ok(true);
        }
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
            win_expand_environment_strings, win_read_registry_string,
            win_write_registry_expand_string,
        };
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE,
        };

        let Some(current_path) = win_read_registry_string("Environment", "Path") else {
            return Ok(false);
        };

        let mut changed = false;
        let mut new_entries = Vec::new();

        for entry in current_path.split(';') {
            let trimmed = entry.trim();
            if trimmed.is_empty() {
                continue;
            }
            let expanded = win_expand_environment_strings(trimmed);
            if normalize_path(Path::new(trimmed)) == target
                || normalize_path(Path::new(&expanded)) == target
            {
                changed = true;
            } else {
                new_entries.push(trimmed);
            }
        }

        if changed {
            let new_path = new_entries.join(";");
            win_write_registry_expand_string("Environment", "Path", &new_path)?;
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
        // For Unix, removing from shell profiles is best-effort
        let _ = target;
        Ok(true)
    }
}
