use clap::Args;
use std::fs;
use std::path::{Path, PathBuf};

use crate::{CliError, CliOutput, CliResult};

const DEFAULT_CONFIG_PATH: &str = "sona-cli.toml";

#[derive(Debug, Args)]
#[command(
    about = "Create a commented TOML starter template",
    after_help = "Examples:\n  sona-cli init-config\n  sona-cli init-config ./sona-cli.toml\n  sona-cli init-config ./sona-cli.toml --force\n\nThe generated file is fully commented out. Uncomment the keys you need before using it with --config.\n`sona-cli transcribe` requires model_id to be enabled."
)]
pub struct InitConfigArgs {
    /// Target TOML path. Defaults to ./sona-cli.toml.
    #[arg(
        value_name = "PATH",
        help = "Path to write the commented starter template, default sona-cli.toml"
    )]
    path: Option<PathBuf>,
    /// Overwrite the target file if it already exists.
    #[arg(long, help = "Overwrite an existing starter template or config file")]
    force: bool,
}

pub fn run_init_config(args: InitConfigArgs) -> CliResult<CliOutput> {
    let path = args
        .path
        .unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH));
    write_config_template(&path, args.force)?;
    Ok(CliOutput::stderr(format!(
        "Created config template at {}",
        path.display()
    )))
}

fn generate_config_content() -> String {
    sona_core::cli_config::render_cli_config_template(
        crate::desktop_paths::default_models_dir().as_deref(),
    )
}

fn write_config_template(path: &Path, force: bool) -> CliResult<()> {
    if path.exists() && !force {
        return Err(CliError::Io(format!(
            "Config file already exists: {}. Use --force to overwrite.",
            path.display()
        )));
    }

    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| {
            CliError::Io(format!(
                "Failed to create config directory {}: {error}",
                parent.display()
            ))
        })?;
    }

    fs::write(path, generate_config_content()).map_err(|error| {
        CliError::Io(format!(
            "Failed to write config file {}: {error}",
            path.display()
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_contains_shared_transcribe_and_serve_keys() {
        let content = sona_core::cli_config::render_cli_config_template(None);
        for key in [
            "model_id",
            "vad_model_id",
            "punctuation_model_id",
            "language",
            "threads",
            "enable_itn",
            "vad_buffer_size",
            "gpu_acceleration",
            "hotwords",
            "format",
            "quiet",
            "jobs",
            "host",
            "port",
            "api_key",
            "ip_whitelist",
            "max_streaming",
            "max_concurrent",
            "max_queue_size",
            "max_upload_size_mb",
            "job_ttl_minutes",
        ] {
            assert!(
                content.contains(&format!("# {}", key)) || content.contains(&format!("# {key} = ")),
                "template should include commented key {key}"
            );
        }
    }

    #[test]
    fn generated_config_uses_forward_slashes_for_model_path() {
        let path = PathBuf::from("C:\\Users\\test\\models");
        let content = sona_core::cli_config::render_cli_config_template(Some(path.as_path()));

        assert!(content.contains("# models_dir = \"C:/Users/test/models\""));
    }
}
