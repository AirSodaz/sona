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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GpuFallbackNotice {
    pub(crate) from_provider: String,
    pub(crate) to_provider: String,
    pub(crate) error: String,
}

impl GpuFallbackNotice {
    pub(crate) fn directml_retry(error: impl Into<String>) -> Self {
        Self {
            from_provider: "directml".to_string(),
            to_provider: "cpu".to_string(),
            error: error.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GpuAccelerationPlan {
    providers: Vec<Option<String>>,
    auto_windows_directml_fallback: bool,
}

impl GpuAccelerationPlan {
    pub(crate) async fn for_current_platform(gpu_acceleration: Option<&str>) -> Self {
        let cuda_available = check_gpu_availability().await.unwrap_or(false);
        Self::for_platform(
            gpu_acceleration,
            cfg!(target_os = "windows"),
            cuda_available,
            directml_runtime_available(),
        )
    }

    pub(crate) fn for_platform(
        gpu_acceleration: Option<&str>,
        is_windows: bool,
        cuda_available: bool,
        directml_available: bool,
    ) -> Self {
        let Some(gpu) = gpu_acceleration else {
            return Self {
                providers: vec![None],
                auto_windows_directml_fallback: false,
            };
        };

        if gpu != "auto" {
            return Self {
                providers: vec![Some(gpu.to_string())],
                auto_windows_directml_fallback: false,
            };
        }

        #[cfg(target_os = "macos")]
        if !is_windows {
            return Self {
                providers: vec![Some(
                    if std::env::consts::ARCH == "aarch64" {
                        "coreml"
                    } else {
                        "cpu"
                    }
                    .to_string(),
                )],
                auto_windows_directml_fallback: false,
            };
        }

        if is_windows {
            if cuda_available {
                return Self {
                    providers: vec![Some("cuda".to_string())],
                    auto_windows_directml_fallback: false,
                };
            }

            if directml_available {
                return Self {
                    providers: vec![Some("directml".to_string()), Some("cpu".to_string())],
                    auto_windows_directml_fallback: true,
                };
            }
        } else if cuda_available {
            return Self {
                providers: vec![Some("cuda".to_string())],
                auto_windows_directml_fallback: false,
            };
        }

        Self {
            providers: vec![Some("cpu".to_string())],
            auto_windows_directml_fallback: false,
        }
    }

    pub(crate) fn provider_options(&self) -> Vec<Option<String>> {
        self.providers.clone()
    }

    pub(crate) fn primary_provider(&self) -> Option<String> {
        self.providers.first().cloned().flatten()
    }

    pub(crate) fn should_retry_after_failure(&self, provider: &str) -> bool {
        self.auto_windows_directml_fallback && provider == "directml"
    }
}

pub(crate) async fn resolve_gpu_acceleration_plan(
    gpu_acceleration: Option<&str>,
) -> GpuAccelerationPlan {
    GpuAccelerationPlan::for_current_platform(gpu_acceleration).await
}

pub async fn resolve_gpu_acceleration(gpu_acceleration: Option<&str>) -> Option<String> {
    let resolved = resolve_gpu_acceleration_plan(gpu_acceleration)
        .await
        .primary_provider();
    log::info!("[hardware] Resolved GPU acceleration: {resolved:?}");
    resolved
}

pub(crate) fn directml_runtime_available() -> bool {
    cfg!(sona_sherpa_directml)
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

    #[test]
    fn windows_auto_gpu_plan_prefers_cuda_then_directml_then_cpu() {
        let plan = GpuAccelerationPlan::for_platform(Some("auto"), true, true, false);
        assert_eq!(plan.provider_options(), vec![Some("cuda".to_string())]);

        let plan = GpuAccelerationPlan::for_platform(Some("auto"), true, false, true);
        assert_eq!(
            plan.provider_options(),
            vec![Some("directml".to_string()), Some("cpu".to_string())]
        );
    }

    #[test]
    fn windows_auto_gpu_plan_skips_unavailable_directml_runtime() {
        let plan = GpuAccelerationPlan::for_platform(Some("auto"), true, false, false);

        assert_eq!(plan.provider_options(), vec![Some("cpu".to_string())]);
        assert!(!plan.should_retry_after_failure("cpu"));
    }

    #[test]
    fn explicit_directml_gpu_plan_does_not_retry_cpu() {
        let plan = GpuAccelerationPlan::for_platform(Some("directml"), true, false, false);

        assert_eq!(plan.provider_options(), vec![Some("directml".to_string())]);
        assert!(!plan.should_retry_after_failure("directml"));
    }

    #[test]
    fn fallback_notice_records_directml_error() {
        let notice = GpuFallbackNotice::directml_retry("init failed");

        assert_eq!(notice.from_provider, "directml");
        assert_eq!(notice.to_provider, "cpu");
        assert_eq!(notice.error, "init failed");
    }
}
