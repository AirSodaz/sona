use std::process::Command;

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
