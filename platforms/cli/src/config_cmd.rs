use clap::{Args, Subcommand};
use std::path::PathBuf;

use crate::{CliError, CliOutput, CliResult};

#[derive(Debug, Args)]
#[command(
    about = "Inspect and manage Sona CLI configuration",
    after_help = "Examples:\n  sona-cli config init\n  sona-cli config init ./custom.toml -F\n  sona-cli config path\n  sona-cli config check\n  sona-cli config show"
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

pub fn run_config(args: ConfigArgs) -> CliResult<CliOutput> {
    match args.command {
        ConfigCommands::Init(init_args) => crate::init_config::run_init_config(init_args),
        ConfigCommands::Path(path_args) => run_config_path(path_args),
        ConfigCommands::Check(check_args) => run_config_check(check_args),
        ConfigCommands::Show(show_args) => run_config_show(show_args),
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
