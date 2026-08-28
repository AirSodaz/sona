pub const DEFAULT_GPU_ACCELERATION: &str = "auto";
pub const GPU_ACCELERATION_VALUES: &[&str] = &["auto", "cpu", "vulkan", "metal", "cuda"];

pub fn resolve_gpu_acceleration(
    value: Option<String>,
) -> Result<Option<String>, RuntimeValidationError> {
    let value = value.unwrap_or_else(|| DEFAULT_GPU_ACCELERATION.to_string());
    let normalized = value.trim().to_ascii_lowercase();

    let mapped = match normalized.as_str() {
        "coreml" => "metal",
        "directml" => "auto",
        other => other,
    };

    if GPU_ACCELERATION_VALUES.contains(&mapped) {
        Ok(Some(mapped.to_string()))
    } else {
        Err(RuntimeValidationError::new(
            "gpu_acceleration",
            format!(
                "gpu_acceleration must be one of {}.",
                GPU_ACCELERATION_VALUES.join(", ")
            ),
        ))
    }
}
use super::error::RuntimeValidationError;
