/// Checks if a compatible GPU is available for acceleration.
///
/// On macOS, it checks for Apple Silicon (arm64).
/// On other platforms (Windows/Linux), it checks for NVIDIA GPUs via `nvidia-smi`.
///
/// # Returns
///
/// * `Ok(true)` if a compatible GPU is found.
/// * `Ok(false)` if no compatible GPU is found.
/// * `Err(String)` if an unexpected error occurs (though currently it returns `Ok(false)` on check failure).
#[tauri::command]
pub fn check_gpu_availability() -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        use std::env;
        // Check for Apple Silicon (arm64)
        if env::consts::ARCH == "aarch64" {
            return Ok(true);
        }
        return Ok(false);
    }

    #[cfg(not(target_os = "macos"))]
    {
        use std::process::Command;
        // Check for NVIDIA GPU via nvidia-smi
        // Using "which" or "where" first might be safer but calling it directly works if in PATH
        match Command::new("nvidia-smi").output() {
            Ok(output) => {
                if output.status.success() {
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            Err(_) => Ok(false),
        }
    }
}
