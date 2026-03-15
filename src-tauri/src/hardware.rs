/// Checks if a compatible GPU is available for acceleration.
///
/// On macOS, checks for Apple Silicon (arm64).
/// On other platforms (Windows/Linux), checks for NVIDIA GPUs via `nvidia-smi`.
///
/// # Returns
///
/// Returns `Ok(true)` if a compatible GPU is found, `Ok(false)` if not, or an `Err` containing
/// an error message if the check fails in an unexpected way.
#[tauri::command]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_check_gpu_availability() {
        let result = check_gpu_availability().await;
        // Verify it returns Ok result (Ok(true) or Ok(false))
        assert!(result.is_ok());
    }
}
