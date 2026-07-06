use sona_core::gpu::{DEFAULT_GPU_ACCELERATION, GPU_ACCELERATION_VALUES, resolve_gpu_acceleration};

#[test]
fn gpu_acceleration_defaults_and_normalizes_without_cli_runtime() {
    assert_eq!(DEFAULT_GPU_ACCELERATION, "auto");
    assert!(GPU_ACCELERATION_VALUES.contains(&"auto"));
    assert!(GPU_ACCELERATION_VALUES.contains(&"cpu"));
    assert_eq!(
        resolve_gpu_acceleration(None).unwrap().as_deref(),
        Some("auto")
    );
    assert_eq!(
        resolve_gpu_acceleration(Some(" CUDA ".to_string()))
            .unwrap()
            .as_deref(),
        Some("cuda")
    );
}

#[test]
fn gpu_acceleration_rejects_unknown_values() {
    let error = resolve_gpu_acceleration(Some("metal".to_string())).unwrap_err();
    assert!(error.contains("gpu_acceleration must be one of"));
}
