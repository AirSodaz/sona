use clap::{Args, Subcommand};
use std::path::PathBuf;

use crate::{CliError, CliOutput, CliResult};

#[derive(Debug, Args)]
#[command(
    about = "Inspect and manage Sona CLI configuration",
    after_help = "Examples:\n  sona-cli config init\n  sona-cli config init ./custom.toml -F\n  sona-cli config path\n  sona-cli config check\n  sona-cli config show\n  sona-cli config get transcribe.model_id\n  sona-cli config edit"
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
        after_help = "Examples:\n  sona-cli config get transcribe.model_id\n  sona-cli config get serve.port"
    )]
    Get(ConfigGetArgs),

    /// Open the configuration file in $EDITOR and validate syntax upon exit.
    #[command(
        about = "Open the configuration file in $EDITOR and validate syntax upon exit",
        after_help = "Examples:\n  sona-cli config edit\n  sona-cli config edit -c ./custom.toml"
    )]
    Edit(ConfigEditArgs),
}

#[derive(Debug, Args)]
pub struct ConfigPathArgs {
    /// Optional configuration file path override.
    #[arg(short = 'c', long = "config", value_name = "FILE")]
    pub config: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ConfigCheckArgs {
    /// Optional configuration file path override.
    #[arg(short = 'c', long = "config", value_name = "FILE")]
    pub config: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ConfigShowArgs {
    /// Optional configuration file path override.
    #[arg(short = 'c', long = "config", value_name = "FILE")]
    pub config: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ConfigGetArgs {
    /// Dot-separated key path, e.g. transcribe.model_id or serve.port.
    #[arg(value_name = "KEY")]
    pub key: String,
    /// Optional config file to inspect.
    #[arg(short = 'c', long = "config", value_name = "FILE")]
    pub config: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ConfigEditArgs {
    /// Optional config file to edit.
    #[arg(short = 'c', long = "config", value_name = "FILE")]
    pub config: Option<PathBuf>,
}

pub fn run_config(args: ConfigArgs) -> CliResult<CliOutput> {
    match args.command {
        ConfigCommands::Init(init_args) => crate::init_config::run_init_config(init_args),
        ConfigCommands::Path(path_args) => run_config_path(path_args),
        ConfigCommands::Check(check_args) => run_config_check(check_args),
        ConfigCommands::Show(show_args) => run_config_show(show_args),
        ConfigCommands::Get(get_args) => run_config_get(get_args),
        ConfigCommands::Edit(edit_args) => run_config_edit(edit_args),
    }
}

fn run_config_path(args: ConfigPathArgs) -> CliResult<CliOutput> {
    let resolved = crate::init_config::resolve_config_path(args.config.as_ref());
    match resolved {
        Some(path) => Ok(CliOutput::stdout(path.display().to_string())),
        None => Err(CliError::Validation(
            "No configuration file found. Run 'sona-cli config init' to create one.".to_string(),
        )),
    }
}

fn run_config_check(args: ConfigCheckArgs) -> CliResult<CliOutput> {
    let resolved = crate::init_config::resolve_config_path(args.config.as_ref());
    let path = resolved.ok_or_else(|| {
        CliError::Validation(
            "No configuration file found to check. Run 'sona-cli config init' to create one."
                .to_string(),
        )
    })?;

    let mut errors = Vec::new();
    if let Err(err) = sona_runtime_fs::load_transcribe_config_file(&path) {
        errors.push(format!("[transcribe]: {err}"));
    }
    if let Err(err) = sona_runtime_fs::load_transcribe_live_config_file(&path) {
        errors.push(format!("[transcribe_live]: {err}"));
    }
    if let Err(err) = sona_runtime_fs::load_serve_config_file(&path) {
        errors.push(format!("[serve]: {err}"));
    }

    if errors.is_empty() {
        Ok(CliOutput::stdout(format!(
            "Configuration at {} is valid.",
            path.display()
        )))
    } else {
        Err(CliError::Validation(format!(
            "Configuration error in {}: {}",
            path.display(),
            errors.join("; ")
        )))
    }
}

fn run_config_show(args: ConfigShowArgs) -> CliResult<CliOutput> {
    let resolved = crate::init_config::resolve_config_path(args.config.as_ref());
    let path = resolved.ok_or_else(|| {
        CliError::Validation(
            "No configuration file found. Run 'sona-cli config init' to create one.".to_string(),
        )
    })?;

    let content = std::fs::read_to_string(&path)
        .map_err(|e| CliError::Io(format!("Failed to read {}: {e}", path.display())))?;
    Ok(CliOutput::stdout(content))
}

fn run_config_get(args: ConfigGetArgs) -> CliResult<CliOutput> {
    let resolved = crate::init_config::resolve_config_path(args.config.as_ref());
    let path = resolved.ok_or_else(|| {
        CliError::Validation(
            "No configuration file found. Run 'sona-cli config init' to create one.".to_string(),
        )
    })?;

    let content = std::fs::read_to_string(&path)
        .map_err(|e| CliError::Io(format!("Failed to read {}: {e}", path.display())))?;
    let toml_val: toml::Value = toml::from_str(&content).map_err(|e| {
        CliError::Validation(format!("Invalid TOML syntax in {}: {e}", path.display()))
    })?;

    let parts: Vec<&str> = args.key.split('.').collect();
    let mut current = &toml_val;
    for part in parts {
        match current.get(part) {
            Some(next) => current = next,
            None => {
                return Err(CliError::Validation(format!(
                    "Key '{}' not found in {}",
                    args.key,
                    path.display()
                )));
            }
        }
    }

    let rendered = match current {
        toml::Value::String(s) => s.clone(),
        toml::Value::Integer(i) => i.to_string(),
        toml::Value::Float(f) => f.to_string(),
        toml::Value::Boolean(b) => b.to_string(),
        other => other.to_string(),
    };

    Ok(CliOutput::stdout(rendered))
}

fn run_config_edit(args: ConfigEditArgs) -> CliResult<CliOutput> {
    let path = match crate::init_config::resolve_config_path(args.config.as_ref()) {
        Some(path) => path,
        None => {
            let default_path = args
                .config
                .unwrap_or_else(|| PathBuf::from(crate::init_config::DEFAULT_CONFIG_PATH));
            let _ = crate::init_config::run_init_config(crate::init_config::InitConfigArgs {
                path: Some(default_path.clone()),
                global: false,
                force: false,
            })?;
            default_path
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

    run_config_check(ConfigCheckArgs { config: Some(path) })
}
