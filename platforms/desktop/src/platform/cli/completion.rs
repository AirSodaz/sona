use serde::{Deserialize, Serialize};
#[cfg(windows)]
use std::collections::HashSet;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use super::path_env::{
    best_available_cli_binary, clean_path_display, probe_cli_version, run_cli_completion_command,
};

pub(crate) const MARKER_START: &str = "# >>> sona-cli completion >>>";
pub(crate) const MARKER_END: &str = "# <<< sona-cli completion <<<";
pub(crate) const VERSION_HEADER_PREFIX: &str = "# sona-cli completion (v";
pub(crate) const SUPPORTED_SHELLS: [&str; 4] = ["powershell", "zsh", "bash", "fish"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionInstallState {
    NotInstalled,
    Installed,
    Outdated,
    Partial,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellCompletionStatus {
    pub shell: String,
    pub state: CompletionInstallState,
    pub script_path: String,
    pub profile_paths: Vec<String>,
    pub installed_version: Option<String>,
    pub current_version: String,
    pub profile_managed: bool,
    pub has_legacy_inline_block: bool,
    pub manual_script_cmd: String,
    pub manual_profile_line: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct CompletionLayout {
    pub shell: String,
    pub script_path: PathBuf,
    pub profile_paths: Vec<PathBuf>,
    pub profile_block: Option<String>,
    pub manual_script_cmd: String,
    pub manual_profile_line: Option<String>,
}

fn non_empty_env<F>(read_env: &F, key: &str) -> Option<String>
where
    F: Fn(&str) -> Option<OsString>,
{
    read_env(key)
        .map(|v| v.to_string_lossy().trim().to_string())
        .filter(|s| !s.is_empty())
}

fn resolve_home_dir<F>(read_env: &F) -> Result<PathBuf, String>
where
    F: Fn(&str) -> Option<OsString>,
{
    #[cfg(windows)]
    {
        if let Some(profile) = non_empty_env(read_env, "USERPROFILE") {
            return Ok(PathBuf::from(profile));
        }
        if let Some(home) = non_empty_env(read_env, "HOME") {
            return Ok(PathBuf::from(home));
        }
        Err("Cannot determine home directory: USERPROFILE/HOME is not set".to_string())
    }
    #[cfg(not(windows))]
    {
        if let Some(home) = non_empty_env(read_env, "HOME") {
            return Ok(PathBuf::from(home));
        }
        Err("Cannot determine home directory: HOME is not set".to_string())
    }
}

#[cfg(windows)]
fn is_pwsh_available<F>(read_env: &F) -> bool
where
    F: Fn(&str) -> Option<OsString>,
{
    if let Some(path_var) = read_env("PATH") {
        for dir in std::env::split_paths(&path_var) {
            if dir.join("pwsh.exe").is_file() {
                return true;
            }
        }
    }
    if let Some(program_files) = non_empty_env(read_env, "ProgramFiles")
        && Path::new(&program_files)
            .join("PowerShell")
            .join("7")
            .join("pwsh.exe")
            .is_file()
    {
        return true;
    }
    false
}

#[cfg(windows)]
fn resolve_powershell_profiles_windows(
    home: &Path,
    documents_override: Option<&Path>,
    has_pwsh: bool,
) -> Vec<PathBuf> {
    let docs = documents_override
        .map(Path::to_path_buf)
        .or_else(super::path_env::win_path::get_windows_documents_dir)
        .unwrap_or_else(|| home.join("Documents"));

    let mut seen = HashSet::new();
    let mut profiles = Vec::new();
    let push_unique = |seen: &mut HashSet<String>, profiles: &mut Vec<PathBuf>, p: PathBuf| {
        let key = p.to_string_lossy().to_lowercase();
        if seen.insert(key) {
            profiles.push(p);
        }
    };

    let ps7_dir = docs.join("PowerShell");
    let ps7_profile = ps7_dir.join("Microsoft.PowerShell_profile.ps1");
    if ps7_profile.is_file() || ps7_dir.is_dir() || has_pwsh {
        push_unique(&mut seen, &mut profiles, ps7_profile);
    }

    let ps5_dir = docs.join("WindowsPowerShell");
    let ps5_profile = ps5_dir.join("Microsoft.PowerShell_profile.ps1");
    if ps5_profile.is_file() || ps5_dir.is_dir() || profiles.is_empty() {
        push_unique(&mut seen, &mut profiles, ps5_profile);
    }

    profiles
}

pub(crate) fn completion_layout_with_env<F>(
    shell: &str,
    read_env: &F,
    windows_docs_override: Option<&Path>,
    has_pwsh_override: Option<bool>,
) -> Result<CompletionLayout, String>
where
    F: Fn(&str) -> Option<OsString>,
{
    let normalized_shell = shell.trim().to_lowercase();
    let home = resolve_home_dir(read_env)?;

    match normalized_shell.as_str() {
        "powershell" => {
            let script_path = home
                .join(".config")
                .join("sona")
                .join("completions")
                .join("sona-cli.ps1");

            #[cfg(windows)]
            let (profile_paths, loader_line, manual_script_cmd) = {
                let has_pwsh = has_pwsh_override.unwrap_or_else(|| is_pwsh_available(read_env));
                (
                    resolve_powershell_profiles_windows(&home, windows_docs_override, has_pwsh),
                    r#"if (Test-Path "$HOME\.config\sona\completions\sona-cli.ps1") { . "$HOME\.config\sona\completions\sona-cli.ps1" }"#
                        .to_string(),
                    r#"New-Item -ItemType Directory -Force "$HOME\.config\sona\completions" | Out-Null; sona-cli completion powershell | Out-File -Encoding utf8 "$HOME\.config\sona\completions\sona-cli.ps1""#
                        .to_string(),
                )
            };

            #[cfg(not(windows))]
            let (profile_paths, loader_line, manual_script_cmd) = {
                let _ = (windows_docs_override, has_pwsh_override);
                let config_base = non_empty_env(read_env, "XDG_CONFIG_HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| home.join(".config"));
                (
                    vec![
                        config_base
                            .join("powershell")
                            .join("Microsoft.PowerShell_profile.ps1"),
                    ],
                    r#"if (Test-Path "$HOME/.config/sona/completions/sona-cli.ps1") { . "$HOME/.config/sona/completions/sona-cli.ps1" }"#
                        .to_string(),
                    "mkdir -p ~/.config/sona/completions && sona-cli completion powershell > ~/.config/sona/completions/sona-cli.ps1"
                        .to_string(),
                )
            };

            Ok(CompletionLayout {
                shell: normalized_shell,
                script_path,
                profile_paths,
                profile_block: Some(format!("{MARKER_START}\n{loader_line}\n{MARKER_END}")),
                manual_script_cmd,
                manual_profile_line: Some(loader_line),
            })
        }
        "zsh" => {
            let manual_profile_line =
                "fpath=(~/.zfunc $fpath); autoload -Uz compinit && compinit".to_string();
            Ok(CompletionLayout {
                shell: normalized_shell,
                script_path: home.join(".zfunc").join("_sona-cli"),
                profile_paths: vec![home.join(".zshrc")],
                profile_block: Some(format!(
                    "{MARKER_START}\nfpath=(~/.zfunc $fpath)\nautoload -Uz compinit && compinit\n{MARKER_END}"
                )),
                manual_script_cmd:
                    "mkdir -p ~/.zfunc && sona-cli completion zsh > ~/.zfunc/_sona-cli".to_string(),
                manual_profile_line: Some(manual_profile_line),
            })
        }
        "bash" => {
            let data_home = non_empty_env(read_env, "XDG_DATA_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".local").join("share"));
            Ok(CompletionLayout {
                shell: normalized_shell,
                script_path: data_home
                    .join("bash-completion")
                    .join("completions")
                    .join("sona-cli"),
                profile_paths: Vec::new(),
                profile_block: None,
                manual_script_cmd: "mkdir -p ~/.local/share/bash-completion/completions && sona-cli completion bash > ~/.local/share/bash-completion/completions/sona-cli"
                    .to_string(),
                manual_profile_line: None,
            })
        }
        "fish" => {
            let config_home = non_empty_env(read_env, "XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".config"));
            Ok(CompletionLayout {
                shell: normalized_shell,
                script_path: config_home
                    .join("fish")
                    .join("completions")
                    .join("sona-cli.fish"),
                profile_paths: Vec::new(),
                profile_block: None,
                manual_script_cmd: "mkdir -p ~/.config/fish/completions && sona-cli completion fish > ~/.config/fish/completions/sona-cli.fish"
                    .to_string(),
                manual_profile_line: None,
            })
        }
        other => Err(format!(
            "Unsupported shell for completion management: {other}"
        )),
    }
}

fn find_marker_span(content: &str) -> Option<(usize, usize)> {
    let start_pos = content.find(MARKER_START)?;
    let line_start = content[..start_pos].rfind('\n').map_or(0, |idx| idx + 1);
    let end_rel = content[start_pos..].find(MARKER_END)?;
    let end_marker_end = start_pos + end_rel + MARKER_END.len();
    let line_end = content[end_marker_end..]
        .find('\n')
        .map_or(content.len(), |idx| end_marker_end + idx + 1);
    Some((line_start, line_end))
}

fn normalize_newlines(text: &str, eol: &str) -> String {
    text.replace("\r\n", "\n").replace('\n', eol)
}

pub(crate) fn upsert_marker_block(existing: &str, block: &str) -> String {
    let (has_bom, body) = match existing.strip_prefix('\u{FEFF}') {
        Some(rest) => (true, rest),
        None => (false, existing),
    };
    let eol = if body.contains("\r\n") { "\r\n" } else { "\n" };
    let normalized_block = normalize_newlines(block.trim(), eol);

    let updated_body = if let Some((start, end)) = find_marker_span(body) {
        let prefix = &body[..start];
        let suffix = &body[end..];
        let mut out = String::with_capacity(body.len() + normalized_block.len() + 4);
        out.push_str(prefix);
        out.push_str(&normalized_block);
        out.push_str(eol);
        if !suffix.is_empty() {
            out.push_str(suffix);
        }
        out
    } else {
        let trimmed = body.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            format!("{normalized_block}{eol}")
        } else {
            format!("{trimmed}{eol}{eol}{normalized_block}{eol}")
        }
    };

    if has_bom {
        format!("\u{FEFF}{updated_body}")
    } else {
        updated_body
    }
}

pub(crate) fn remove_marker_block(existing: &str) -> Option<String> {
    let (has_bom, body) = match existing.strip_prefix('\u{FEFF}') {
        Some(rest) => (true, rest),
        None => (false, existing),
    };
    let (start, end) = find_marker_span(body)?;
    let eol = if body.contains("\r\n") { "\r\n" } else { "\n" };

    let prefix = body[..start].trim_end_matches(['\r', '\n']);
    let suffix = body[end..].trim_start_matches(['\r', '\n']);

    let updated_body = match (prefix.is_empty(), suffix.is_empty()) {
        (true, true) => String::new(),
        (true, false) => {
            let trimmed_suffix = suffix.trim_end_matches(['\r', '\n']);
            if trimmed_suffix.is_empty() {
                String::new()
            } else {
                format!("{trimmed_suffix}{eol}")
            }
        }
        (false, true) => format!("{prefix}{eol}"),
        (false, false) => format!("{prefix}{eol}{eol}{suffix}"),
    };

    if updated_body.is_empty() {
        Some(String::new())
    } else if has_bom {
        Some(format!("\u{FEFF}{updated_body}"))
    } else {
        Some(updated_body)
    }
}

pub(crate) fn parse_script_version(content: &str) -> Option<String> {
    let first_line = content
        .trim_start_matches('\u{FEFF}')
        .lines()
        .next()?
        .trim();
    let rest = first_line.strip_prefix(VERSION_HEADER_PREFIX)?;
    let ver = rest.strip_suffix(')')?.trim();
    if ver.is_empty() {
        None
    } else {
        Some(ver.to_string())
    }
}

fn zsh_already_configures_zfunc(zshrc_content: &str) -> bool {
    let outside = remove_marker_block(zshrc_content).unwrap_or_else(|| zshrc_content.to_string());
    outside
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with('#'))
        .any(|line| line.contains(".zfunc"))
}

pub(crate) fn detect_legacy_inline_powershell(profile_content: &str) -> bool {
    let outside =
        remove_marker_block(profile_content).unwrap_or_else(|| profile_content.to_string());
    outside.contains("Register-ArgumentCompleter")
        && (outside.contains("'sona-cli'") || outside.contains("\"sona-cli\""))
}

pub(crate) fn resolve_current_cli_version() -> String {
    best_available_cli_binary()
        .as_deref()
        .and_then(|bin| probe_cli_version(bin).0)
        .unwrap_or_else(|| env!("SONA_DISPLAY_VERSION").to_string())
}

pub(crate) fn inspect_shell_status(
    layout: &CompletionLayout,
    current_version: &str,
) -> ShellCompletionStatus {
    let script_exists = layout.script_path.is_file();
    let installed_version = if script_exists {
        fs::read_to_string(&layout.script_path)
            .ok()
            .and_then(|c| parse_script_version(&c))
    } else {
        None
    };

    let mut any_profile_has_marker = false;
    let mut all_profiles_satisfied = true;
    let mut has_legacy_inline_block = false;

    if layout.profile_block.is_some() {
        if layout.profile_paths.is_empty() {
            all_profiles_satisfied = false;
        }
        for profile_path in &layout.profile_paths {
            let content = if profile_path.is_file() {
                fs::read_to_string(profile_path).unwrap_or_default()
            } else {
                String::new()
            };

            let has_marker = find_marker_span(&content).is_some();
            if has_marker {
                any_profile_has_marker = true;
            }
            if layout.shell == "powershell" && detect_legacy_inline_powershell(&content) {
                has_legacy_inline_block = true;
            }
            let satisfied = if layout.shell == "zsh" {
                has_marker || zsh_already_configures_zfunc(&content)
            } else {
                has_marker
            };
            if !satisfied {
                all_profiles_satisfied = false;
            }
        }
    }

    let profile_managed = if layout.profile_block.is_none() {
        true
    } else {
        all_profiles_satisfied
    };

    let state = if !script_exists && !any_profile_has_marker {
        CompletionInstallState::NotInstalled
    } else if !script_exists || !profile_managed {
        CompletionInstallState::Partial
    } else if installed_version.as_deref() != Some(current_version) {
        CompletionInstallState::Outdated
    } else {
        CompletionInstallState::Installed
    };

    ShellCompletionStatus {
        shell: layout.shell.clone(),
        state,
        script_path: clean_path_display(&layout.script_path),
        profile_paths: layout
            .profile_paths
            .iter()
            .map(|p| clean_path_display(p))
            .collect(),
        installed_version,
        current_version: current_version.to_string(),
        profile_managed,
        has_legacy_inline_block,
        manual_script_cmd: layout.manual_script_cmd.clone(),
        manual_profile_line: layout.manual_profile_line.clone(),
    }
}

pub(crate) fn get_all_completion_statuses_sync() -> Result<Vec<ShellCompletionStatus>, String> {
    let current_version = resolve_current_cli_version();
    let read_env = |k: &str| std::env::var_os(k);
    let mut statuses = Vec::with_capacity(SUPPORTED_SHELLS.len());
    for shell in SUPPORTED_SHELLS {
        let layout = completion_layout_with_env(shell, &read_env, None, None)?;
        statuses.push(inspect_shell_status(&layout, &current_version));
    }
    Ok(statuses)
}

pub(crate) fn install_completion_with_layout(
    layout: &CompletionLayout,
    raw_script: &str,
    version: &str,
) -> Result<ShellCompletionStatus, String> {
    let trimmed = raw_script.trim_start_matches('\u{FEFF}');
    let content = format!("{VERSION_HEADER_PREFIX}{version})\n{trimmed}");

    if let Some(parent) = layout.script_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Failed to create completion directory {}: {e}",
                parent.display()
            )
        })?;
    }
    fs::write(&layout.script_path, content).map_err(|e| {
        format!(
            "Failed to write completion script {}: {e}",
            layout.script_path.display()
        )
    })?;

    if let Some(block) = layout.profile_block.as_deref() {
        for profile_path in &layout.profile_paths {
            let existing = if profile_path.is_file() {
                fs::read_to_string(profile_path).map_err(|e| {
                    format!("Failed to read profile {}: {e}", profile_path.display())
                })?
            } else {
                String::new()
            };

            if layout.shell == "zsh" && zsh_already_configures_zfunc(&existing) {
                continue;
            }

            let updated = upsert_marker_block(&existing, block);
            if updated != existing {
                if let Some(parent) = profile_path.parent() {
                    fs::create_dir_all(parent).map_err(|e| {
                        format!(
                            "Failed to create profile directory {}: {e}",
                            parent.display()
                        )
                    })?;
                }
                fs::write(profile_path, updated).map_err(|e| {
                    format!("Failed to write profile {}: {e}", profile_path.display())
                })?;
            }
        }
    }

    Ok(inspect_shell_status(layout, version))
}

pub(crate) fn uninstall_completion_with_layout(
    layout: &CompletionLayout,
    current_version: &str,
) -> Result<ShellCompletionStatus, String> {
    if layout.script_path.is_file() {
        fs::remove_file(&layout.script_path).map_err(|e| {
            format!(
                "Failed to remove completion script {}: {e}",
                layout.script_path.display()
            )
        })?;
    }

    for profile_path in &layout.profile_paths {
        if !profile_path.is_file() {
            continue;
        }
        let existing = fs::read_to_string(profile_path)
            .map_err(|e| format!("Failed to read profile {}: {e}", profile_path.display()))?;
        if let Some(cleaned) = remove_marker_block(&existing) {
            fs::write(profile_path, cleaned)
                .map_err(|e| format!("Failed to update profile {}: {e}", profile_path.display()))?;
        }
    }

    Ok(inspect_shell_status(layout, current_version))
}

pub(crate) fn install_completion_sync(shell: &str) -> Result<ShellCompletionStatus, String> {
    let cli = best_available_cli_binary()
        .ok_or_else(|| "Sona CLI executable not available to generate completions.".to_string())?;
    let version = probe_cli_version(&cli)
        .0
        .unwrap_or_else(|| env!("SONA_DISPLAY_VERSION").to_string());
    let read_env = |k: &str| std::env::var_os(k);
    let layout = completion_layout_with_env(shell, &read_env, None, None)?;
    let raw_script = run_cli_completion_command(&cli, &layout.shell)?;
    install_completion_with_layout(&layout, &raw_script, &version)
}

pub(crate) fn uninstall_completion_sync(shell: &str) -> Result<ShellCompletionStatus, String> {
    let current_version = resolve_current_cli_version();
    let read_env = |k: &str| std::env::var_os(k);
    let layout = completion_layout_with_env(shell, &read_env, None, None)?;
    uninstall_completion_with_layout(&layout, &current_version)
}

pub(crate) fn get_completion_script_sync(shell: &str) -> Result<String, String> {
    let cli = best_available_cli_binary()
        .ok_or_else(|| "Sona CLI executable not available to generate completions.".to_string())?;
    run_cli_completion_command(&cli, shell)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn env_reader(map: HashMap<&'static str, OsString>) -> impl Fn(&str) -> Option<OsString> {
        move |key| map.get(key).cloned()
    }

    #[test]
    fn status_transitions_cover_not_installed_installed_outdated_partial_and_uninstall() {
        let temp = TempDir::new().unwrap();
        let home = temp.path().to_path_buf();
        let docs = home.join("Documents");
        let ps_dir = docs.join("WindowsPowerShell");
        fs::create_dir_all(&ps_dir).unwrap();
        let profile = ps_dir.join("Microsoft.PowerShell_profile.ps1");
        fs::write(&profile, "\u{FEFF}# user line\r\n").unwrap();

        let mut env = HashMap::new();
        env.insert("USERPROFILE", home.as_os_str().to_os_string());
        env.insert("HOME", home.as_os_str().to_os_string());
        env.insert(
            "XDG_CONFIG_HOME",
            home.join(".config").as_os_str().to_os_string(),
        );
        let read_env = env_reader(env);

        let layout =
            completion_layout_with_env("powershell", &read_env, Some(&docs), Some(false)).unwrap();

        // 1. NotInstalled
        let s0 = inspect_shell_status(&layout, "0.10.5");
        assert_eq!(s0.state, CompletionInstallState::NotInstalled);
        assert!(!s0.has_legacy_inline_block);

        // 2. Installed
        let s1 = install_completion_with_layout(
            &layout,
            "Register-ArgumentCompleter -CommandName 'sona-cli' -ScriptBlock {}",
            "0.10.5",
        )
        .unwrap();
        assert_eq!(s1.state, CompletionInstallState::Installed);
        assert_eq!(s1.installed_version.as_deref(), Some("0.10.5"));
        assert!(s1.profile_managed);
        assert!(!s1.has_legacy_inline_block);

        // Profile preserves BOM, CRLF, and user content
        let profile_text = fs::read_to_string(&layout.profile_paths[0]).unwrap();
        assert!(profile_text.starts_with('\u{FEFF}'));
        assert!(profile_text.contains("\r\n"));
        assert!(profile_text.contains("# user line"));
        assert!(profile_text.contains(MARKER_START));

        // 3. Outdated when newer version exists
        let s2 = inspect_shell_status(&layout, "0.10.6");
        assert_eq!(s2.state, CompletionInstallState::Outdated);

        // 4. Partial when script removed while loader stays in profile
        fs::remove_file(&layout.script_path).unwrap();
        let s3 = inspect_shell_status(&layout, "0.10.6");
        assert_eq!(s3.state, CompletionInstallState::Partial);

        // 5. Uninstall cleans up cleanly
        let s4 = uninstall_completion_with_layout(&layout, "0.10.6").unwrap();
        assert_eq!(s4.state, CompletionInstallState::NotInstalled);
        let profile_after = fs::read_to_string(&layout.profile_paths[0]).unwrap();
        assert_eq!(profile_after, "\u{FEFF}# user line\r\n");
    }

    #[test]
    fn detects_legacy_inline_powershell_block_outside_markers() {
        let legacy = "Write-Host hi\nRegister-ArgumentCompleter -Native -CommandName 'sona-cli' -ScriptBlock { }\n";
        assert!(detect_legacy_inline_powershell(legacy));

        let clean_managed = "# >>> sona-cli completion >>>\nif (Test-Path \"$HOME\\.config\\sona\\completions\\sona-cli.ps1\") { . \"$HOME\\.config\\sona\\completions\\sona-cli.ps1\" }\n# <<< sona-cli completion <<<\n";
        assert!(!detect_legacy_inline_powershell(clean_managed));
    }
}
