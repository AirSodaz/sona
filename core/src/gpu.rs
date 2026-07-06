pub const DEFAULT_GPU_ACCELERATION: &str = "auto";
pub const GPU_ACCELERATION_VALUES: &[&str] = &["auto", "cpu", "cuda", "coreml", "directml"];

pub fn resolve_gpu_acceleration(value: Option<String>) -> Result<Option<String>, String> {
    let value = value.unwrap_or_else(|| DEFAULT_GPU_ACCELERATION.to_string());
    let normalized = value.trim().to_ascii_lowercase();

    if GPU_ACCELERATION_VALUES.contains(&normalized.as_str()) {
        Ok(Some(normalized))
    } else {
        Err(format!(
            "gpu_acceleration must be one of {}.",
            GPU_ACCELERATION_VALUES.join(", ")
        ))
    }
}
