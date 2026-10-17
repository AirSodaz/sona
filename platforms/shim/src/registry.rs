use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    Stable,
    Nightly,
}

impl Channel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Channel::Stable => "stable",
            Channel::Nightly => "nightly",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        let trimmed = s.trim().to_lowercase();
        if trimmed == "stable" {
            Some(Channel::Stable)
        } else if trimmed == "nightly" {
            Some(Channel::Nightly)
        } else {
            None
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Channel::Stable => "Sona Stable",
            Channel::Nightly => "Sona Nightly",
        }
    }

    pub fn default_ipc_endpoint(&self) -> String {
        #[cfg(windows)]
        {
            match self {
                Channel::Stable => r"\\.\pipe\sona-agent-ipc".to_string(),
                Channel::Nightly => r"\\.\pipe\sona-agent-ipc-nightly".to_string(),
            }
        }
        #[cfg(unix)]
        {
            if let Some(runtime_dir) = std::env::var("XDG_RUNTIME_DIR")
                .ok()
                .filter(|d| !d.trim().is_empty())
            {
                let socket_name = match self {
                    Channel::Stable => "sona-agent.sock",
                    Channel::Nightly => "sona-agent-nightly.sock",
                };
                return PathBuf::from(runtime_dir)
                    .join(socket_name)
                    .to_string_lossy()
                    .to_string();
            }
            let home = std::env::var("HOME")
                .ok()
                .filter(|h| !h.trim().is_empty())
                .unwrap_or_else(|| ".".to_string());
            let dir_name = match self {
                Channel::Stable => "sona",
                Channel::Nightly => "sona-nightly",
            };
            PathBuf::from(home)
                .join(".local")
                .join("share")
                .join(dir_name)
                .join("agent.sock")
                .to_string_lossy()
                .to_string()
        }
        #[cfg(not(any(windows, unix)))]
        {
            "unsupported".to_string()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetRole {
    Cli,
    Mcp,
    Shim,
}

impl TargetRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            TargetRole::Cli => "cli",
            TargetRole::Mcp => "mcp",
            TargetRole::Shim => "shim",
        }
    }

    pub fn binary_name(&self) -> &'static str {
        #[cfg(windows)]
        {
            match self {
                TargetRole::Cli => "sona-cli.exe",
                TargetRole::Mcp => "sona-mcp.exe",
                TargetRole::Shim => "sona-shim.exe",
            }
        }
        #[cfg(not(windows))]
        {
            match self {
                TargetRole::Cli => "sona-cli",
                TargetRole::Mcp => "sona-mcp",
                TargetRole::Shim => "sona-shim",
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelInfo {
    pub channel: Channel,
    pub install_path: PathBuf,
    pub cli_path: Option<PathBuf>,
    pub mcp_path: Option<PathBuf>,
    pub version: Option<String>,
    pub executable: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RegistryFile {
    #[serde(default)]
    pub channels: HashMap<String, RegistryChannelEntry>,
    #[serde(default)]
    pub default_channel: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryChannelEntry {
    pub install_path: String,
    pub cli_path: Option<String>,
    pub mcp_path: Option<String>,
    pub version: Option<String>,
    pub executable: Option<String>,
}

pub fn get_sona_dir() -> PathBuf {
    if let Some(custom) = std::env::var("SONA_DIR")
        .ok()
        .filter(|s| !s.trim().is_empty())
    {
        return PathBuf::from(custom);
    }
    #[cfg(windows)]
    {
        if let Some(profile) = std::env::var("USERPROFILE")
            .ok()
            .filter(|s| !s.trim().is_empty())
        {
            PathBuf::from(profile).join(".sona")
        } else if let Some(appdata) = std::env::var("LOCALAPPDATA")
            .ok()
            .filter(|s| !s.trim().is_empty())
        {
            PathBuf::from(appdata).join("sona")
        } else {
            PathBuf::from(r"C:\Users\Default\.sona")
        }
    }
    #[cfg(unix)]
    {
        if let Ok(home) = std::env::var("HOME").filter(|h| !h.trim().is_empty()) {
            PathBuf::from(home).join(".sona")
        } else if let Ok(xdg) = std::env::var("XDG_DATA_HOME").filter(|x| !x.trim().is_empty()) {
            PathBuf::from(xdg).join("sona")
        } else {
            PathBuf::from(".sona")
        }
    }
    #[cfg(not(any(windows, unix)))]
    {
        PathBuf::from(".sona")
    }
}

pub fn get_registry_json_path() -> PathBuf {
    get_sona_dir().join("registry.json")
}

// ---------------------------------------------------------------------------
// Channel Registry: Reading and Writing
// ---------------------------------------------------------------------------

#[cfg(windows)]
pub fn get_registry_base_subkey() -> String {
    if let Some(custom) = std::env::var("SONA_REGISTRY_SUBKEY")
        .ok()
        .filter(|s| !s.trim().is_empty())
    {
        return custom;
    }
    r"Software\Sona".to_string()
}

pub fn get_default_channel() -> Channel {
    #[cfg(windows)]
    {
        let settings_key = format!(r"{}\Settings", get_registry_base_subkey());
        if let Some(c) = win_read_registry_string(&settings_key, "DefaultChannel")
            .and_then(|val| Channel::parse(&val))
        {
            return c;
        }
    }

    // Unix or fallback to JSON registry
    if let Some(c) = read_registry_file()
        .ok()
        .and_then(|file| file.default_channel)
        .and_then(|def| Channel::parse(&def))
    {
        return c;
    }
    Channel::Stable
}

pub fn set_default_channel(channel: Channel) -> Result<(), String> {
    #[cfg(windows)]
    {
        let settings_key = format!(r"{}\Settings", get_registry_base_subkey());
        win_write_registry_string(&settings_key, "DefaultChannel", channel.as_str())?;
    }
    let mut file = read_registry_file()?;
    file.default_channel = Some(channel.as_str().to_string());
    write_registry_file(&file)
}

pub fn get_registered_channel(channel: Channel) -> Option<ChannelInfo> {
    #[cfg(windows)]
    {
        let subkey = format!(
            r"{}\Channels\{}",
            get_registry_base_subkey(),
            channel.as_str()
        );
        if let Some(install_path_str) = win_read_registry_string(&subkey, "InstallPath") {
            let install_path = PathBuf::from(install_path_str);
            let version = win_read_registry_string(&subkey, "Version");
            let executable = win_read_registry_string(&subkey, "Executable");

            let cli = resolve_executable_in_dir(&install_path, TargetRole::Cli);
            let mcp = resolve_executable_in_dir(&install_path, TargetRole::Mcp);

            return Some(ChannelInfo {
                channel,
                install_path,
                cli_path: cli,
                mcp_path: mcp,
                version,
                executable,
            });
        }
    }

    if let Some(entry) = read_registry_file()
        .ok()
        .and_then(|file| file.channels.get(channel.as_str()).cloned())
    {
        let install_path = PathBuf::from(&entry.install_path);
        let cli = entry
            .cli_path
            .as_ref()
            .map(PathBuf::from)
            .or_else(|| resolve_executable_in_dir(&install_path, TargetRole::Cli));
        let mcp = entry
            .mcp_path
            .as_ref()
            .map(PathBuf::from)
            .or_else(|| resolve_executable_in_dir(&install_path, TargetRole::Mcp));

        return Some(ChannelInfo {
            channel,
            install_path,
            cli_path: cli,
            mcp_path: mcp,
            version: entry.version,
            executable: entry.executable,
        });
    }

    None
}

pub fn register_channel(
    channel: Channel,
    install_path: &Path,
    version: Option<&str>,
    executable: Option<&str>,
) -> Result<(), String> {
    let install_path_str = install_path.to_string_lossy().to_string();

    #[cfg(windows)]
    {
        let subkey = format!(
            r"{}\Channels\{}",
            get_registry_base_subkey(),
            channel.as_str()
        );
        win_write_registry_string(&subkey, "InstallPath", &install_path_str)?;
        if let Some(ver) = version {
            win_write_registry_string(&subkey, "Version", ver)?;
        }
        if let Some(exe) = executable {
            win_write_registry_string(&subkey, "Executable", exe)?;
        }
    }

    let mut file = read_registry_file()?;
    let cli = resolve_executable_in_dir(install_path, TargetRole::Cli)
        .map(|p| p.to_string_lossy().to_string());
    let mcp = resolve_executable_in_dir(install_path, TargetRole::Mcp)
        .map(|p| p.to_string_lossy().to_string());

    file.channels.insert(
        channel.as_str().to_string(),
        RegistryChannelEntry {
            install_path: install_path_str,
            cli_path: cli,
            mcp_path: mcp,
            version: version.map(|s| s.to_string()),
            executable: executable.map(|s| s.to_string()),
        },
    );

    write_registry_file(&file)
}

pub fn unregister_channel(channel: Channel) -> Result<(), String> {
    #[cfg(windows)]
    {
        let subkey = format!(
            r"{}\Channels\{}",
            get_registry_base_subkey(),
            channel.as_str()
        );
        let _ = win_delete_registry_key(&subkey);
    }

    let mut file = read_registry_file()?;
    file.channels.remove(channel.as_str());

    let is_current_default = get_default_channel() == channel
        || file.default_channel.as_deref() == Some(channel.as_str());

    if is_current_default {
        let other = match channel {
            Channel::Stable => Channel::Nightly,
            Channel::Nightly => Channel::Stable,
        };
        let other_still_registered =
            file.channels.contains_key(other.as_str()) || is_channel_installed(other);
        if other_still_registered {
            file.default_channel = Some(other.as_str().to_string());
            #[cfg(windows)]
            {
                let settings_key = format!(r"{}\Settings", get_registry_base_subkey());
                let _ = win_write_registry_string(&settings_key, "DefaultChannel", other.as_str());
            }
        } else {
            file.default_channel = None;
            #[cfg(windows)]
            {
                let settings_key = format!(r"{}\Settings", get_registry_base_subkey());
                let _ = win_delete_registry_value(&settings_key, "DefaultChannel");
            }
        }
    }
    write_registry_file(&file)
}

// ---------------------------------------------------------------------------
// JSON File Helpers
// ---------------------------------------------------------------------------

fn read_registry_file() -> Result<RegistryFile, String> {
    let path = get_registry_json_path();
    if !path.exists() {
        return Ok(RegistryFile::default());
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
    if content.trim().is_empty() {
        return Ok(RegistryFile::default());
    }
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse {}: {}", path.display(), e))
}

fn write_registry_file(file: &RegistryFile) -> Result<(), String> {
    let path = get_registry_json_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create {}: {}", parent.display(), e))?;
    }
    let content = serde_json::to_string_pretty(file)
        .map_err(|e| format!("Failed to serialize registry: {}", e))?;
    let temp_name = format!(
        "registry.json.tmp.{}.{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    let temp_path = path.with_file_name(temp_name);
    std::fs::write(&temp_path, &content)
        .map_err(|e| format!("Failed to write {}: {}", temp_path.display(), e))?;
    std::fs::rename(&temp_path, &path)
        .or_else(|_| {
            let _ = std::fs::remove_file(&path);
            std::fs::rename(&temp_path, &path)
        })
        .map_err(|e| {
            let _ = std::fs::remove_file(&temp_path);
            format!("Failed to commit {}: {}", path.display(), e)
        })
}

// ---------------------------------------------------------------------------
// Executable Resolution and Discovery Chain
// ---------------------------------------------------------------------------

pub fn resolve_executable_in_dir(dir: &Path, role: TargetRole) -> Option<PathBuf> {
    let name = role.binary_name();

    // 1. Direct in dir (Windows standard: %LOCALAPPDATA%\Programs\Sona\sona-cli.exe)
    let direct = dir.join(name);
    if direct.is_file() {
        return Some(direct);
    }

    // 2. macOS .app bundle structure: <App>/Contents/MacOS/<binary>
    let macos_path = dir.join("Contents").join("MacOS").join(name);
    if macos_path.is_file() {
        return Some(macos_path);
    }

    // 3. Linux bin subdir: <Dir>/bin/<binary>
    let bin_path = dir.join("bin").join(name);
    if bin_path.is_file() {
        return Some(bin_path);
    }

    None
}

pub fn get_heuristic_locations(channel: Channel) -> Vec<PathBuf> {
    let mut locs = Vec::new();

    #[cfg(windows)]
    {
        let folder_name = match channel {
            Channel::Stable => "Sona",
            Channel::Nightly => "Sona-Nightly",
        };
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            locs.push(
                PathBuf::from(local_app_data)
                    .join("Programs")
                    .join(folder_name),
            );
        }
        if let Ok(prog_files) = std::env::var("ProgramFiles") {
            locs.push(PathBuf::from(prog_files).join(folder_name));
        }
        if let Ok(prog_files_x86) = std::env::var("ProgramFiles(x86)") {
            locs.push(PathBuf::from(prog_files_x86).join(folder_name));
        }
    }

    #[cfg(target_os = "macos")]
    {
        let app_name = match channel {
            Channel::Stable => "Sona.app",
            Channel::Nightly => "Sona-Nightly.app",
        };
        locs.push(PathBuf::from("/Applications").join(app_name));
        if let Ok(home) = std::env::var("HOME") {
            locs.push(PathBuf::from(home).join("Applications").join(app_name));
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let lib_name = match channel {
            Channel::Stable => "sona",
            Channel::Nightly => "sona-nightly",
        };
        locs.push(PathBuf::from("/usr/lib").join(lib_name));
        locs.push(PathBuf::from("/opt").join(lib_name));
        if let Ok(home) = std::env::var("HOME") {
            locs.push(
                PathBuf::from(home)
                    .join(".local")
                    .join("lib")
                    .join(lib_name),
            );
        }
    }

    locs
}

pub fn find_executable(channel: Channel, role: TargetRole) -> Option<PathBuf> {
    // Level 1: Channel Registry
    if let Some(info) = get_registered_channel(channel) {
        let candidate = match role {
            TargetRole::Cli => info.cli_path,
            TargetRole::Mcp => info.mcp_path,
            TargetRole::Shim => None,
        };
        if let Some(path) = candidate.filter(|p| p.is_file()) {
            return Some(path);
        }
        if let Some(path) = resolve_executable_in_dir(&info.install_path, role) {
            return Some(path);
        }
    }

    // Level 2: Heuristic Paths
    for loc in get_heuristic_locations(channel) {
        if let Some(path) = resolve_executable_in_dir(&loc, role) {
            return Some(path);
        }
    }

    // Level 3: macOS LaunchServices dynamic bundle search
    #[cfg(target_os = "macos")]
    {
        let bundle_id = match channel {
            Channel::Stable => "com.asoda.sona",
            Channel::Nightly => "com.asoda.sona.nightly",
        };
        if let Ok(output) = std::process::Command::new("mdfind")
            .arg(format!("kMDItemCFBundleIdentifier == '{}'", bundle_id))
            .output()
        {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                for line in text.lines() {
                    let p = PathBuf::from(line.trim());
                    if p.exists() {
                        if let Some(exe) = resolve_executable_in_dir(&p, role) {
                            return Some(exe);
                        }
                    }
                }
            }
        }
    }

    None
}

pub fn is_channel_installed(channel: Channel) -> bool {
    find_executable(channel, TargetRole::Cli).is_some()
        || find_executable(channel, TargetRole::Mcp).is_some()
}

// ---------------------------------------------------------------------------
// IPC Liveness Probe
// ---------------------------------------------------------------------------

pub fn is_channel_ipc_alive(channel: Channel) -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{
            ERROR_FILE_NOT_FOUND, ERROR_PIPE_BUSY, ERROR_SEM_TIMEOUT, GetLastError,
        };
        use windows_sys::Win32::System::Pipes::WaitNamedPipeW;

        let pipe_name = channel.default_ipc_endpoint();
        let wide: Vec<u16> = pipe_name.encode_utf16().chain(std::iter::once(0)).collect();

        unsafe {
            const NMPWAIT_NOWAIT: u32 = 1;
            let res = WaitNamedPipeW(wide.as_ptr(), NMPWAIT_NOWAIT);
            if res != 0 {
                return true;
            }
            let err = GetLastError();
            // If the pipe is busy or timed out, it exists and has a listener!
            if err == ERROR_PIPE_BUSY || err == ERROR_SEM_TIMEOUT {
                return true;
            }
            if err == ERROR_FILE_NOT_FOUND {
                return false;
            }
            false
        }
    }

    #[cfg(unix)]
    {
        use std::os::unix::net::UnixStream;
        let endpoint = channel.default_ipc_endpoint();
        let path = PathBuf::from(endpoint);
        if !path.exists() {
            return false;
        }
        UnixStream::connect(path).is_ok()
    }

    #[cfg(not(any(windows, unix)))]
    {
        false
    }
}

pub fn detect_active_ipc_channel() -> Option<Channel> {
    let nightly_alive = is_channel_ipc_alive(Channel::Nightly);
    let stable_alive = is_channel_ipc_alive(Channel::Stable);

    match (stable_alive, nightly_alive) {
        (true, false) => Some(Channel::Stable),
        (false, true) => Some(Channel::Nightly),
        (true, true) => Some(get_default_channel()),
        (false, false) => None,
    }
}

// ---------------------------------------------------------------------------
// Win32 Registry Helpers
// ---------------------------------------------------------------------------

#[cfg(windows)]
fn to_wide_null(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
pub fn win_read_registry_raw(subkey: &str, value_name: &str) -> Option<(String, u32)> {
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_READ, REG_EXPAND_SZ, REG_SZ, RegCloseKey, RegOpenKeyExW,
        RegQueryValueExW,
    };

    let subkey_w = to_wide_null(subkey);
    let value_w = to_wide_null(value_name);
    let mut hkey: HKEY = std::ptr::null_mut();

    unsafe {
        let status = RegOpenKeyExW(HKEY_CURRENT_USER, subkey_w.as_ptr(), 0, KEY_READ, &mut hkey);
        if status != ERROR_SUCCESS {
            return None;
        }

        let mut val_type: u32 = 0;
        let mut byte_count: u32 = 0;

        let status = RegQueryValueExW(
            hkey,
            value_w.as_ptr(),
            std::ptr::null_mut(),
            &mut val_type,
            std::ptr::null_mut(),
            &mut byte_count,
        );
        if status != ERROR_SUCCESS
            || (val_type != REG_SZ && val_type != REG_EXPAND_SZ)
            || byte_count == 0
        {
            RegCloseKey(hkey);
            return None;
        }

        let u16_count = (byte_count as usize).div_ceil(2);
        let mut buffer: Vec<u16> = vec![0; u16_count];
        let mut buffer_bytes = (u16_count * 2) as u32;

        let status = RegQueryValueExW(
            hkey,
            value_w.as_ptr(),
            std::ptr::null_mut(),
            &mut val_type,
            buffer.as_mut_ptr() as *mut u8,
            &mut buffer_bytes,
        );
        RegCloseKey(hkey);

        if status != ERROR_SUCCESS {
            return None;
        }

        while buffer.last() == Some(&0) {
            buffer.pop();
        }

        let raw_str = String::from_utf16(&buffer).ok()?;
        Some((raw_str, val_type))
    }
}

#[cfg(windows)]
pub fn win_read_registry_string_raw(subkey: &str, value_name: &str) -> Option<String> {
    win_read_registry_raw(subkey, value_name).map(|(s, _)| s)
}

#[cfg(windows)]
pub fn win_read_registry_string(subkey: &str, value_name: &str) -> Option<String> {
    use windows_sys::Win32::System::Registry::REG_EXPAND_SZ;
    let (raw_str, val_type) = win_read_registry_raw(subkey, value_name)?;
    if val_type == REG_EXPAND_SZ {
        Some(win_expand_environment_strings(&raw_str))
    } else {
        Some(raw_str)
    }
}
#[cfg(windows)]
pub fn win_registry_value_exists(subkey: &str, value_name: &str) -> bool {
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_READ, RegCloseKey, RegOpenKeyExW, RegQueryValueExW,
    };

    let subkey_w = to_wide_null(subkey);
    let value_w = to_wide_null(value_name);
    let mut hkey: HKEY = std::ptr::null_mut();

    unsafe {
        let status = RegOpenKeyExW(HKEY_CURRENT_USER, subkey_w.as_ptr(), 0, KEY_READ, &mut hkey);
        if status != ERROR_SUCCESS {
            return false;
        }

        let status = RegQueryValueExW(
            hkey,
            value_w.as_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        RegCloseKey(hkey);
        status == ERROR_SUCCESS
    }
}

#[cfg(windows)]
pub fn win_expand_environment_strings(s: &str) -> String {
    use windows_sys::Win32::System::Environment::ExpandEnvironmentStringsW;
    let wide_in = to_wide_null(s);
    unsafe {
        let req_size = ExpandEnvironmentStringsW(wide_in.as_ptr(), std::ptr::null_mut(), 0);
        if req_size == 0 {
            return s.to_string();
        }
        let mut buffer: Vec<u16> = vec![0; req_size as usize];
        let res = ExpandEnvironmentStringsW(wide_in.as_ptr(), buffer.as_mut_ptr(), req_size);
        if res == 0 {
            return s.to_string();
        }
        while buffer.last() == Some(&0) {
            buffer.pop();
        }
        String::from_utf16(&buffer).unwrap_or_else(|_| s.to_string())
    }
}

#[cfg(windows)]
pub fn win_write_registry_string(
    subkey: &str,
    value_name: &str,
    value: &str,
) -> Result<(), String> {
    win_write_registry_value(
        subkey,
        value_name,
        value,
        windows_sys::Win32::System::Registry::REG_SZ,
    )
}

#[cfg(windows)]
pub fn win_write_registry_expand_string(
    subkey: &str,
    value_name: &str,
    value: &str,
) -> Result<(), String> {
    win_write_registry_value(
        subkey,
        value_name,
        value,
        windows_sys::Win32::System::Registry::REG_EXPAND_SZ,
    )
}

#[cfg(windows)]
fn win_write_registry_value(
    subkey: &str,
    value_name: &str,
    value: &str,
    reg_type: u32,
) -> Result<(), String> {
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_WRITE, RegCloseKey, RegCreateKeyExW, RegSetValueExW,
    };

    let subkey_w = to_wide_null(subkey);
    let value_w = to_wide_null(value_name);
    let val_data_w = to_wide_null(value);
    let byte_count = (val_data_w.len() * 2) as u32;

    let mut hkey: HKEY = std::ptr::null_mut();
    let mut disposition: u32 = 0;

    unsafe {
        let status = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            subkey_w.as_ptr(),
            0,
            std::ptr::null_mut(),
            0,
            KEY_WRITE,
            std::ptr::null_mut(),
            &mut hkey,
            &mut disposition,
        );
        if status != ERROR_SUCCESS {
            return Err(format!("RegCreateKeyExW failed with code {}", status));
        }

        let status = RegSetValueExW(
            hkey,
            value_w.as_ptr(),
            0,
            reg_type,
            val_data_w.as_ptr() as *const u8,
            byte_count,
        );
        RegCloseKey(hkey);

        if status != ERROR_SUCCESS {
            return Err(format!("RegSetValueExW failed with code {}", status));
        }

        Ok(())
    }
}

#[cfg(windows)]
pub fn win_delete_registry_value(subkey: &str, value_name: &str) -> Result<(), String> {
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_WRITE, RegCloseKey, RegDeleteValueW, RegOpenKeyExW,
    };

    let subkey_w = to_wide_null(subkey);
    let value_w = to_wide_null(value_name);
    let mut hkey: HKEY = std::ptr::null_mut();

    unsafe {
        let status = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            subkey_w.as_ptr(),
            0,
            KEY_WRITE,
            &mut hkey,
        );
        if status != ERROR_SUCCESS {
            return Ok(()); // Key already doesn't exist
        }

        let status = RegDeleteValueW(hkey, value_w.as_ptr());
        RegCloseKey(hkey);

        if status != ERROR_SUCCESS && status != 2 {
            // 2 = ERROR_FILE_NOT_FOUND
            return Err(format!("RegDeleteValueW failed with code {}", status));
        }

        Ok(())
    }
}

#[cfg(windows)]
pub fn win_delete_registry_key(subkey: &str) -> Result<(), String> {
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RegDeleteKeyW};

    let subkey_w = to_wide_null(subkey);
    unsafe {
        let status = RegDeleteKeyW(HKEY_CURRENT_USER, subkey_w.as_ptr());
        if status != ERROR_SUCCESS && status != 2 {
            // 2 = ERROR_FILE_NOT_FOUND
            return Err(format!("RegDeleteKeyW failed with code {}", status));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_parsing() {
        assert_eq!(Channel::parse("stable"), Some(Channel::Stable));
        assert_eq!(Channel::parse("STABLE"), Some(Channel::Stable));
        assert_eq!(Channel::parse("  stable  "), Some(Channel::Stable));
        assert_eq!(Channel::parse("nightly"), Some(Channel::Nightly));
        assert_eq!(Channel::parse("NIGHTLY"), Some(Channel::Nightly));
        assert_eq!(Channel::parse("  nightly\n"), Some(Channel::Nightly));
        assert_eq!(Channel::parse("beta"), None);
        assert_eq!(Channel::parse(""), None);
    }

    #[test]
    fn test_ipc_endpoints() {
        let stable_ep = Channel::Stable.default_ipc_endpoint();
        let nightly_ep = Channel::Nightly.default_ipc_endpoint();

        assert_ne!(stable_ep, nightly_ep);
        if cfg!(windows) {
            assert!(stable_ep.contains("sona-agent-ipc"));
            assert!(nightly_ep.contains("sona-agent-ipc-nightly"));
        } else if cfg!(unix) {
            assert!(stable_ep.contains("agent.sock"));
            assert!(nightly_ep.contains("agent.sock") || nightly_ep.contains("nightly"));
        }
    }

    #[test]
    fn test_target_roles() {
        assert_eq!(TargetRole::Cli.as_str(), "cli");
        assert_eq!(TargetRole::Mcp.as_str(), "mcp");
        assert_eq!(TargetRole::Shim.as_str(), "shim");

        if cfg!(windows) {
            assert_eq!(TargetRole::Cli.binary_name(), "sona-cli.exe");
            assert_eq!(TargetRole::Mcp.binary_name(), "sona-mcp.exe");
            assert_eq!(TargetRole::Shim.binary_name(), "sona-shim.exe");
        } else {
            assert_eq!(TargetRole::Cli.binary_name(), "sona-cli");
            assert_eq!(TargetRole::Mcp.binary_name(), "sona-mcp");
            assert_eq!(TargetRole::Shim.binary_name(), "sona-shim");
        }
    }

    #[test]
    #[cfg(windows)]
    fn test_win_registry_raw_preserves_expand_sz() {
        let test_subkey = r"Software\SonaTestRegistryRaw";
        let _ = win_delete_registry_key(test_subkey);

        let unexpanded = r"%USERPROFILE%\test_sona_path";
        win_write_registry_expand_string(test_subkey, "RawTest", unexpanded).unwrap();

        let raw = win_read_registry_string_raw(test_subkey, "RawTest");
        assert_eq!(raw.as_deref(), Some(unexpanded));

        let expanded = win_read_registry_string(test_subkey, "RawTest");
        assert!(expanded.is_some());
        let expanded_val = expanded.unwrap();
        assert_ne!(expanded_val, unexpanded);
        assert!(!expanded_val.contains("%USERPROFILE%"));
        assert!(expanded_val.ends_with(r"\test_sona_path"));

        let _ = win_delete_registry_key(test_subkey);
    }
}
