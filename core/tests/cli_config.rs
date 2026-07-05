use std::path::Path;

use sona_core::cli_config::render_cli_config_template;

#[test]
fn cli_config_template_renders_models_dir_with_forward_slashes() {
    let content = render_cli_config_template(Some(Path::new(r"C:\Users\test\models")));

    assert!(content.contains("# models_dir = \"C:/Users/test/models\""));
    assert!(content.contains("[transcribe]"));
    assert!(content.contains("[serve]"));
    assert!(content.contains("# model_id = \"sherpa-onnx-whisper-turbo\""));
    assert!(content.contains("# api_key = \"\""));
}

#[test]
fn cli_config_template_uses_platform_placeholder_without_models_dir() {
    let content = render_cli_config_template(None);
    let models_line = content
        .lines()
        .find(|line| line.contains("models_dir"))
        .expect("template should include a models_dir comment");

    assert!(models_line.starts_with("# models_dir = \""));
    assert!(!models_line.contains('\\'));

    #[cfg(target_os = "windows")]
    assert!(models_line.contains("C:/Users/you/AppData/Local/com.asoda.sona/models"));

    #[cfg(target_os = "macos")]
    assert!(models_line.contains("/Users/you/Library/Application Support/com.asoda.sona/models"));

    #[cfg(target_os = "linux")]
    assert!(models_line.contains("/home/you/.local/share/com.asoda.sona/models"));
}
