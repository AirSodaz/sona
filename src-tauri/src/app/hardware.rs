/// Checks if a compatible GPU is available for acceleration.
///
/// On macOS, checks for Apple Silicon (arm64).
/// On other platforms (Windows/Linux), checks for NVIDIA GPUs via `nvidia-smi`.
///
/// # Returns
///
/// Returns `Ok(true)` if a compatible GPU is found, `Ok(false)` if not, or an `Err` containing
/// an error message if the check fails in an unexpected way.
pub async fn check_gpu_availability() -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        use std::env;
        // Check for Apple Silicon (arm64)
        Ok(env::consts::ARCH == "aarch64")
    }

    #[cfg(not(target_os = "macos"))]
    {
        use tokio::process::Command;
        // Check for NVIDIA GPU via nvidia-smi
        // Using "which" or "where" first might be safer but calling it directly works if in PATH
        let is_available = Command::new("nvidia-smi")
            .output()
            .await
            .map(|output| output.status.success())
            .unwrap_or(false);

        Ok(is_available)
    }
}

pub async fn resolve_gpu_acceleration(gpu_acceleration: Option<&str>) -> Option<String> {
    let gpu = gpu_acceleration?;
    if gpu != "auto" {
        return Some(gpu.to_string());
    }

    #[cfg(target_os = "macos")]
    {
        if std::env::consts::ARCH == "aarch64" {
            log::info!("[hardware] Auto-resolved GPU acceleration: coreml");
            Some("coreml".to_string())
        } else {
            log::info!("[hardware] Auto-resolved GPU acceleration: cpu");
            Some("cpu".to_string())
        }
    }

    #[cfg(target_os = "windows")]
    {
        if check_gpu_availability().await.unwrap_or(false) {
            log::info!("[hardware] Auto-resolved GPU acceleration: cuda");
            Some("cuda".to_string())
        } else {
            log::info!("[hardware] Auto-resolved GPU acceleration: directml");
            Some("directml".to_string())
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        if check_gpu_availability().await.unwrap_or(false) {
            log::info!("[hardware] Auto-resolved GPU acceleration: cuda");
            Some("cuda".to_string())
        } else {
            log::info!("[hardware] Auto-resolved GPU acceleration: cpu");
            Some("cpu".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_check_gpu_availability() {
        let result = check_gpu_availability().await;
        // Verify it returns Ok result (Ok(true) or Ok(false))
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
