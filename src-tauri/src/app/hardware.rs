pub(crate) use sona_local_asr::gpu::{GpuAccelerationPlan, GpuFallbackNotice};

/// Checks whether the local ASR adapter can use a compatible GPU backend.
pub async fn check_gpu_availability() -> Result<bool, String> {
    sona_local_asr::gpu::check_gpu_availability().await
}

pub(crate) async fn resolve_gpu_acceleration_plan(
    gpu_acceleration: Option<&str>,
) -> GpuAccelerationPlan {
    sona_local_asr::gpu::resolve_gpu_acceleration_plan(gpu_acceleration).await
}

pub async fn resolve_gpu_acceleration(gpu_acceleration: Option<&str>) -> Option<String> {
    let resolved = resolve_gpu_acceleration_plan(gpu_acceleration)
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
        let result = resolve_gpu_acceleration(Some("cuda")).await;
        assert_eq!(result, Some("cuda".to_string()));

        let result = resolve_gpu_acceleration(Some("cpu")).await;
        assert_eq!(result, Some("cpu".to_string()));

        let result = resolve_gpu_acceleration(Some("auto")).await;
        assert!(result.is_some());
    }
}
