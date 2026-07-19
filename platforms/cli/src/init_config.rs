use clap::Args;
use std::path::PathBuf;

use crate::{CliOutput, CliResult};

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
    let content = generate_config_content();
    sona_runtime_fs::write_cli_config_template_file(&path, &content, args.force)
        .map_err(|error| crate::CliError::Io(error.to_string()))?;
    Ok(CliOutput::stderr(format!(
        "Created config template at {}",
        path.display()
    )))
}

fn generate_config_content() -> String {
    crate::config_template::render_config_template(
        crate::desktop_paths::default_models_dir().as_deref(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_contains_transcribe_and_serve_keys() {
        let content = crate::config_template::render_config_template(None);
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
        ] {
            assert!(
                content.contains(&format!("# {}", key)) || content.contains(&format!("# {key} = ")),
                "template should include commented key {key}"
            );
        }

        for key in [
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
                "template should include commented serve key {key}"
            );
        }

        assert!(content.contains("[serve]"));
        assert!(content.contains("sona-cli serve"));
    }

    #[test]
    fn generated_config_uses_forward_slashes_for_model_path() {
        let path = PathBuf::from("C:\\Users\\test\\models");
        let content = crate::config_template::render_config_template(Some(path.as_path()));

        assert!(content.contains("# models_dir = \"C:/Users/test/models\""));
    }
}
