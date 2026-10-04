use clap::Args;
use std::path::PathBuf;

use crate::{CliOutput, CliResult};

pub(crate) const DEFAULT_CONFIG_PATH: &str = "sona-cli.toml";

pub(crate) fn resolve_config_path(configured: Option<&PathBuf>) -> Option<PathBuf> {
    resolve_config_path_with_env(
        configured,
        std::path::Path::new(DEFAULT_CONFIG_PATH),
        |name| std::env::var_os(name),
    )
}

pub(crate) fn resolve_config_path_with_env<F>(
    configured: Option<&PathBuf>,
    default_path: &std::path::Path,
    read_env: F,
) -> Option<PathBuf>
where
    F: FnOnce(&str) -> Option<std::ffi::OsString>,
{
    if let Some(path) = configured {
        return Some(path.clone());
    }
    if let Some(env_path) = read_env("SONA_CONFIG").filter(|s| !s.is_empty()) {
        let env_path = PathBuf::from(env_path);
        if env_path.is_file() {
            return Some(env_path);
        }
    }
    if default_path.is_file() {
        return Some(default_path.to_path_buf());
    }
    None
}
#[derive(Debug, Args)]
#[command(
    about = "Create a commented TOML starter template",
    after_help = "Examples:\n  sona-cli init-config\n  sona-cli init-config ./sona-cli.toml\n  sona-cli init-config ./sona-cli.toml -F\n\nThe generated file is fully commented out. Uncomment the keys you need before using it with --config.\n`sona-cli transcribe` requires either model_id (local) or online_provider (cloud) to be configured."
)]
pub struct InitConfigArgs {
    /// Target TOML path. Defaults to ./sona-cli.toml.
    #[arg(
        value_name = "PATH",
        help = "Path to write the commented starter template, default sona-cli.toml"
    )]
    path: Option<PathBuf>,
    /// Overwrite the target file if it already exists.
    #[arg(
        short = 'F',
        long,
        help = "Overwrite an existing starter template or config file"
    )]
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
            "ffmpeg_path",
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

    #[test]
    fn resolve_config_path_precedence() {
        use std::ffi::OsString;
        let dir = tempfile::tempdir().unwrap();
        let cli_file = dir.path().join("cli.toml");
        let env_file = dir.path().join("env.toml");
        let default_file = dir.path().join("default.toml");
        let non_existent = dir.path().join("non_existent.toml");
        std::fs::write(&cli_file, "").unwrap();
        std::fs::write(&env_file, "").unwrap();
        std::fs::write(&default_file, "").unwrap();

        // 1. CLI flag takes highest precedence over env var and default file
        let resolved = resolve_config_path_with_env(Some(&cli_file), &default_file, |_| {
            Some(OsString::from(&env_file))
        });
        assert_eq!(resolved, Some(cli_file.clone()));

        // 2. When no CLI flag, valid SONA_CONFIG takes precedence over default file
        let resolved = resolve_config_path_with_env(None, &default_file, |name| {
            assert_eq!(name, "SONA_CONFIG");
            Some(OsString::from(&env_file))
        });
        assert_eq!(resolved, Some(env_file));

        // 3. When no CLI flag and empty/absent SONA_CONFIG, falls back to existing default file
        let resolved = resolve_config_path_with_env(None, &default_file, |_| Some(OsString::new()));
        assert_eq!(resolved, Some(default_file));

        // 4. When default file does not exist either, returns None
        let resolved = resolve_config_path_with_env(None, &non_existent, |_| None);
        assert_eq!(resolved, None);
    }
}
