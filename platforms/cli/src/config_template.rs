use std::path::Path;

const CONFIG_TEMPLATE: &str = r#"# Sona CLI config template
# Generated keys are commented out by default. Uncomment the settings you want
# before using this file with Sona commands.
# `sona-cli transcribe` requires model_id to be enabled.
# Save as sona-cli.toml, then pass it with:
#   sona-cli transcribe ./sample.wav -c ./sona-cli.toml
#   sona-cli serve -c ./sona-cli.toml
#
# Top-level keys are shared defaults for both commands.
# Uncomment the same key inside [transcribe] or [serve] to override it per command.

# Shared model location. If omitted, Sona tries the desktop app models directory.
{models_dir_line}

# gpu_acceleration = "auto"
# vad_model_id = "silero-vad"
# punctuation_model_id = "sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8"

[transcribe]
# models_dir = "..."
# gpu_acceleration = "auto"
# vad_model_id = "silero-vad"
# punctuation_model_id = "sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8"
# model_id = "sherpa-onnx-whisper-turbo"
# language = "auto"
# threads = 4
# enable_itn = false
# vad_buffer_size = 5.0
# hotwords = "Sona,offline ASR"
# format = "srt"
# quiet = false
# jobs = 1

[serve]
# models_dir = "..."
# gpu_acceleration = "auto"
# vad_model_id = "silero-vad"
# punctuation_model_id = "sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8"
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

    CONFIG_TEMPLATE.replace("{models_dir_line}", &models_dir_line)
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
        assert!(content.contains("[serve]"));
        assert!(content.contains("# model_id = \"sherpa-onnx-whisper-turbo\""));
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
