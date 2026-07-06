use std::fs;
use std::path::PathBuf;

use sona_core::cli_runtime::{
    DEFAULT_GPU_ACCELERATION, DEFAULT_SERVE_HOST, DEFAULT_SERVE_IP_WHITELIST, DEFAULT_SERVE_PORT,
    GPU_ACCELERATION_VALUES, ServeRuntimeArgs, resolve_cli_gpu_acceleration,
    resolve_serve_runtime_options,
};
use sona_core::preset_models::{DEFAULT_PUNCTUATION_MODEL_ID, DEFAULT_SILERO_VAD_MODEL_ID};
use sona_core::runtime_config::ServeConfigSection;
use tempfile::tempdir;

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
