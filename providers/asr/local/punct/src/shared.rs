use sona_core::ports::asr::{AsrPortError, AsrPortErrorKind};
use std::path::{Path, PathBuf};

pub(crate) const NUM_THREADS: i32 = 1;

pub fn resolve_model_onnx_path(path: &Path) -> Result<PathBuf, AsrPortError> {
    if !path.exists() {
        return Err(AsrPortError::new(
            AsrPortErrorKind::Model,
            format!("Model path does not exist: {}", path.display()),
        ));
    }

    if path.is_file() {
        return Ok(path.to_path_buf());
    }

    let entries = std::fs::read_dir(path).map_err(|error| {
        AsrPortError::new(
            AsrPortErrorKind::FileSystem,
            format!("Failed to read model directory {}: {error}", path.display()),
        )
    })?;
    entries
        .flatten()
        .find(|entry| entry.path().extension().is_some_and(|ext| ext == "onnx"))
        .map(|entry| entry.path())
        .ok_or_else(|| {
            AsrPortError::new(
                AsrPortErrorKind::Model,
                format!(
                    "No .onnx file found in punctuation model directory {}",
                    path.display()
                ),
            )
        })
}
