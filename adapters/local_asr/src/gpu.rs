#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuFallbackNotice {
    pub from_provider: String,
    pub to_provider: String,
    pub error: String,
}

impl GpuFallbackNotice {
    pub fn directml_retry(error: impl Into<String>) -> Self {
        Self {
            from_provider: "directml".to_string(),
            to_provider: "cpu".to_string(),
            error: error.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuAccelerationPlan {
    providers: Vec<Option<String>>,
    auto_windows_directml_fallback: bool,
}

impl GpuAccelerationPlan {
    pub async fn for_current_platform(gpu_acceleration: Option<&str>) -> Self {
        let cuda_available = check_gpu_availability().await.unwrap_or(false);
        Self::for_platform(
            gpu_acceleration,
            cfg!(target_os = "windows"),
            cuda_available,
            directml_runtime_available(),
        )
    }

    pub fn for_platform(
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

    pub fn provider_options(&self) -> Vec<Option<String>> {
        self.providers.clone()
    }

    pub fn should_retry_after_failure(&self, provider: &str) -> bool {
        self.auto_windows_directml_fallback && provider == "directml"
    }
}

pub async fn check_gpu_availability() -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        Ok(std::env::consts::ARCH == "aarch64")
    }

    #[cfg(not(target_os = "macos"))]
    {
        let is_available = tokio::process::Command::new("nvidia-smi")
            .output()
            .await
            .map(|output| output.status.success())
            .unwrap_or(false);

        Ok(is_available)
    }
}

pub async fn resolve_gpu_acceleration_plan(gpu_acceleration: Option<&str>) -> GpuAccelerationPlan {
    GpuAccelerationPlan::for_current_platform(gpu_acceleration).await
}

pub fn directml_runtime_available() -> bool {
    cfg!(sona_sherpa_directml)
}

#[cfg(test)]
mod tests {
    use super::{GpuAccelerationPlan, GpuFallbackNotice};

    #[test]
    fn windows_auto_gpu_plan_falls_back_to_cpu_after_directml() {
        let plan = GpuAccelerationPlan::for_platform(Some("auto"), true, false, true);

        assert_eq!(
            plan.provider_options(),
            vec![Some("directml".to_string()), Some("cpu".to_string())]
        );
    }

    #[test]
    fn directml_fallback_notice_records_error() {
        let notice = GpuFallbackNotice::directml_retry("init failed");
        assert_eq!(notice.from_provider, "directml");
        assert_eq!(notice.to_provider, "cpu");
        assert_eq!(notice.error, "init failed");
    }
}
