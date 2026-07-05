use std::path::PathBuf;

use sona_core::cli_runtime::{
    DEFAULT_GPU_ACCELERATION, GPU_ACCELERATION_VALUES, ServeConfigSection, TranscribeConfigSection,
    UnifiedConfigFile, resolve_cli_gpu_acceleration,
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
fn gpu_acceleration_defaults_and_normalizes() {
    assert_eq!(DEFAULT_GPU_ACCELERATION, "auto");
    assert!(GPU_ACCELERATION_VALUES.contains(&"auto"));
    assert!(GPU_ACCELERATION_VALUES.contains(&"cpu"));
    assert_eq!(
        resolve_cli_gpu_acceleration(None).unwrap().as_deref(),
        Some("auto")
    );
    assert_eq!(
        resolve_cli_gpu_acceleration(Some(" CUDA ".to_string()))
            .unwrap()
            .as_deref(),
        Some("cuda")
    );
}

#[test]
fn gpu_acceleration_rejects_unknown_values() {
    let error = resolve_cli_gpu_acceleration(Some("metal".to_string())).unwrap_err();
    assert!(error.contains("gpu_acceleration must be one of"));
}

#[test]
fn runtime_config_sections_default_cleanly() {
    let transcribe = TranscribeConfigSection::default();
    assert!(transcribe.model_id.is_none());

    let serve = ServeConfigSection::default();
    assert!(serve.host.is_none());
}
