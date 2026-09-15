use serde::{Deserialize, Serialize};
#[cfg(feature = "specta")]
use specta::Type;

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct ModelFileConfig {
    pub encoder: Option<String>,
    pub decoder: Option<String>,
    pub model: Option<String>,
    pub joiner: Option<String>,
    pub tokens: Option<String>,
    pub conv_frontend: Option<String>,
    pub encoder_adaptor: Option<String>,
    pub llm: Option<String>,
    pub embedding: Option<String>,
    pub tokenizer: Option<String>,
    pub mmproj: Option<String>,
    pub preprocessor: Option<String>,
    pub uncached_decoder: Option<String>,
    pub cached_decoder: Option<String>,
    pub merged_decoder: Option<String>,
}

impl ModelFileConfig {
    /// Returns an iterator over all configured file names.
    pub fn file_names(&self) -> impl Iterator<Item = &str> {
        [
            self.encoder.as_deref(),
            self.decoder.as_deref(),
            self.model.as_deref(),
            self.joiner.as_deref(),
            self.tokens.as_deref(),
            self.conv_frontend.as_deref(),
            self.encoder_adaptor.as_deref(),
            self.llm.as_deref(),
            self.embedding.as_deref(),
            self.tokenizer.as_deref(),
            self.mmproj.as_deref(),
            self.preprocessor.as_deref(),
            self.uncached_decoder.as_deref(),
            self.cached_decoder.as_deref(),
            self.merged_decoder.as_deref(),
        ]
        .into_iter()
        .flatten()
    }

    /// Checks whether any configured model file indicates INT8 quantization.
    pub fn is_int8_quantized(&self) -> bool {
        self.file_names()
            .any(|name| name.to_ascii_lowercase().contains("int8"))
    }
}

/// Helper function to detect whether an installed model is an INT8 quantized model.
/// Checks the model directory/file path and any file paths specified in ModelFileConfig.
pub fn is_int8_model(model_path: &std::path::Path, file_config: Option<&ModelFileConfig>) -> bool {
    let has_int8_name = model_path
        .file_name()
        .is_some_and(|name| name.to_string_lossy().to_ascii_lowercase().contains("int8"));

    if has_int8_name {
        return true;
    }

    file_config.is_some_and(|fc| fc.is_int8_quantized())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_is_int8_detection() {
        let fc_int8 = ModelFileConfig {
            encoder: Some("encoder.int8.onnx".to_string()),
            ..Default::default()
        };
        assert!(fc_int8.is_int8_quantized());
        assert!(is_int8_model(Path::new("models/my-model"), Some(&fc_int8)));

        let fc_fp32 = ModelFileConfig {
            model: Some("model.onnx".to_string()),
            tokens: Some("tokens.txt".to_string()),
            ..Default::default()
        };
        assert!(!fc_fp32.is_int8_quantized());
        assert!(!is_int8_model(Path::new("models/my-model"), Some(&fc_fp32)));

        // Parent directory containing "int8" does not cause false positives
        assert!(!is_int8_model(
            Path::new("/workspace/sprint8/models/sensevoice-fp32"),
            Some(&fc_fp32)
        ));

        // Model path contains int8 in directory/file name
        assert!(is_int8_model(Path::new("models/sensevoice-int8"), None));
    }
}
