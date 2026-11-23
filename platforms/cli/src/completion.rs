use std::collections::HashSet;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use clap_complete::Shell;

use crate::{CliError, CliOutput, CliResult};

pub(crate) const MARKER_START: &str = "# >>> sona-cli completion >>>";
pub(crate) const MARKER_END: &str = "# <<< sona-cli completion <<<";
pub(crate) const VERSION_HEADER_PREFIX: &str = "# sona-cli completion (v";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompletionLayout {
    pub script_path: PathBuf,
    pub profile_paths: Vec<PathBuf>,
    pub profile_block: Option<String>,
}

pub(crate) fn ensure_supported_managed_shell(shell: Shell) -> CliResult<()> {
    match shell {
        Shell::Bash | Shell::Zsh | Shell::Fish | Shell::PowerShell => Ok(()),
        other => Err(CliError::Usage(format!(
            "--install and --uninstall currently support bash, zsh, fish, and powershell (got {other}). Run `sona-cli completion {other}` to print the script to stdout."
        ))),
    }
}

pub(crate) fn version_header(version: &str) -> String {
    format!("{VERSION_HEADER_PREFIX}{version})")
}

pub(crate) fn generate_raw_script(shell: Shell, cmd: &mut clap::Command) -> CliResult<String> {
    let mut buf = Vec::new();
    clap_complete::generate(shell, cmd, "sona-cli", &mut buf);
    String::from_utf8(buf).map_err(|error| CliError::Serialize(error.to_string()))
}

pub(crate) fn generate_standalone_script(
    shell: Shell,
    cmd: &mut clap::Command,
    version: &str,
) -> CliResult<String> {
    let raw = generate_raw_script(shell, cmd)?;
    let trimmed = raw.trim_start_matches('\u{FEFF}');
    Ok(format!("{}\n{}", version_header(version), trimmed))
}

fn non_empty_env<F>(read_env: &F, key: &str) -> Option<String>
where
    F: Fn(&str) -> Option<OsString>,
{
    read_env(key)
        .map(|val| val.to_string_lossy().trim().to_string())
        .filter(|s| !s.is_empty())
}

fn io_err(context: &str, err: std::io::Error) -> CliError {
    CliError::Io(format!("{context}: {err}"))
}

fn resolve_home_dir<F>(read_env: &F) -> CliResult<PathBuf>
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
        Err(CliError::Io(
            "Cannot determine home directory: USERPROFILE/HOME is not set".to_string(),
        ))
    }
    #[cfg(not(windows))]
    {
        if let Some(home) = non_empty_env(read_env, "HOME") {
            return Ok(PathBuf::from(home));
        }
        Err(CliError::Io(
            "Cannot determine home directory: HOME is not set".to_string(),
        ))
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
fn resolve_powershell_profiles_windows<F>(read_env: &F, home: &Path) -> Vec<PathBuf>
where
    F: Fn(&str) -> Option<OsString>,
{
    if let Some(explicit_profile) = non_empty_env(read_env, "SONA_PS_PROFILE") {
        return vec![PathBuf::from(explicit_profile)];
    }

    let mut doc_roots: Vec<PathBuf> = Vec::new();
    if let Some(onedrive) = non_empty_env(read_env, "OneDrive") {
        let onedrive_docs = PathBuf::from(onedrive).join("Documents");
        if onedrive_docs.is_dir() {
            doc_roots.push(onedrive_docs);
        }
    }
    let user_docs = home.join("Documents");
    if user_docs.is_dir() || doc_roots.is_empty() {
        doc_roots.push(user_docs);
    }

    let mut seen = HashSet::new();
    let mut profiles = Vec::new();
    let push_unique = |seen: &mut HashSet<String>, profiles: &mut Vec<PathBuf>, p: PathBuf| {
        let key = p.to_string_lossy().to_lowercase();
        if seen.insert(key) {
            profiles.push(p);
        }
    };

    for docs in &doc_roots {
        let ps7_dir = docs.join("PowerShell");
        let ps7_profile = ps7_dir.join("Microsoft.PowerShell_profile.ps1");
        if ps7_profile.is_file() || ps7_dir.is_dir() {
            push_unique(&mut seen, &mut profiles, ps7_profile);
        }

        let ps5_dir = docs.join("WindowsPowerShell");
        let ps5_profile = ps5_dir.join("Microsoft.PowerShell_profile.ps1");
        if ps5_profile.is_file() || ps5_dir.is_dir() {
            push_unique(&mut seen, &mut profiles, ps5_profile);
        }
    }

    // Check if OneDrive Documents exists even without a PowerShell subdir yet;
    // prefer USERPROFILE\Documents unless OneDrive already owns a PowerShell folder.
    let primary_docs = doc_roots
        .iter()
        .find(|d| d.join("PowerShell").is_dir() || d.join("WindowsPowerShell").is_dir())
        .cloned()
        .unwrap_or_else(|| home.join("Documents"));

    if is_pwsh_available(read_env) {
        push_unique(
            &mut seen,
            &mut profiles,
            primary_docs
                .join("PowerShell")
                .join("Microsoft.PowerShell_profile.ps1"),
        );
    }

    if profiles.is_empty() {
        push_unique(
            &mut seen,
            &mut profiles,
            primary_docs
                .join("WindowsPowerShell")
                .join("Microsoft.PowerShell_profile.ps1"),
        );
    }

    profiles
}

pub(crate) fn completion_layout_with_env<F>(
    shell: Shell,
    read_env: &F,
) -> CliResult<CompletionLayout>
where
    F: Fn(&str) -> Option<OsString>,
{
    ensure_supported_managed_shell(shell)?;
    let home = resolve_home_dir(read_env)?;

    match shell {
        Shell::PowerShell => {
            let script_path = home
                .join(".config")
                .join("sona")
                .join("completions")
                .join("sona-cli.ps1");
            #[cfg(windows)]
            let (profile_paths, loader_line) = (
                resolve_powershell_profiles_windows(read_env, &home),
                r#"if (Test-Path "$HOME\.config\sona\completions\sona-cli.ps1") { . "$HOME\.config\sona\completions\sona-cli.ps1" }"#,
            );
            #[cfg(not(windows))]
            let (profile_paths, loader_line) = {
                let config_base = non_empty_env(read_env, "XDG_CONFIG_HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| home.join(".config"));
                (
                    vec![
                        config_base
                            .join("powershell")
                            .join("Microsoft.PowerShell_profile.ps1"),
                    ],
                    r#"if (Test-Path "$HOME/.config/sona/completions/sona-cli.ps1") { . "$HOME/.config/sona/completions/sona-cli.ps1" }"#,
                )
            };
            Ok(CompletionLayout {
                script_path,
                profile_paths,
                profile_block: Some(format!("{MARKER_START}\n{loader_line}\n{MARKER_END}")),
            })
        }
        Shell::Zsh => Ok(CompletionLayout {
            script_path: home.join(".zfunc").join("_sona-cli"),
            profile_paths: vec![home.join(".zshrc")],
            profile_block: Some(format!(
                "{MARKER_START}\nfpath=(~/.zfunc $fpath)\nautoload -Uz compinit && compinit\n{MARKER_END}"
            )),
        }),
        Shell::Bash => {
            let data_home = non_empty_env(read_env, "XDG_DATA_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".local").join("share"));
            Ok(CompletionLayout {
                script_path: data_home
                    .join("bash-completion")
                    .join("completions")
                    .join("sona-cli"),
                profile_paths: Vec::new(),
                profile_block: None,
            })
        }
        Shell::Fish => {
            let config_home = non_empty_env(read_env, "XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".config"));
            Ok(CompletionLayout {
                script_path: config_home
                    .join("fish")
                    .join("completions")
                    .join("sona-cli.fish"),
                profile_paths: Vec::new(),
                profile_block: None,
            })
        }
        _ => unreachable!("checked by ensure_supported_managed_shell"),
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

fn zsh_already_configures_zfunc(zshrc_content: &str) -> bool {
    let outside_block =
        remove_marker_block(zshrc_content).unwrap_or_else(|| zshrc_content.to_string());
    outside_block
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with('#'))
        .any(|line| line.contains(".zfunc"))
}

pub(crate) fn install_completion_with_env<F>(
    shell: Shell,
    cmd: &mut clap::Command,
    version: &str,
    read_env: &F,
) -> CliResult<CliOutput>
where
    F: Fn(&str) -> Option<OsString>,
{
    let layout = completion_layout_with_env(shell, read_env)?;
    let script_text = generate_standalone_script(shell, cmd, version)?;

    if let Some(parent) = layout.script_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            io_err(
                &format!("Failed to create directory {}", parent.display()),
                e,
            )
        })?;
    }
    fs::write(&layout.script_path, script_text).map_err(|e| {
        io_err(
            &format!(
                "Failed to write completion script {}",
                layout.script_path.display()
            ),
            e,
        )
    })?;

    let mut updated_profiles = Vec::new();
    let mut skipped_zsh_profile = false;

    if let Some(block) = layout.profile_block.as_deref() {
        for profile_path in &layout.profile_paths {
            let existing = if profile_path.is_file() {
                fs::read_to_string(profile_path).map_err(|e| {
                    io_err(
                        &format!("Failed to read profile {}", profile_path.display()),
                        e,
                    )
                })?
            } else {
                String::new()
            };

            if shell == Shell::Zsh && zsh_already_configures_zfunc(&existing) {
                skipped_zsh_profile = true;
                continue;
            }

            let updated = upsert_marker_block(&existing, block);
            if updated != existing {
                if let Some(parent) = profile_path.parent() {
                    fs::create_dir_all(parent).map_err(|e| {
                        io_err(
                            &format!("Failed to create directory {}", parent.display()),
                            e,
                        )
                    })?;
                }
                fs::write(profile_path, updated).map_err(|e| {
                    io_err(
                        &format!("Failed to write profile {}", profile_path.display()),
                        e,
                    )
                })?;
                updated_profiles.push(profile_path.clone());
            } else if profile_path.is_file() {
                updated_profiles.push(profile_path.clone());
            }
        }
    }

    let mut lines = vec![format!(
        "Installed {shell} completion script: {}",
        layout.script_path.display()
    )];
    if !updated_profiles.is_empty() {
        for p in &updated_profiles {
            lines.push(format!("Updated profile loader: {}", p.display()));
        }
    } else if skipped_zsh_profile {
        lines.push("Skipped ~/.zshrc update because ~/.zfunc is already configured.".to_string());
    } else if layout.profile_paths.is_empty() {
        lines.push(
            "No profile modification required (shell auto-loads this directory).".to_string(),
        );
    }
    lines.push("Restart or reload your shell to activate completions.".to_string());

    Ok(CliOutput::stdout(lines.join("\n")))
}

pub(crate) fn uninstall_completion_with_env<F>(shell: Shell, read_env: &F) -> CliResult<CliOutput>
where
    F: Fn(&str) -> Option<OsString>,
{
    let layout = completion_layout_with_env(shell, read_env)?;
    let mut removed_script = false;
    if layout.script_path.is_file() {
        fs::remove_file(&layout.script_path).map_err(|e| {
            io_err(
                &format!("Failed to remove script {}", layout.script_path.display()),
                e,
            )
        })?;
        removed_script = true;
    }

    let mut cleaned_profiles = Vec::new();
    for profile_path in &layout.profile_paths {
        if !profile_path.is_file() {
            continue;
        }
        let existing = fs::read_to_string(profile_path).map_err(|e| {
            io_err(
                &format!("Failed to read profile {}", profile_path.display()),
                e,
            )
        })?;
        if let Some(cleaned) = remove_marker_block(&existing) {
            fs::write(profile_path, cleaned).map_err(|e| {
                io_err(
                    &format!("Failed to write profile {}", profile_path.display()),
                    e,
                )
            })?;
            cleaned_profiles.push(profile_path.clone());
        }
    }

    let mut lines = Vec::new();
    if removed_script {
        lines.push(format!(
            "Removed {shell} completion script: {}",
            layout.script_path.display()
        ));
    } else {
        lines.push(format!(
            "Completion script not present: {}",
            layout.script_path.display()
        ));
    }
    for p in &cleaned_profiles {
        lines.push(format!("Cleaned profile loader block: {}", p.display()));
    }
    if !removed_script && cleaned_profiles.is_empty() {
        lines.push("No installed completion artifacts were found.".to_string());
    }

    Ok(CliOutput::stdout(lines.join("\n")))
}

pub(crate) fn install_completion(
    shell: Shell,
    cmd: &mut clap::Command,
    version: &str,
) -> CliResult<CliOutput> {
    install_completion_with_env(shell, cmd, version, &|k| std::env::var_os(k))
}

pub(crate) fn uninstall_completion(shell: Shell) -> CliResult<CliOutput> {
    uninstall_completion_with_env(shell, &|k| std::env::var_os(k))
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
    fn upsert_and_remove_are_idempotent_and_preserve_bom_and_crlf() {
        let initial = "\u{FEFF}Set-PSReadLineOption -EditMode Emacs\r\n";
        let block = "# >>> sona-cli completion >>>\nif (Test-Path \"$HOME\\.config\\sona\\completions\\sona-cli.ps1\") { . \"$HOME\\.config\\sona\\completions\\sona-cli.ps1\" }\n# <<< sona-cli completion <<<";

        let once = upsert_marker_block(initial, block);
        assert!(once.starts_with('\u{FEFF}'));
        assert!(once.contains("\r\n"));
        assert!(once.contains("Set-PSReadLineOption -EditMode Emacs"));
        assert!(once.contains(MARKER_START));

        let twice = upsert_marker_block(&once, block);
        assert_eq!(once, twice);
        assert_eq!(twice.matches(MARKER_START).count(), 1);

        let cleaned = remove_marker_block(&twice).expect("block should be removed");
        assert_eq!(cleaned, initial);
    }

    #[test]
    fn install_and_uninstall_powershell_writes_standalone_file_and_minimal_loader() {
        let temp = TempDir::new().unwrap();
        let home = temp.path().to_path_buf();
        let docs = home.join("Documents");
        let ps_dir = docs.join("WindowsPowerShell");
        fs::create_dir_all(&ps_dir).unwrap();
        let profile = ps_dir.join("Microsoft.PowerShell_profile.ps1");
        fs::write(&profile, "Write-Host 'hello'\n").unwrap();

        let mut env = HashMap::new();
        env.insert("USERPROFILE", home.as_os_str().to_os_string());
        env.insert("HOME", home.as_os_str().to_os_string());
        #[cfg(windows)]
        env.insert("SONA_PS_PROFILE", profile.as_os_str().to_os_string());
        #[cfg(not(windows))]
        env.insert(
            "XDG_CONFIG_HOME",
            home.join(".config").as_os_str().to_os_string(),
        );
        let read_env = env_reader(env);

        let mut cmd = crate::cli_command();
        let out =
            install_completion_with_env(Shell::PowerShell, &mut cmd, "0.10.5", &read_env).unwrap();
        assert!(out.stdout.contains("sona-cli.ps1"));

        let script_path = home
            .join(".config")
            .join("sona")
            .join("completions")
            .join("sona-cli.ps1");
        let script = fs::read_to_string(&script_path).unwrap();
        assert!(script.starts_with("# sona-cli completion (v0.10.5)\n"));
        assert!(script.contains("Register-ArgumentCompleter"));

        #[cfg(windows)]
        let target_profile = profile;
        #[cfg(not(windows))]
        let target_profile = home
            .join(".config")
            .join("powershell")
            .join("Microsoft.PowerShell_profile.ps1");

        let profile_content = fs::read_to_string(&target_profile).unwrap();
        assert!(profile_content.contains(MARKER_START));
        assert!(profile_content.contains("sona-cli.ps1"));
        assert!(!profile_content.contains("Register-ArgumentCompleter"));

        // Second install is idempotent
        install_completion_with_env(Shell::PowerShell, &mut cmd, "0.10.5", &read_env).unwrap();
        assert_eq!(
            fs::read_to_string(&target_profile).unwrap(),
            profile_content
        );

        // Uninstall removes script and marker block while keeping user lines
        uninstall_completion_with_env(Shell::PowerShell, &read_env).unwrap();
        assert!(!script_path.exists());
        let after_uninstall = fs::read_to_string(&target_profile).unwrap();
        assert!(!after_uninstall.contains(MARKER_START));
        #[cfg(windows)]
        assert_eq!(after_uninstall, "Write-Host 'hello'\n");
    }

    #[test]
    fn install_zsh_respects_existing_zfunc_configuration() {
        let temp = TempDir::new().unwrap();
        let home = temp.path().to_path_buf();
        let zshrc = home.join(".zshrc");
        let existing_zshrc = "fpath=(~/.zfunc $fpath)\nautoload -Uz compinit && compinit\n";
        fs::write(&zshrc, existing_zshrc).unwrap();

        let mut env = HashMap::new();
        env.insert("HOME", home.as_os_str().to_os_string());
        env.insert("USERPROFILE", home.as_os_str().to_os_string());
        let read_env = env_reader(env);

        let mut cmd = crate::cli_command();
        let out = install_completion_with_env(Shell::Zsh, &mut cmd, "0.10.5", &read_env).unwrap();
        assert!(out.stdout.contains("already configured"));
        assert_eq!(fs::read_to_string(&zshrc).unwrap(), existing_zshrc);
        assert!(home.join(".zfunc").join("_sona-cli").is_file());
    }

    #[test]
    fn elvish_rejects_install_flag_with_usage_error() {
        let err = ensure_supported_managed_shell(Shell::Elvish).unwrap_err();
        assert!(matches!(err, CliError::Usage(_)));
    }
}
