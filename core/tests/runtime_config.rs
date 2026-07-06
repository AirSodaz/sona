use std::path::PathBuf;

use sona_core::runtime_config::{
    ServeConfigSection, TranscribeConfigSection, UnifiedConfigFile, load_serve_config_file,
};

#[test]
fn unified_config_merges_shared_values_for_transcribe_and_serve() {
    let toml_str = r#"
models_dir = "/shared/models"
gpu_acceleration = "cuda"
model_id = "legacy-model"
host = "0.0.0.0"
port = 8080

[transcribe]
language = "ja"

[serve]
port = 15000
"#;

    let unified: UnifiedConfigFile = toml::from_str(toml_str).unwrap();

    let transcribe = unified.clone().into_transcribe_config();
    assert_eq!(transcribe.models_dir, Some(PathBuf::from("/shared/models")));
    assert_eq!(transcribe.gpu_acceleration.as_deref(), Some("cuda"));
    assert_eq!(transcribe.model_id.as_deref(), Some("legacy-model"));
    assert_eq!(transcribe.language.as_deref(), Some("ja"));

    let serve = unified.into_serve_config();
    assert_eq!(serve.models_dir, Some(PathBuf::from("/shared/models")));
    assert_eq!(serve.gpu_acceleration.as_deref(), Some("cuda"));
    assert_eq!(serve.host.as_deref(), Some("0.0.0.0"));
    assert_eq!(serve.port, Some(15000));
}

#[test]
fn runtime_config_sections_default_cleanly() {
    let transcribe = TranscribeConfigSection::default();
    assert!(transcribe.model_id.is_none());

    let serve = ServeConfigSection::default();
    assert!(serve.host.is_none());
}

#[test]
fn load_serve_config_file_reads_shared_and_section_values() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("sona-cli.toml");
    std::fs::write(
        &config_path,
        r#"
models_dir = "/shared/models"
gpu_acceleration = "cuda"
host = "0.0.0.0"

[serve]
port = 15000
"#,
    )
    .unwrap();

    let serve = load_serve_config_file(&config_path).unwrap();

    assert_eq!(serve.models_dir, Some(PathBuf::from("/shared/models")));
    assert_eq!(serve.gpu_acceleration.as_deref(), Some("cuda"));
    assert_eq!(serve.host.as_deref(), Some("0.0.0.0"));
    assert_eq!(serve.port, Some(15000));
}
