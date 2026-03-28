use crate::sherpa::ModelFileConfig;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const PRESET_MODELS_JSON: &str = include_str!("../../src/shared/preset-models.json");

/// Default model rules used when a preset omits explicit requirements.
pub const DEFAULT_MODEL_RULES: ModelRules = ModelRules {
    requires_vad: true,
    requires_punctuation: false,
};

/// Companion-model requirements for a preset model.
#[derive(Debug, Clone, Copy, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelRules {
    pub requires_vad: bool,
    pub requires_punctuation: bool,
}

/// Shared preset metadata consumed by both the GUI and the CLI.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetModel {
    pub id: String,
    pub name: String,
    pub description: String,
    pub url: String,
    #[serde(rename = "type")]
    pub model_type: String,
    pub modes: Option<Vec<String>>,
    pub language: String,
    pub size: String,
    pub is_archive: Option<bool>,
    pub filename: Option<String>,
    pub rules: Option<ModelRules>,
    pub file_config: Option<ModelFileConfig>,
}

static PRESET_MODELS: OnceLock<Vec<PresetModel>> = OnceLock::new();

/// Returns the parsed shared preset metadata.
pub fn preset_models() -> &'static [PresetModel] {
    PRESET_MODELS
        .get_or_init(|| {
            serde_json::from_str(PRESET_MODELS_JSON)
                .expect("shared preset models JSON should be valid")
        })
        .as_slice()
}

/// Finds a shared preset model by its stable identifier.
pub fn find_preset_model(model_id: &str) -> Option<&'static PresetModel> {
    preset_models().iter().find(|model| model.id == model_id)
}

impl PresetModel {
    /// Resolves the installed model path under the given models directory.
    pub fn resolve_install_path(&self, models_dir: &Path) -> PathBuf {
        match &self.filename {
            Some(filename) => models_dir.join(filename),
            None => models_dir.join(&self.id),
        }
    }

    /// Returns the expected download filename under the given models directory.
    pub fn resolve_download_path(&self, models_dir: &Path) -> PathBuf {
        if self.is_archive() {
            models_dir.join(format!("{}.tar.bz2", self.id))
        } else {
            self.resolve_install_path(models_dir)
        }
    }

    /// Returns true when the preset ships as an archive.
    pub fn is_archive(&self) -> bool {
        self.is_archive.unwrap_or(true)
    }

    /// Returns the effective rules for this preset model.
    pub fn resolved_rules(&self) -> ModelRules {
        self.rules.unwrap_or(DEFAULT_MODEL_RULES)
    }

    /// Returns true when the preset supports the requested mode.
    pub fn supports_mode(&self, mode: &str) -> bool {
        self.modes
            .as_ref()
            .map(|modes| modes.iter().any(|item| item == mode))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_shared_preset_models() {
        assert!(!preset_models().is_empty());
        assert!(find_preset_model("silero-vad").is_some());
    }

    #[test]
    fn resolves_directory_models_by_id() {
        let model = find_preset_model("sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25").unwrap();
        let path = model.resolve_install_path(Path::new("C:/models"));
        assert_eq!(
            path,
            PathBuf::from("C:/models/sherpa-onnx-qwen3-asr-0.6B-int8-2026-03-25")
        );
    }

    #[test]
    fn resolves_file_models_by_filename() {
        let model = find_preset_model("silero-vad").unwrap();
        let path = model.resolve_install_path(Path::new("C:/models"));
        assert_eq!(path, PathBuf::from("C:/models/silero_vad.onnx"));
    }

    #[test]
    fn resolves_archive_download_path_by_id() {
        let model = find_preset_model("sherpa-onnx-whisper-turbo").unwrap();
        let path = model.resolve_download_path(Path::new("C:/models"));
        assert_eq!(
            path,
            PathBuf::from("C:/models/sherpa-onnx-whisper-turbo.tar.bz2")
        );
    }

    #[test]
    fn resolves_file_download_path_by_filename() {
        let model = find_preset_model("silero-vad").unwrap();
        let path = model.resolve_download_path(Path::new("C:/models"));
        assert_eq!(path, PathBuf::from("C:/models/silero_vad.onnx"));
    }

    #[test]
    fn falls_back_to_default_rules() {
        let model = find_preset_model("silero-vad").unwrap();
        assert_eq!(model.resolved_rules(), DEFAULT_MODEL_RULES);
    }
}
