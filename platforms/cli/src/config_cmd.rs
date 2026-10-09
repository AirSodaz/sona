use clap::{Args, Subcommand};
use std::path::PathBuf;

use crate::{CliError, CliOutput, CliResult};

#[derive(Debug, Args)]
#[command(
    about = "Inspect and manage Sona CLI configuration",
    after_help = "Examples:\n  sona-cli config init\n  sona-cli config init ./custom.toml -F\n  sona-cli config path\n  sona-cli config check\n  sona-cli config show\n  sona-cli config get transcribe.model_id\n  sona-cli config set transcribe.model_id whisper-turbo\n  sona-cli config set serve.port 14200 --global\n  sona-cli config edit"
)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommands,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommands {
    /// Create a commented TOML starter configuration template.
    #[command(
        about = "Create a commented TOML starter configuration template",
        after_help = "Examples:\n  sona-cli config init\n  sona-cli config init ./sona-cli.toml -F"
    )]
    Init(crate::init_config::InitConfigArgs),

    /// Print the resolved active configuration file path.
    #[command(
        about = "Print the resolved active configuration file path",
        after_help = "Examples:\n  sona-cli config path\n  sona-cli config path -c ./custom.toml"
    )]
    Path(ConfigPathArgs),

    /// Validate the syntax and sections of the configuration file.
    #[command(
        about = "Validate the syntax and sections of the configuration file",
        after_help = "Examples:\n  sona-cli config check\n  sona-cli config check -c ./custom.toml"
    )]
    Check(ConfigCheckArgs),

    /// Display the contents of the resolved configuration file.
    #[command(
        about = "Display the contents of the resolved configuration file",
        after_help = "Examples:\n  sona-cli config show\n  sona-cli config show -c ./custom.toml"
    )]
    Show(ConfigShowArgs),

    /// Read a specific configuration key (e.g. transcribe.model_id).
    #[command(
        about = "Read a specific configuration key (e.g. transcribe.model_id)",
        after_help = "Examples:\n  sona-cli config get transcribe.model_id\n  sona-cli config get serve.port\n  sona-cli config get transcribe.model_id --global"
    )]
    Get(ConfigGetArgs),

    /// Set a specific configuration key (e.g. transcribe.model_id whisper-turbo).
    #[command(
        about = "Set a specific configuration key (e.g. transcribe.model_id whisper-turbo)",
        after_help = "Examples:\n  sona-cli config set transcribe.model_id whisper-turbo\n  sona-cli config set serve.port 14200\n  sona-cli config set transcribe.enable_itn true\n  sona-cli config set transcribe.model_id whisper-turbo --global"
    )]
    Set(ConfigSetArgs),

    /// Remove a specific configuration key (e.g. transcribe.model_id).
    #[command(
        about = "Remove a specific configuration key (e.g. transcribe.model_id)",
        visible_alias = "rm",
        after_help = "Examples:\n  sona-cli config unset transcribe.model_id\n  sona-cli config rm serve.port\n  sona-cli config unset transcribe.model_id --global"
    )]
    Unset(ConfigUnsetArgs),
    /// Open the configuration file in $EDITOR and validate syntax upon exit.
    #[command(
        about = "Open the configuration file in $EDITOR and validate syntax upon exit",
        after_help = "Examples:\n  sona-cli config edit\n  sona-cli config edit -c ./custom.toml"
    )]
    Edit(ConfigEditArgs),
}

#[derive(Clone, Debug, Default, Args)]
pub struct ConfigTargetArgs {
    /// Optional configuration file path override.
    #[arg(
        short = 'c',
        long = "config",
        value_name = "FILE",
        conflicts_with = "global"
    )]
    pub config: Option<PathBuf>,
    /// Target user standard configuration path instead of local directory.
    #[arg(
        short = 'g',
        long = "global",
        alias = "user",
        conflicts_with = "config"
    )]
    pub global: bool,
}

#[derive(Debug, Args)]
pub struct ConfigPathArgs {
    #[command(flatten)]
    pub target: ConfigTargetArgs,
}

#[derive(Debug, Args)]
pub struct ConfigCheckArgs {
    #[command(flatten)]
    pub target: ConfigTargetArgs,
}

#[derive(Debug, Args)]
pub struct ConfigShowArgs {
    #[command(flatten)]
    pub target: ConfigTargetArgs,
}

#[derive(Debug, Args)]
pub struct ConfigGetArgs {
    /// Dot-separated key path, e.g. transcribe.model_id or serve.port.
    #[arg(value_name = "KEY")]
    pub key: String,
    #[command(flatten)]
    pub target: ConfigTargetArgs,
}

#[derive(Debug, Args)]
pub struct ConfigSetArgs {
    /// Dot-separated key path, e.g. transcribe.model_id or serve.port.
    #[arg(value_name = "KEY")]
    pub key: String,
    /// Value to set (automatically parses numbers, booleans, and strings).
    #[arg(value_name = "VALUE")]
    pub value: String,
    #[command(flatten)]
    pub target: ConfigTargetArgs,
}

#[derive(Debug, Args)]
pub struct ConfigUnsetArgs {
    /// Dot-separated key path, e.g. transcribe.model_id or serve.port.
    #[arg(value_name = "KEY")]
    pub key: String,
    #[command(flatten)]
    pub target: ConfigTargetArgs,
}

#[derive(Debug, Args)]
pub struct ConfigEditArgs {
    #[command(flatten)]
    pub target: ConfigTargetArgs,
}
pub fn run_config(args: ConfigArgs) -> CliResult<CliOutput> {
    match args.command {
        ConfigCommands::Init(init_args) => crate::init_config::run_init_config(init_args),
        ConfigCommands::Path(path_args) => run_config_path(path_args),
        ConfigCommands::Check(check_args) => run_config_check(check_args),
        ConfigCommands::Show(show_args) => run_config_show(show_args),
        ConfigCommands::Get(get_args) => run_config_get(get_args),
        ConfigCommands::Set(set_args) => run_config_set(set_args),
        ConfigCommands::Unset(unset_args) => run_config_unset(unset_args),
        ConfigCommands::Edit(edit_args) => run_config_edit(edit_args),
    }
}

fn resolve_target_file_path(config: Option<&PathBuf>, global: bool) -> CliResult<PathBuf> {
    if global {
        crate::init_config::canonical_user_config_path(&|name| std::env::var_os(name)).ok_or_else(
            || {
                CliError::Validation(
                    "Could not determine standard user configuration directory.".to_string(),
                )
            },
        )
    } else if let Some(path) = config {
        Ok(path.clone())
    } else {
        match crate::init_config::resolve_config_path(None) {
            Some(path) => Ok(path),
            None => Ok(PathBuf::from(crate::init_config::DEFAULT_CONFIG_PATH)),
        }
    }
}

fn resolve_existing_file_path(config: Option<&PathBuf>, global: bool) -> CliResult<PathBuf> {
    if global {
        let path = crate::init_config::canonical_user_config_path(&|name| std::env::var_os(name))
            .ok_or_else(|| {
            CliError::Validation(
                "Could not determine standard user configuration directory.".to_string(),
            )
        })?;
        if !path.is_file() {
            return Err(CliError::Validation(format!(
                "Global configuration file not found at {}. Run 'sona-cli config init --global' to create one.",
                path.display()
            )));
        }
        Ok(path)
    } else {
        crate::init_config::resolve_config_path(config).ok_or_else(|| {
            CliError::Validation(
                "No configuration file found. Run 'sona-cli config init' to create one."
                    .to_string(),
            )
        })
    }
}

fn check_config_file(path: &std::path::Path) -> CliResult<CliOutput> {
    let mut errors = Vec::new();
    if let Err(err) = sona_runtime_fs::load_transcribe_config_file(path) {
        errors.push(format!("[transcribe]: {err}"));
    }
    if let Err(err) = sona_runtime_fs::load_transcribe_live_config_file(path) {
        errors.push(format!("[transcribe_live]: {err}"));
    }
    if let Err(err) = sona_runtime_fs::load_serve_config_file(path) {
        errors.push(format!("[serve]: {err}"));
    }

    if !errors.is_empty() {
        return Err(CliError::Validation(format!(
            "Configuration error in {}: {}",
            path.display(),
            errors.join("; ")
        )));
    }

    let content = std::fs::read_to_string(path)
        .map_err(|e| CliError::Io(format!("Failed to read {}: {e}", path.display())))?;
    let warnings = find_unknown_config_keys(&content, path);

    let mut output = format!("Configuration at {} is valid.", path.display());
    if !warnings.is_empty() {
        output.push('\n');
        output.push_str(&warnings.join("\n"));
    }
    Ok(CliOutput::stdout(output))
}

fn find_unknown_config_keys(content: &str, path: &std::path::Path) -> Vec<String> {
    let Ok(value) = content.parse::<toml::Table>() else {
        return Vec::new();
    };

    let mut warnings = Vec::new();
    let path_display = path.display();

    for (k, v) in &value {
        if !sona_core::runtime::config::ROOT_CONFIG_KEYS.contains(&k.as_str()) {
            warnings.push(format!(
                "Warning: Unrecognized key '{k}' in {path_display}; this key will have no effect."
            ));
        } else if let toml::Value::Table(table) = v {
            let allowed = match k.as_str() {
                "transcribe" => Some(sona_core::runtime::config::TRANSCRIBE_CONFIG_KEYS),
                "live" | "transcribe_live" => Some(sona_core::runtime::config::LIVE_CONFIG_KEYS),
                "serve" => Some(sona_core::runtime::config::SERVE_CONFIG_KEYS),
                _ => None,
            };
            if let Some(keys) = allowed {
                for sub_k in table.keys() {
                    if !keys.contains(&sub_k.as_str()) {
                        warnings.push(format!("Warning: Unrecognized key '{k}.{sub_k}' in {path_display}; this key will have no effect."));
                    }
                }
            }
        }
    }

    warnings
}

fn run_config_path(args: ConfigPathArgs) -> CliResult<CliOutput> {
    let path = resolve_existing_file_path(args.target.config.as_ref(), args.target.global)?;
    Ok(CliOutput::stdout(path.display().to_string()))
}

fn run_config_check(args: ConfigCheckArgs) -> CliResult<CliOutput> {
    let path = resolve_existing_file_path(args.target.config.as_ref(), args.target.global)?;
    check_config_file(&path)
}

fn run_config_show(args: ConfigShowArgs) -> CliResult<CliOutput> {
    let path = resolve_existing_file_path(args.target.config.as_ref(), args.target.global)?;
    let content = std::fs::read_to_string(&path)
        .map_err(|e| CliError::Io(format!("Failed to read {}: {e}", path.display())))?;
    Ok(CliOutput::stdout(content))
}

fn get_toml_value<'a>(toml_val: &'a toml::Value, key: &str) -> Option<&'a toml::Value> {
    let parts: Vec<&str> = key.split('.').collect();
    if parts.is_empty() {
        return None;
    }

    let traverse = |target_parts: &[&str]| -> Option<&'a toml::Value> {
        let mut current = toml_val;
        for part in target_parts {
            current = current.get(part)?;
        }
        Some(current)
    };

    if let Some(val) = traverse(&parts) {
        return Some(val);
    }

    if parts[0] == "live" {
        let mut alt_parts = parts.clone();
        alt_parts[0] = "transcribe_live";
        return traverse(&alt_parts);
    } else if parts[0] == "transcribe_live" {
        let mut alt_parts = parts.clone();
        alt_parts[0] = "live";
        return traverse(&alt_parts);
    }

    None
}

fn run_config_get(args: ConfigGetArgs) -> CliResult<CliOutput> {
    let path = resolve_existing_file_path(args.target.config.as_ref(), args.target.global)?;
    let content = std::fs::read_to_string(&path)
        .map_err(|e| CliError::Io(format!("Failed to read {}: {e}", path.display())))?;
    let toml_val: toml::Value = toml::from_str(&content).map_err(|e| {
        CliError::Validation(format!("Invalid TOML syntax in {}: {e}", path.display()))
    })?;

    let value = get_toml_value(&toml_val, &args.key).ok_or_else(|| {
        CliError::Validation(format!(
            "Key '{}' not found in {}",
            args.key,
            path.display()
        ))
    })?;

    let rendered = match value {
        toml::Value::String(s) => s.clone(),
        toml::Value::Integer(i) => i.to_string(),
        toml::Value::Float(f) => f.to_string(),
        toml::Value::Boolean(b) => b.to_string(),
        other => other.to_string(),
    };

    Ok(CliOutput::stdout(rendered))
}

fn parse_toml_value(raw: &str) -> toml_edit::Item {
    let trimmed = raw.trim();
    if (trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2)
        || (trimmed.starts_with('\'') && trimmed.ends_with('\'') && trimmed.len() >= 2)
    {
        return toml_edit::value(&trimmed[1..trimmed.len() - 1]);
    }
    if trimmed.eq_ignore_ascii_case("true") {
        return toml_edit::value(true);
    }
    if trimmed.eq_ignore_ascii_case("false") {
        return toml_edit::value(false);
    }
    if let Ok(i) = trimmed.parse::<i64>() {
        return toml_edit::value(i);
    }
    if (trimmed.contains('.') || trimmed.contains('e') || trimmed.contains('E'))
        && let Ok(f) = trimmed.parse::<f64>()
    {
        return toml_edit::value(f);
    }
    if trimmed.starts_with('[')
        && trimmed.ends_with(']')
        && let Ok(parsed) = trimmed.parse::<toml_edit::Value>()
    {
        return toml_edit::Item::Value(parsed);
    }
    toml_edit::value(trimmed)
}

fn set_in_document(
    doc: &mut toml_edit::DocumentMut,
    key_path: &str,
    item: toml_edit::Item,
) -> CliResult<()> {
    let mut parts: Vec<&str> = key_path
        .split('.')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if parts.is_empty() {
        return Err(CliError::Validation("Key cannot be empty.".to_string()));
    }

    if parts.len() > 1 {
        if parts[0] == "live"
            && doc.as_table().contains_key("transcribe_live")
            && !doc.as_table().contains_key("live")
        {
            parts[0] = "transcribe_live";
        } else if parts[0] == "transcribe_live"
            && doc.as_table().contains_key("live")
            && !doc.as_table().contains_key("transcribe_live")
        {
            parts[0] = "live";
        }
    }

    if parts.len() == 1 {
        doc[parts[0]] = item;
        return Ok(());
    }

    let mut current_table = doc.as_table_mut();
    for &part in &parts[..parts.len() - 1] {
        if !current_table.contains_key(part) {
            current_table.insert(part, toml_edit::Item::Table(toml_edit::Table::new()));
        }
        let next_item = current_table.get_mut(part).unwrap();
        match next_item {
            toml_edit::Item::Table(table) => {
                current_table = table;
            }
            _ => {
                return Err(CliError::Validation(format!(
                    "Cannot set '{key_path}': '{part}' exists and is not a table."
                )));
            }
        }
    }

    let last_key = parts.last().unwrap();
    current_table.insert(last_key, item);
    Ok(())
}

fn run_config_set(args: ConfigSetArgs) -> CliResult<CliOutput> {
    let path = resolve_target_file_path(args.target.config.as_ref(), args.target.global)?;
    let file_existed = path.is_file();

    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));

    if !parent.exists() {
        std::fs::create_dir_all(parent).map_err(|e| {
            CliError::Io(format!(
                "Failed to create config directory {}: {e}",
                parent.display()
            ))
        })?;
    }

    let existing_content = if file_existed {
        std::fs::read_to_string(&path)
            .map_err(|e| CliError::Io(format!("Failed to read {}: {e}", path.display())))?
    } else {
        String::new()
    };

    let mut doc: toml_edit::DocumentMut = existing_content.parse().map_err(|e| {
        CliError::Validation(format!("Invalid TOML syntax in {}: {e}", path.display()))
    })?;

    let parsed_item = parse_toml_value(&args.value);
    set_in_document(&mut doc, &args.key, parsed_item)?;

    let new_content = doc.to_string();

    let mut temp_file = tempfile::NamedTempFile::new_in(parent)
        .map_err(|e| CliError::Io(format!("Failed to create temporary config file: {e}")))?;

    std::io::Write::write_all(&mut temp_file, new_content.as_bytes())
        .map_err(|e| CliError::Io(format!("Failed to write temporary config file: {e}")))?;

    std::io::Write::flush(&mut temp_file)
        .map_err(|e| CliError::Io(format!("Failed to flush temporary config file: {e}")))?;

    if let Err(err) = check_config_file(temp_file.path()) {
        return Err(CliError::Validation(format!(
            "Failed to set '{} = {}': resulting configuration is invalid: {}",
            args.key, args.value, err
        )));
    }

    temp_file
        .persist(&path)
        .map_err(|e| CliError::Io(format!("Failed to save config to {}: {e}", path.display())))?;

    Ok(CliOutput::stdout(format!(
        "Set {} = {} in {}",
        args.key,
        args.value,
        path.display()
    )))
}

fn unset_in_document(doc: &mut toml_edit::DocumentMut, key_path: &str) -> CliResult<()> {
    let mut parts: Vec<&str> = key_path
        .split('.')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if parts.is_empty() {
        return Err(CliError::Validation("Key cannot be empty.".to_string()));
    }

    if parts.len() > 1 {
        if parts[0] == "live"
            && doc.as_table().contains_key("transcribe_live")
            && !doc.as_table().contains_key("live")
        {
            parts[0] = "transcribe_live";
        } else if parts[0] == "transcribe_live"
            && doc.as_table().contains_key("live")
            && !doc.as_table().contains_key("transcribe_live")
        {
            parts[0] = "live";
        }
    }

    if parts.len() == 1 {
        if doc.as_table_mut().remove(parts[0]).is_some() {
            return Ok(());
        } else {
            return Err(CliError::Validation(format!("Key '{key_path}' not found.")));
        }
    }

    let mut current_table = doc.as_table_mut();
    for &part in &parts[..parts.len() - 1] {
        match current_table.get_mut(part) {
            Some(toml_edit::Item::Table(table)) => {
                current_table = table;
            }
            _ => {
                return Err(CliError::Validation(format!("Key '{key_path}' not found.")));
            }
        }
    }

    let last_key = parts.last().unwrap();
    if current_table.remove(last_key).is_some() {
        Ok(())
    } else {
        Err(CliError::Validation(format!("Key '{key_path}' not found.")))
    }
}

fn run_config_unset(args: ConfigUnsetArgs) -> CliResult<CliOutput> {
    let path = resolve_existing_file_path(args.target.config.as_ref(), args.target.global)?;

    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));

    let existing_content = std::fs::read_to_string(&path)
        .map_err(|e| CliError::Io(format!("Failed to read {}: {e}", path.display())))?;

    let mut doc: toml_edit::DocumentMut = existing_content.parse().map_err(|e| {
        CliError::Validation(format!("Invalid TOML syntax in {}: {e}", path.display()))
    })?;

    unset_in_document(&mut doc, &args.key).map_err(|_| {
        CliError::Validation(format!(
            "Key '{}' not found in {}.",
            args.key,
            path.display()
        ))
    })?;

    let new_content = doc.to_string();

    let mut temp_file = tempfile::NamedTempFile::new_in(parent)
        .map_err(|e| CliError::Io(format!("Failed to create temporary config file: {e}")))?;

    std::io::Write::write_all(&mut temp_file, new_content.as_bytes())
        .map_err(|e| CliError::Io(format!("Failed to write temporary config file: {e}")))?;

    std::io::Write::flush(&mut temp_file)
        .map_err(|e| CliError::Io(format!("Failed to flush temporary config file: {e}")))?;

    if let Err(err) = check_config_file(temp_file.path()) {
        return Err(CliError::Validation(format!(
            "Failed to unset '{}': resulting configuration is invalid: {}",
            args.key, err
        )));
    }

    temp_file
        .persist(&path)
        .map_err(|e| CliError::Io(format!("Failed to save config to {}: {e}", path.display())))?;

    Ok(CliOutput::stdout(format!(
        "Removed {} from {}",
        args.key,
        path.display()
    )))
}

fn run_config_edit(args: ConfigEditArgs) -> CliResult<CliOutput> {
    let path = if args.target.global {
        let global_path =
            crate::init_config::canonical_user_config_path(&|name| std::env::var_os(name))
                .ok_or_else(|| {
                    CliError::Validation(
                        "Could not determine standard user configuration directory.".to_string(),
                    )
                })?;
        if !global_path.is_file() {
            let _ = crate::init_config::run_init_config(crate::init_config::InitConfigArgs {
                path: None,
                global: true,
                force: false,
            })?;
        }
        global_path
    } else {
        match crate::init_config::resolve_config_path(args.target.config.as_ref()) {
            Some(path) => path,
            None => {
                let default_path = args
                    .target
                    .config
                    .unwrap_or_else(|| PathBuf::from(crate::init_config::DEFAULT_CONFIG_PATH));
                let _ = crate::init_config::run_init_config(crate::init_config::InitConfigArgs {
                    path: Some(default_path.clone()),
                    global: false,
                    force: false,
                })?;
                default_path
            }
        }
    };

    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| {
            if cfg!(windows) {
                "notepad".to_string()
            } else {
                "vi".to_string()
            }
        });

    let tokens = shlex::split(&editor).ok_or_else(|| {
        CliError::Validation(format!(
            "Failed to parse editor command '{editor}': mismatched or unclosed quotes."
        ))
    })?;
    if tokens.is_empty() {
        return Err(CliError::Validation("Editor command is empty.".to_string()));
    }
    let (program, extra_args) = (&tokens[0], &tokens[1..]);

    let status = std::process::Command::new(program)
        .args(extra_args)
        .arg(&path)
        .status()
        .map_err(|e| CliError::Io(format!("Failed to launch editor '{editor}': {e}")))?;

    if !status.success() {
        return Err(CliError::Io(format!(
            "Editor '{editor}' exited with non-zero status: {status}"
        )));
    }

    check_config_file(&path)
}
