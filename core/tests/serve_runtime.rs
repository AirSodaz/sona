use std::fs;

use sona_core::preset_models::{DEFAULT_PUNCTUATION_MODEL_ID, DEFAULT_SILERO_VAD_MODEL_ID};
use sona_core::runtime_config::ServeConfigSection;
use sona_core::serve_runtime::{
    DEFAULT_SERVE_HOST, DEFAULT_SERVE_IP_WHITELIST, DEFAULT_SERVE_PORT, ServeRuntimeArgs,
    resolve_serve_runtime_options,
};
use tempfile::tempdir;

#[test]
fn resolve_serve_runtime_options_merges_args_config_and_defaults() {
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

#[test]
fn resolve_serve_runtime_options_rejects_zero_max_concurrent() {
    let dir = tempdir().unwrap();
    let models_dir = dir.path().join("models");
    fs::create_dir_all(&models_dir).unwrap();

    let error = resolve_serve_runtime_options(
        ServeRuntimeArgs {
            models_dir: Some(models_dir),
            max_concurrent: Some(0),
            ..Default::default()
        },
        None,
    )
    .unwrap_err();

    assert_eq!(error, "max_concurrent must be greater than 0");
}
