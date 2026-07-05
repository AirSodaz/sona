#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuAccelerationPlan {
    providers: Vec<Option<String>>,
    auto_windows_directml_fallback: bool,
}

impl GpuAccelerationPlan {
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

#[cfg(test)]
mod tests {
    use super::GpuAccelerationPlan;

    #[test]
    fn windows_auto_gpu_plan_falls_back_to_cpu_after_directml() {
        let plan = GpuAccelerationPlan::for_platform(Some("auto"), true, false, true);

        assert_eq!(
            plan.provider_options(),
            vec![Some("directml".to_string()), Some("cpu".to_string())]
        );
    }
}
