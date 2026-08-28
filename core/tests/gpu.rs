use sona_core::runtime::gpu::{
    DEFAULT_GPU_ACCELERATION, GPU_ACCELERATION_VALUES, resolve_gpu_acceleration,
};

#[test]
fn gpu_acceleration_defaults_and_normalizes_without_cli_runtime() {
    assert_eq!(DEFAULT_GPU_ACCELERATION, "auto");
    assert!(GPU_ACCELERATION_VALUES.contains(&"auto"));
    assert!(GPU_ACCELERATION_VALUES.contains(&"cpu"));
    assert!(GPU_ACCELERATION_VALUES.contains(&"vulkan"));
    assert!(GPU_ACCELERATION_VALUES.contains(&"metal"));
    assert!(GPU_ACCELERATION_VALUES.contains(&"cuda"));
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
    assert_eq!(
        resolve_gpu_acceleration(Some("vulkan".to_string()))
            .unwrap()
            .as_deref(),
        Some("vulkan")
    );
    assert_eq!(
        resolve_gpu_acceleration(Some("metal".to_string()))
            .unwrap()
            .as_deref(),
        Some("metal")
    );
    assert_eq!(
        resolve_gpu_acceleration(Some("coreml".to_string()))
            .unwrap()
            .as_deref(),
        Some("metal")
    );
    assert_eq!(
        resolve_gpu_acceleration(Some("directml".to_string()))
            .unwrap()
            .as_deref(),
        Some("auto")
    );
}

#[test]
fn gpu_acceleration_rejects_unknown_values() {
    let error = resolve_gpu_acceleration(Some("invalid_accel".to_string())).unwrap_err();
    assert_eq!(error.subject, "gpu_acceleration");
    assert!(error.message.contains("gpu_acceleration must be one of"));
    assert!(error.message.contains("vulkan"));
    assert!(error.message.contains("metal"));
}
