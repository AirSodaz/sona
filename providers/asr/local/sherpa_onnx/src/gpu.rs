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

pub use sona_core::models::config::is_int8_model;

/// Target execution environment parameters for resolving hardware acceleration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformEnv {
    pub is_windows: bool,
    pub is_macos: bool,
    pub cuda_available: bool,
    pub directml_available: bool,
}

impl PlatformEnv {
    pub fn current(cuda_available: bool, directml_available: bool) -> Self {
        let is_windows = cfg!(target_os = "windows");
        let is_macos = cfg!(target_os = "macos");
        Self {
            is_windows,
            is_macos,
            cuda_available: if is_macos { false } else { cuda_available },
            directml_available: if is_windows {
                directml_available
            } else {
                false
            },
        }
    }

    pub const fn new(
        is_windows: bool,
        is_macos: bool,
        cuda_available: bool,
        directml_available: bool,
    ) -> Self {
        Self {
            is_windows,
            is_macos,
            cuda_available,
            directml_available,
        }
    }

    pub const fn macos() -> Self {
        Self {
            is_windows: false,
            is_macos: true,
            cuda_available: false,
            directml_available: false,
        }
    }

    pub const fn windows(cuda_available: bool, directml_available: bool) -> Self {
        Self {
            is_windows: true,
            is_macos: false,
            cuda_available,
            directml_available,
        }
    }

    pub const fn linux(cuda_available: bool) -> Self {
        Self {
            is_windows: false,
            is_macos: false,
            cuda_available,
            directml_available: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuAccelerationPlan {
    providers: Vec<Option<String>>,
    auto_windows_directml_fallback: bool,
}

impl GpuAccelerationPlan {
    pub async fn for_current_platform(gpu_acceleration: Option<&str>, is_int8: bool) -> Self {
        #[cfg(target_os = "macos")]
        let env = PlatformEnv::macos();
        #[cfg(not(target_os = "macos"))]
        let env = {
            let cuda_available = check_gpu_availability().await.unwrap_or(false);
            PlatformEnv::current(cuda_available, directml_runtime_available())
        };
        Self::for_platform(gpu_acceleration, env, is_int8)
    }

    pub fn for_platform(gpu_acceleration: Option<&str>, env: PlatformEnv, is_int8: bool) -> Self {
        let Some(gpu) = gpu_acceleration else {
            return Self {
                providers: vec![None],
                auto_windows_directml_fallback: false,
            };
        };

        // Rule 1: On macOS, sherpa-onnx strictly uses CPU.
        // CoreML is broken/leaks memory on complex models and is deprecated upstream.
        if env.is_macos {
            return Self {
                providers: vec![Some("cpu".to_string())],
                auto_windows_directml_fallback: false,
            };
        }

        let gpu = gpu.trim().to_ascii_lowercase();
        let gpu = gpu.as_str();

        // Rule 2: On non-macOS platforms:
        // Handle explicit acceleration requests
        if gpu != "auto" {
            let provider = match gpu {
                "cuda" => {
                    // CUDA acceleration excludes INT8 models (CPU is optimized for INT8)
                    if is_int8 {
                        "cpu".to_string()
                    } else {
                        "cuda".to_string()
                    }
                }
                "directml" => {
                    if env.is_windows {
                        "directml".to_string()
                    } else {
                        "cpu".to_string()
                    }
                }
                "metal" | "coreml" | "vulkan" => "cpu".to_string(),
                other => other.to_string(),
            };
            return Self {
                providers: vec![Some(provider)],
                auto_windows_directml_fallback: false,
            };
        }

        // "auto" resolution on non-macOS platforms:
        // If CUDA is available and model is not INT8, prioritize CUDA
        if env.cuda_available && !is_int8 {
            return Self {
                providers: vec![Some("cuda".to_string()), Some("cpu".to_string())],
                auto_windows_directml_fallback: false,
            };
        }

        // Windows DirectML fallback for non-INT8 models
        if env.is_windows && env.directml_available && !is_int8 {
            return Self {
                providers: vec![Some("directml".to_string()), Some("cpu".to_string())],
                auto_windows_directml_fallback: true,
            };
        }

        // INT8 models or CPU fallback
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

pub async fn check_gpu_availability() -> Result<bool, sona_core::ports::asr::AsrPortError> {
    #[cfg(target_os = "macos")]
    {
        Ok(std::env::consts::ARCH == "aarch64")
    }
    #[cfg(not(target_os = "macos"))]
    {
        if sona_core::runtime::cuda_addon::is_cuda_addon_active() {
            return Ok(true);
        }
        let is_available = tokio::process::Command::new("nvidia-smi")
            .output()
            .await
            .map(|output| output.status.success())
            .unwrap_or(false);

        Ok(is_available)
    }
}

pub async fn resolve_gpu_acceleration_plan(
    gpu_acceleration: Option<&str>,
    is_int8: bool,
) -> GpuAccelerationPlan {
    GpuAccelerationPlan::for_current_platform(gpu_acceleration, is_int8).await
}

pub fn directml_runtime_available() -> bool {
    cfg!(sona_sherpa_directml)
}

#[cfg(test)]
mod tests {
    use super::{GpuAccelerationPlan, GpuFallbackNotice, PlatformEnv};

    #[test]
    fn windows_auto_gpu_plan_falls_back_to_cpu_after_directml_for_non_int8() {
        let env = PlatformEnv::windows(false, true);
        let plan = GpuAccelerationPlan::for_platform(Some("auto"), env, false);

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

    #[test]
    fn metal_and_vulkan_gpu_plans_resolve_safely_for_platform() {
        let plan_windows_metal = GpuAccelerationPlan::for_platform(
            Some("metal"),
            PlatformEnv::windows(false, false),
            false,
        );
        assert_eq!(
            plan_windows_metal.provider_options(),
            vec![Some("cpu".to_string())]
        );

        let plan_vulkan = GpuAccelerationPlan::for_platform(
            Some("vulkan"),
            PlatformEnv::windows(true, false),
            false,
        );
        assert_eq!(
            plan_vulkan.provider_options(),
            vec![Some("cpu".to_string())]
        );
    }

    #[test]
    fn macos_strictly_uses_cpu_for_all_accelerations() {
        let env = PlatformEnv::macos();

        // Auto on macOS (both non-int8 and int8)
        let plan_auto = GpuAccelerationPlan::for_platform(Some("auto"), env, false);
        assert_eq!(plan_auto.provider_options(), vec![Some("cpu".to_string())]);

        let plan_auto_int8 = GpuAccelerationPlan::for_platform(Some("auto"), env, true);
        assert_eq!(
            plan_auto_int8.provider_options(),
            vec![Some("cpu".to_string())]
        );

        // Metal / CoreML on macOS
        let plan_metal = GpuAccelerationPlan::for_platform(Some("metal"), env, false);
        assert_eq!(plan_metal.provider_options(), vec![Some("cpu".to_string())]);

        let plan_coreml = GpuAccelerationPlan::for_platform(Some("coreml"), env, false);
        assert_eq!(
            plan_coreml.provider_options(),
            vec![Some("cpu".to_string())]
        );

        // CUDA on macOS
        let plan_cuda = GpuAccelerationPlan::for_platform(Some("cuda"), env, false);
        assert_eq!(plan_cuda.provider_options(), vec![Some("cpu".to_string())]);
    }

    #[test]
    fn cuda_acceleration_excludes_int8_models() {
        let env_cuda = PlatformEnv::linux(true);

        // Explicit cuda request with int8 model -> resolves to cpu
        let plan_cuda_int8 = GpuAccelerationPlan::for_platform(Some("cuda"), env_cuda, true);
        assert_eq!(
            plan_cuda_int8.provider_options(),
            vec![Some("cpu".to_string())]
        );

        // Case-insensitivity and whitespace trimming
        let plan_cuda_upper = GpuAccelerationPlan::for_platform(Some(" CUDA "), env_cuda, true);
        assert_eq!(
            plan_cuda_upper.provider_options(),
            vec![Some("cpu".to_string())]
        );

        // Explicit cuda request with non-int8 model -> resolves to cuda
        let plan_cuda_fp32 = GpuAccelerationPlan::for_platform(Some("cuda"), env_cuda, false);
        assert_eq!(
            plan_cuda_fp32.provider_options(),
            vec![Some("cuda".to_string())]
        );

        // Auto request with cuda available and int8 model -> excludes cuda, uses cpu
        let plan_auto_int8 = GpuAccelerationPlan::for_platform(Some("auto"), env_cuda, true);
        assert_eq!(
            plan_auto_int8.provider_options(),
            vec![Some("cpu".to_string())]
        );

        // Auto request with cuda available and non-int8 model -> prioritizes cuda with cpu fallback
        let plan_auto_fp32 = GpuAccelerationPlan::for_platform(Some("auto"), env_cuda, false);
        assert_eq!(
            plan_auto_fp32.provider_options(),
            vec![Some("cuda".to_string()), Some("cpu".to_string())]
        );
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct LocalGpuAvailabilityProvider;

#[async_trait::async_trait]
impl sona_core::ports::runtime::GpuAvailabilityPort for LocalGpuAvailabilityProvider {
    async fn is_gpu_available(&self) -> bool {
        check_gpu_availability().await.unwrap_or(false)
    }
}
