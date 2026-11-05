use sona_core::ports::asr::{AsrPortError, AsrPortErrorKind};
use std::path::{Path, PathBuf};

pub(crate) const SUPPORTED_SAMPLE_RATE: u32 = 16_000;

pub(crate) fn reject_unsupported_rate(
    engine_label: &str,
    sample_rate: u32,
) -> Result<(), AsrPortError> {
    if sample_rate == SUPPORTED_SAMPLE_RATE {
        Ok(())
    } else {
        Err(AsrPortError::new(
            AsrPortErrorKind::Unsupported,
            format!(
                "{engine_label} supports {SUPPORTED_SAMPLE_RATE} Hz input, got {sample_rate} Hz."
            ),
        ))
    }
}

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
    let mut onnx_files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|ext| ext == "onnx"))
        .collect();

    onnx_files.sort();
    for canonical_name in ["model.onnx", "silero_vad.onnx", "fsmn_vad.onnx"] {
        if let Some(canonical) = onnx_files
            .iter()
            .find(|p| p.file_name().and_then(|n| n.to_str()) == Some(canonical_name))
        {
            return Ok(canonical.clone());
        }
    }

    onnx_files.into_iter().next().ok_or_else(|| {
        AsrPortError::new(
            AsrPortErrorKind::Model,
            format!(
                "No .onnx file found in VAD model directory {}",
                path.display()
            ),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsupported_sample_rates() {
        assert!(reject_unsupported_rate("test", 16_000).is_ok());
        let err = reject_unsupported_rate("test", 44_100).unwrap_err();
        assert_eq!(err.kind, AsrPortErrorKind::Unsupported);
    }

    #[test]
    fn resolve_model_onnx_path_validates_existence() {
        let err = resolve_model_onnx_path(Path::new("non_existent_file.onnx")).unwrap_err();
        assert_eq!(err.kind, AsrPortErrorKind::Model);
    }
}
