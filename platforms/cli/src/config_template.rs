use std::path::Path;

use sona_core::models::preset_models::{
    DEFAULT_PUNCTUATION_MODEL_ID, DEFAULT_SENSEVOICE_INT8_MODEL_ID, DEFAULT_SILERO_VAD_MODEL_ID,
    DEFAULT_WHISPER_TURBO_MODEL_ID,
};

const CONFIG_TEMPLATE: &str = r#"# Sona CLI config template
# Generated keys are commented out by default. Uncomment the settings you want
# before using this file with Sona commands.
# `sona-cli transcribe` requires either model_id (local) or online_provider (cloud) to be set.
# Online ASR provider selection, api_key_env, and online_config can be set in this file or via CLI flags.
# Save as sona-cli.toml, then pass it with:
#   sona-cli transcribe ./sample.wav -c ./sona-cli.toml
#   sona-cli live -c ./sona-cli.toml
#   ffmpeg -i sample.wav -f s16le -ac 1 -ar 16000 - | sona-cli live --input stdin -c ./sona-cli.toml
#   sona-cli serve -c ./sona-cli.toml
#
# Top-level keys are shared defaults for CLI commands.
# Uncomment the same key inside [transcribe] or [serve] to override it for one command.

# Shared model location. If omitted, Sona tries the desktop app models directory.
{models_dir_line}

# gpu_acceleration = "auto"
# vad_model_id = "{vad_model_id}"
# punctuation_model_id = "{punctuation_model_id}"
# online_provider = "volcengine-doubao"
# api_key_env = "SONA_VOLCENGINE_ASR_API_KEY"
# online_config = "./online-config.json"
# ffmpeg_path = "..."

[transcribe]
# models_dir = "..."
# gpu_acceleration = "auto"
# ffmpeg_path = "..."
# vad_model_id = "{vad_model_id}"
# punctuation_model_id = "{punctuation_model_id}"
# model_id = "{whisper_turbo_model_id}"
# online_provider = "volcengine-doubao"
# api_key_env = "SONA_VOLCENGINE_ASR_API_KEY"
# online_config = "./online-config.json"
# language = "auto"
# threads = 4
# enable_itn = false
# vad_buffer_size = 5.0
# hotwords = "Sona,offline ASR"
# format = "srt"
# quiet = false
# jobs = 1 # Note: Concurrent batch jobs (>1) are not yet supported

[transcribe_live] # (alias: [live])
# Input source: microphone or stdin. stdin must be 16 kHz mono signed 16-bit little-endian PCM.
# input = "microphone"
# Exact CPAL input device name. Only valid with microphone input.
# device = ""
# Stop automatically after this many seconds.
# duration_seconds = 60.0
# Live stdout stream format: text or ndjson (stream_format or output_format).
# stream_format = "text"
# output_format = "text"
# models_dir = "..."
# gpu_acceleration = "auto"
# vad_model_id = "{vad_model_id}"
# punctuation_model_id = "{punctuation_model_id}"
# model_id = "{sensevoice_model_id}"
# online_provider = "volcengine-doubao"
# api_key_env = "SONA_VOLCENGINE_ASR_API_KEY"
# online_config = "./online-config.json"
# language = "auto"
# threads = 4
# enable_itn = false
# vad_buffer_size = 5.0
# hotwords = "Sona,live ASR"
# Final transcript export format: json, txt, srt, vtt, md.
# format = "srt"

[serve]
# models_dir = "..."
# gpu_acceleration = "auto"
# vad_model_id = "{vad_model_id}"
# punctuation_model_id = "{punctuation_model_id}"
# ffmpeg_path = "..."
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

pub fn render_config_template(models_dir: Option<&Path>) -> String {
    let models_dir_line = if let Some(path) = models_dir {
        let path_str = path.to_string_lossy().replace('\\', "/");
        format!("# models_dir = \"{}\"", path_str)
    } else {
        format!("# models_dir = \"{}\"", default_models_dir_placeholder())
    };

    CONFIG_TEMPLATE
        .replace("{models_dir_line}", &models_dir_line)
        .replace("{vad_model_id}", DEFAULT_SILERO_VAD_MODEL_ID)
        .replace("{punctuation_model_id}", DEFAULT_PUNCTUATION_MODEL_ID)
        .replace("{whisper_turbo_model_id}", DEFAULT_WHISPER_TURBO_MODEL_ID)
        .replace("{sensevoice_model_id}", DEFAULT_SENSEVOICE_INT8_MODEL_ID)
}

fn default_models_dir_placeholder() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "C:/Users/you/AppData/Local/com.asoda.sona/models"
    }
    #[cfg(target_os = "macos")]
    {
        "/Users/you/Library/Application Support/com.asoda.sona/models"
    }
    #[cfg(target_os = "linux")]
    {
        "/home/you/.local/share/com.asoda.sona/models"
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        "/path/to/com.asoda.sona/models"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn renders_models_dir_with_forward_slashes() {
        let content = render_config_template(Some(Path::new(r"C:\Users\test\models")));

        assert!(content.contains("# models_dir = \"C:/Users/test/models\""));
        assert!(content.contains("[transcribe]"));
        assert!(content.contains("# model_id = \"sherpa-onnx-whisper-turbo\""));
        assert!(content.contains("[transcribe_live]"));
        assert!(content.contains("# input = \"microphone\""));
        assert!(content.contains("# output_format = \"text\""));
        assert!(content.contains("# stream_format = \"text\""));
        assert!(content.contains("# format = \"srt\""));
        assert!(content.contains("sona-cli live"));
        assert!(content.contains("[serve]"));
        assert!(content.contains("sona-cli serve"));
        assert!(content.contains("# api_key = \"\""));
    }

    #[test]
    fn uses_platform_placeholder_without_models_dir() {
        let content = render_config_template(None);
        let models_line = content
            .lines()
            .find(|line| line.contains("models_dir"))
            .expect("template should include a models_dir comment");

        assert!(models_line.starts_with("# models_dir = \""));
        assert!(!models_line.contains('\\'));
        assert!(content.contains("[transcribe_live]"));

        #[cfg(target_os = "windows")]
        assert!(models_line.contains("C:/Users/you/AppData/Local/com.asoda.sona/models"));

        #[cfg(target_os = "macos")]
        assert!(
            models_line.contains("/Users/you/Library/Application Support/com.asoda.sona/models")
        );

        #[cfg(target_os = "linux")]
        assert!(models_line.contains("/home/you/.local/share/com.asoda.sona/models"));
    }
}
