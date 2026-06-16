use clap::Args;
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_CONFIG_PATH: &str = "sona-cli.toml";

pub const CONFIG_TEMPLATE: &str = r#"# Sona CLI config template
# Save as sona-cli.toml, then pass it with:
#   sona transcribe ./sample.wav -c ./sona-cli.toml
#   sona serve -c ./sona-cli.toml
#
# This is a flat TOML file. Each command reads the keys it supports and ignores
# unrelated keys.

# Shared model location. If omitted, Sona tries the desktop app models directory.
models_dir = "C:/Users/you/AppData/Local/com.asoda.sona/models"

# Transcribe defaults
model_id = "sherpa-onnx-whisper-turbo"
vad_model_id = "silero-vad"
punctuation_model_id = "sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8"
language = "auto"
threads = 4
enable_itn = false
vad_buffer_size = 5.0
gpu_acceleration = "auto"
hotwords = "Sona,offline ASR"
format = "srt"
quiet = false
jobs = 1

# Serve defaults
host = "127.0.0.1"
port = 14200
api_key = ""
ip_whitelist = "localhost"
max_streaming = 2
max_concurrent = 2
max_queue_size = 100
max_upload_size_mb = 50
job_ttl_minutes = 60
"#;

#[derive(Debug, Args)]
#[command(
    about = "Create a commented TOML config template",
    after_help = "Examples:\n  sona init-config\n  sona init-config ./sona-cli.toml\n  sona init-config ./sona-cli.toml --force"
)]
pub struct InitConfigArgs {
    /// Target TOML path. Defaults to ./sona-cli.toml.
    #[arg(
        value_name = "PATH",
        help = "Config path to create, default sona-cli.toml"
    )]
    path: Option<PathBuf>,
    /// Overwrite the target file if it already exists.
    #[arg(long, help = "Overwrite an existing config file")]
    force: bool,
}

pub fn run_init_config(args: InitConfigArgs) -> Result<(), String> {
    let path = args
        .path
        .unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH));
    write_config_template(&path, args.force)?;
    eprintln!("Created config template at {}", path.display());
    Ok(())
}

fn write_config_template(path: &Path, force: bool) -> Result<(), String> {
    if path.exists() && !force {
        return Err(format!(
            "Config file already exists: {}. Use --force to overwrite.",
            path.display()
        ));
    }

    if let Some(parent) = path.parent() && !parent.as_os_str().is_empty() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Failed to create config directory {}: {error}",
                parent.display()
            )
        })?;
    }

    fs::write(path, CONFIG_TEMPLATE)
        .map_err(|error| format!("Failed to write config file {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn template_contains_transcribe_and_serve_keys() {
        for key in [
            "models_dir",
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
                CONFIG_TEMPLATE.contains(key),
                "template should include {key}"
            );
        }
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

        assert!(error.contains("--force"));
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
