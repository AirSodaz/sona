use std::fs;
use std::path::PathBuf;

use sona_core::cli_runtime::{
    DEFAULT_GPU_ACCELERATION, DEFAULT_SERVE_HOST, DEFAULT_SERVE_IP_WHITELIST, DEFAULT_SERVE_PORT,
    GPU_ACCELERATION_VALUES, ServeConfigSection, ServeRuntimeArgs, TranscribeConfigSection,
    UnifiedConfigFile, load_serve_config_file, resolve_cli_gpu_acceleration,
    resolve_cli_models_dir, resolve_serve_runtime_options,
};
use sona_core::preset_models::{DEFAULT_PUNCTUATION_MODEL_ID, DEFAULT_SILERO_VAD_MODEL_ID};
use tempfile::tempdir;

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

#[test]
fn resolve_cli_models_dir_rejects_existing_file() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("not_a_dir.txt");
    fs::write(&file_path, "dummy").unwrap();

    let error = resolve_cli_models_dir(Some(file_path)).unwrap_err();

    assert!(error.contains("exists but is not a directory"));
}

#[test]
fn load_serve_config_file_reads_shared_and_section_values() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("sona-cli.toml");
    fs::write(
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

#[test]
fn resolve_serve_runtime_options_merges_cli_config_and_defaults() {
    let dir = tempdir().unwrap();
    let models_dir = dir.path().join("models");
    fs::create_dir_all(&models_dir).unwrap();

    let resolved = resolve_serve_runtime_options(
        ServeRuntimeArgs {
            host: Some("0.0.0.0".to_string()),
            models_dir: Some(models_dir.clone()),
            max_concurrent: Some(11),
            ..Default::default()
        },
        Some(ServeConfigSection {
            gpu_acceleration: Some("cuda".to_string()),
            max_concurrent: Some(2),
            ..Default::default()
        }),
    )
    .unwrap();

    assert_eq!(resolved.host, "0.0.0.0");
    assert_eq!(resolved.port, DEFAULT_SERVE_PORT);
    assert_eq!(resolved.models_dir, models_dir);
    assert_eq!(resolved.ip_whitelist, DEFAULT_SERVE_IP_WHITELIST);
    assert_eq!(resolved.max_concurrent, 11);
    assert_eq!(resolved.max_streaming, 2);
    assert_eq!(
        resolved.transcription_defaults.gpu_acceleration.as_deref(),
        Some("cuda")
    );
    assert_eq!(
        resolved.transcription_defaults.vad_model_id.as_deref(),
        Some(DEFAULT_SILERO_VAD_MODEL_ID)
    );
    assert_eq!(
        resolved
            .transcription_defaults
            .punctuation_model_id
            .as_deref(),
        Some(DEFAULT_PUNCTUATION_MODEL_ID)
    );
    assert_ne!(resolved.host, DEFAULT_SERVE_HOST);
}
