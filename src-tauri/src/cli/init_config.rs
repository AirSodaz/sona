use clap::Args;
use std::fs;
use std::path::{Path, PathBuf};

use crate::cli::{CliError, CliResult};

const DEFAULT_CONFIG_PATH: &str = "sona-cli.toml";

pub const CONFIG_TEMPLATE: &str = r#"# Sona CLI config template
# Generated keys are commented out by default. Uncomment the settings you want
# before using this file with Sona commands.
# `sona transcribe` requires model_id to be enabled.
# `sona serve` falls back to runtime defaults for omitted keys.
# Save as sona-cli.toml, then pass it with:
#   sona transcribe ./sample.wav -c ./sona-cli.toml
#   sona serve -c ./sona-cli.toml
#
# This is a flat TOML file. Each command reads the keys it supports and ignores
# unrelated keys.

# Shared model location. If omitted, Sona tries the desktop app models directory.
{models_dir_line}

# Transcribe defaults
# model_id = "sherpa-onnx-whisper-turbo"
# vad_model_id = "silero-vad"
# punctuation_model_id = "sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8"
# language = "auto"
# threads = 4
# enable_itn = false
# vad_buffer_size = 5.0
# gpu_acceleration = "auto"
# hotwords = "Sona,offline ASR"
# format = "srt"
# quiet = false
# jobs = 1

# Serve defaults
# host = "127.0.0.1"
# port = 14200
# api_key = ""
# ip_whitelist = "localhost"
# max_streaming = 2
# max_concurrent = 2
# max_queue_size = 100
# max_upload_size_mb = 50
# job_ttl_minutes = 60
"#;

#[derive(Debug, Args)]
#[command(
    about = "Create a commented TOML starter template",
    after_help = "Examples:\n  sona init-config\n  sona init-config ./sona-cli.toml\n  sona init-config ./sona-cli.toml --force\n\nThe generated file is fully commented out. Uncomment the keys you need before using it with --config.\n`sona transcribe` requires model_id to be enabled."
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

pub fn run_init_config(args: InitConfigArgs) -> CliResult<()> {
    let path = args
        .path
        .unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH));
    write_config_template(&path, args.force)?;
    eprintln!("Created config template at {}", path.display());
    Ok(())
}

fn generate_config_content() -> String {
    generate_config_content_inner(crate::cli::models::default_models_dir())
}

fn generate_config_content_inner(models_dir: Option<PathBuf>) -> String {
    let models_dir_line = if let Some(path) = models_dir {
        // Format path with forward slashes even on Windows for TOML compatibility
        let path_str = path.to_string_lossy().replace('\\', "/");
        format!("# models_dir = \"{}\"", path_str)
    } else {
        // Fallback placeholder based on target OS
        #[cfg(target_os = "windows")]
        let default_path = "C:/Users/you/AppData/Local/com.asoda.sona/models";
        #[cfg(target_os = "macos")]
        let default_path = "/Users/you/Library/Application Support/com.asoda.sona/models";
        #[cfg(target_os = "linux")]
        let default_path = "/home/you/.local/share/com.asoda.sona/models";
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        let default_path = "/path/to/com.asoda.sona/models";

        format!("# models_dir = \"{}\"", default_path)
    };

    CONFIG_TEMPLATE.replace("{models_dir_line}", &models_dir_line)
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

    let content = generate_config_content();
    fs::write(path, content).map_err(|error| {
        CliError::Io(format!(
            "Failed to write config file {}: {error}",
            path.display()
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn template_contains_transcribe_and_serve_keys() {
        let content = generate_config_content();
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
        assert!(
            content.contains("models_dir"),
            "template should include models_dir"
        );
    }

    #[test]
    fn generated_config_has_correct_os_path_format() {
        let test_path = PathBuf::from("C:\\Users\\test\\models");
        let content = generate_config_content_inner(Some(test_path));
        assert!(content.contains("# models_dir = \"C:/Users/test/models\""));

        let content_fallback = generate_config_content_inner(None);
        let models_line = content_fallback
            .lines()
            .find(|l| l.contains("models_dir"))
            .unwrap();
        assert!(
            !models_line.contains('\\'),
            "fallback path should use forward slashes: {}",
            models_line
        );
    }

    #[test]
    fn write_config_template_creates_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("sona-cli.toml");

        write_config_template(&path, false).unwrap();

        let contents = fs::read_to_string(path).unwrap();
        assert!(contents.contains("# Sona CLI config template"));
        assert!(contents.contains("model_id"));
        assert!(contents.contains("api_key"));
    }

    #[test]
    fn write_config_template_rejects_existing_file_without_force() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("sona-cli.toml");
        fs::write(&path, "existing = true\n").unwrap();

        let error = write_config_template(&path, false).unwrap_err();

        assert!(error.to_string().contains("--force"));
        assert_eq!(fs::read_to_string(path).unwrap(), "existing = true\n");
    }

    #[test]
    fn write_config_template_force_overwrites_existing_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("sona-cli.toml");
        fs::write(&path, "existing = true\n").unwrap();

        write_config_template(&path, true).unwrap();

        let contents = fs::read_to_string(path).unwrap();
        assert!(!contents.contains("existing = true"));
        assert!(contents.contains("# Sona CLI config template"));
    }
}
