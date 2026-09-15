pub(crate) use sona_sherpa_onnx::gpu::GpuAccelerationPlan;

/// Checks whether the local ASR adapter runtime can use a compatible GPU backend.
pub async fn check_gpu_availability() -> Result<bool, String> {
    sona_sherpa_onnx::gpu::check_gpu_availability()
        .await
        .map_err(|error| error.to_string())
}

pub(crate) async fn resolve_gpu_acceleration_plan(
    gpu_acceleration: Option<&str>,
    is_int8: bool,
) -> GpuAccelerationPlan {
    sona_sherpa_onnx::gpu::resolve_gpu_acceleration_plan(gpu_acceleration, is_int8).await
}

pub async fn resolve_gpu_acceleration(
    gpu_acceleration: Option<&str>,
    is_int8: bool,
) -> Option<String> {
    let resolved = resolve_gpu_acceleration_plan(gpu_acceleration, is_int8)
        .await
        .provider_options()
        .first()
        .cloned()
        .flatten();
    log::info!("[hardware] Resolved GPU acceleration: {resolved:?}");
    resolved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_check_gpu_availability() {
        let result = check_gpu_availability().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_resolve_gpu_acceleration() {
        #[cfg(not(target_os = "macos"))]
        {
            let result = resolve_gpu_acceleration(Some("cuda"), false).await;
            assert_eq!(result, Some("cuda".to_string()));

            let result_int8 = resolve_gpu_acceleration(Some("cuda"), true).await;
            assert_eq!(result_int8, Some("cpu".to_string()));
        }

        #[cfg(target_os = "macos")]
        {
            let result = resolve_gpu_acceleration(Some("cuda"), false).await;
            assert_eq!(result, Some("cpu".to_string()));

            let result_int8 = resolve_gpu_acceleration(Some("cuda"), true).await;
            assert_eq!(result_int8, Some("cpu".to_string()));
        }

        let result = resolve_gpu_acceleration(Some("cpu"), false).await;
        assert_eq!(result, Some("cpu".to_string()));

        let result = resolve_gpu_acceleration(Some("auto"), false).await;
        assert!(result.is_some());

        let result_none = resolve_gpu_acceleration(None, false).await;
        assert_eq!(result_none, None);
    }
}
