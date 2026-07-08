use std::fs;

use serde_json::json;
use sona_core::models::preset_models::{DEFAULT_PUNCTUATION_MODEL_ID, DEFAULT_SILERO_VAD_MODEL_ID};
use sona_core::runtime::config::ServeConfigSection;
use sona_core::runtime::serve::{
    DEFAULT_SERVE_HOST, DEFAULT_SERVE_IP_WHITELIST, DEFAULT_SERVE_PORT, ServeRuntimeArgs,
    resolve_serve_runtime_options, serve_startup_settings_from_app_config,
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

#[test]
fn serve_startup_settings_from_app_config_reads_wrapped_legacy_payload() {
    let settings = serve_startup_settings_from_app_config(&json!({
        "sona-config": {
            "httpServerEnabled": true,
            "httpServerHost": "0.0.0.0",
            "httpServerPort": 15555,
            "httpServerApiKey": "startup-secret",
            "httpServerMaxConcurrent": 3,
            "httpServerMaxQueueSize": 12,
            "httpServerMaxUploadSizeMB": 99,
            "httpServerJobTtlMinutes": 7,
            "httpServerMaxStreaming": 4,
            "httpServerIpWhitelist": "127.0.0.1/32",
            "gpuAcceleration": "cpu"
        }
    }));

    assert!(settings.enabled);
    assert_eq!(settings.config.host.as_deref(), Some("0.0.0.0"));
    assert_eq!(settings.config.port, Some(15555));
    assert_eq!(settings.config.api_key.as_deref(), Some("startup-secret"));
    assert_eq!(settings.config.max_concurrent, Some(3));
    assert_eq!(settings.config.max_queue_size, Some(12));
    assert_eq!(settings.config.max_upload_size_mb, Some(99));
    assert_eq!(settings.config.job_ttl_minutes, Some(7));
    assert_eq!(settings.config.max_streaming, Some(4));
    assert_eq!(
        settings.config.ip_whitelist.as_deref(),
        Some("127.0.0.1/32")
    );
    assert_eq!(settings.config.gpu_acceleration.as_deref(), Some("cpu"));
}

#[test]
fn serve_startup_settings_from_app_config_ignores_non_object_wrappers() {
    let settings = serve_startup_settings_from_app_config(&json!({
        "config": null,
        "httpServerEnabled": true,
        "httpServerPort": 16666
    }));

    assert!(settings.enabled);
    assert_eq!(settings.config.port, Some(16666));
}
