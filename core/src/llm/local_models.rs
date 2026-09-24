use crate::llm::provider_protocol::{LlmModality, LlmModelSummary};
use serde::{Deserialize, Serialize};
#[cfg(feature = "specta")]
use specta::Type;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

const LOCAL_MODELS_JSON: &str = include_str!("local-models.json");

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct LocalLlmDownload {
    pub url: String,
    pub mirror_url: Option<String>,
    pub sha256: String,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct LocalLlmPreset {
    pub id: String,
    pub name: String,
    pub model: String,
    pub filename: String,
    pub description: String,
    pub backend: String,
    pub context_window: u64,
    pub max_output_tokens: u64,
    pub size: String,
    pub parameters: Option<String>,
    pub quantization: Option<String>,
    #[serde(default)]
    pub modalities: Vec<String>,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    pub is_recommended: bool,
    pub download: Option<LocalLlmDownload>,
}

impl LocalLlmPreset {
    pub fn to_model_summary(&self) -> LlmModelSummary {
        let supports_reasoning = self.capabilities.iter().any(|c| c == "reasoning");
        LlmModelSummary {
            model: self.model.clone(),
            display_name: Some(self.name.clone()),
            context_window: Some(self.context_window),
            max_output_tokens: Some(self.max_output_tokens),
            input_modalities: vec![LlmModality::Text],
            output_modalities: vec![LlmModality::Text],
            supports_reasoning: Some(supports_reasoning),
            reasoning_mode: if supports_reasoning {
                Some(crate::llm::runtime::ReasoningMode::Effort {
                    supported_levels: vec![
                        crate::llm::runtime::ThinkingLevel::Minimal,
                        crate::llm::runtime::ThinkingLevel::Low,
                        crate::llm::runtime::ThinkingLevel::Medium,
                        crate::llm::runtime::ThinkingLevel::High,
                        crate::llm::runtime::ThinkingLevel::Xhigh,
                        crate::llm::runtime::ThinkingLevel::Max,
                    ],
                })
            } else {
                Some(crate::llm::runtime::ReasoningMode::None)
            },
            supported_thinking_levels: if supports_reasoning {
                vec![
                    crate::llm::runtime::ThinkingLevel::Minimal,
                    crate::llm::runtime::ThinkingLevel::Low,
                    crate::llm::runtime::ThinkingLevel::Medium,
                    crate::llm::runtime::ThinkingLevel::High,
                    crate::llm::runtime::ThinkingLevel::Xhigh,
                    crate::llm::runtime::ThinkingLevel::Max,
                ]
            } else {
                Vec::new()
            },
            supports_temperature: Some(true),
            ..Default::default()
        }
    }

    pub fn to_preset_model(&self) -> crate::models::preset_models::PresetModel {
        use crate::models::preset_models::{LanguageMode, PresetModel, PresetModelArtifact};

        PresetModel {
            id: self.id.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            model_type: "llm".to_string(),
            modes: Some(vec!["llm".to_string()]),
            languages: self.languages.clone(),
            language_mode: LanguageMode::None,
            size: self.size.clone(),
            artifacts: match &self.download {
                Some(dl) => vec![PresetModelArtifact {
                    url: dl.url.clone(),
                    filename: self.filename.clone(),
                    sha256: Some(dl.sha256.clone()),
                    size_bytes: dl.size_bytes,
                }],
                None => vec![],
            },
            is_recommended: Some(self.is_recommended),
            is_archive: Some(false),
            filename: Some(self.filename.clone()),
            engine: Some(self.backend.clone()),
            rules: None,
            file_config: None,
            group_id: Some(self.id.clone()),
            version_label: self.quantization.clone(),
        }
    }

    pub fn to_model_card(
        &self,
        is_installed: bool,
        installed_path: Option<String>,
        installed_size_bytes: Option<u64>,
    ) -> LocalLlmModelCard {
        LocalLlmModelCard {
            id: self.id.clone(),
            name: self.name.clone(),
            model: self.model.clone(),
            filename: self.filename.clone(),
            description: self.description.clone(),
            backend: self.backend.clone(),
            context_window: self.context_window,
            max_output_tokens: self.max_output_tokens,
            size: self.size.clone(),
            parameters: self.parameters.clone(),
            quantization: self.quantization.clone(),
            modalities: if self.modalities.is_empty() {
                vec!["text".to_string()]
            } else {
                self.modalities.clone()
            },
            languages: self.languages.clone(),
            capabilities: self.capabilities.clone(),
            is_recommended: self.is_recommended,
            is_installed,
            installed_path,
            installed_size_bytes,
            download_url: self.download.as_ref().map(|d| d.url.clone()),
            download_size_bytes: self.download.as_ref().and_then(|d| d.size_bytes),
        }
    }
    /// Checks whether the preset model file exists in the specified models directory.
    pub fn is_installed_in_dir(&self, dir: &Path) -> bool {
        self.find_installed_path(dir).is_some()
    }

    /// Finds the installed path of the preset model in the specified directory.
    pub fn find_installed_path(&self, dir: &Path) -> Option<PathBuf> {
        let candidate_paths = [
            dir.join(&self.filename),
            dir.join(&self.id).join(&self.filename),
        ];
        candidate_paths.into_iter().find(|path| path.is_file())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct LocalLlmModelCard {
    pub id: String,
    pub name: String,
    pub model: String,
    pub filename: String,
    pub description: String,
    pub backend: String,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub context_window: u64,
    #[cfg_attr(feature = "specta", specta(type = specta_typescript::Number))]
    pub max_output_tokens: u64,
    pub size: String,
    pub parameters: Option<String>,
    pub quantization: Option<String>,
    #[serde(default)]
    pub modalities: Vec<String>,
    pub languages: Vec<String>,
    pub capabilities: Vec<String>,
    pub is_recommended: bool,
    pub is_installed: bool,
    pub installed_path: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub installed_size_bytes: Option<u64>,
    pub download_url: Option<String>,
    #[cfg_attr(feature = "specta", specta(type = Option<specta_typescript::Number>))]
    pub download_size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct LocalLlmCardsResponse {
    pub models_dir: String,
    pub cards: Vec<LocalLlmModelCard>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LocalModelsManifest {
    #[allow(dead_code)]
    schema_version: u32,
    models: Vec<LocalLlmPreset>,
}

static LOCAL_MODELS: LazyLock<Vec<LocalLlmPreset>> = LazyLock::new(|| {
    let manifest: LocalModelsManifest =
        serde_json::from_str(LOCAL_MODELS_JSON).expect("local LLM models JSON must be valid");
    manifest.models
});

pub fn local_llm_models() -> &'static [LocalLlmPreset] {
    LOCAL_MODELS.as_slice()
}

pub fn find_local_llm_model(id_or_name: &str) -> Option<&'static LocalLlmPreset> {
    local_llm_models().iter().find(|m| {
        m.id.eq_ignore_ascii_case(id_or_name)
            || m.model.eq_ignore_ascii_case(id_or_name)
            || m.name.eq_ignore_ascii_case(id_or_name)
            || m.filename.eq_ignore_ascii_case(id_or_name)
    })
}

static PRESET_NON_LLM_FILENAMES: LazyLock<HashSet<String>> = LazyLock::new(|| {
    let mut names = HashSet::new();
    for preset in crate::models::preset_models::preset_models() {
        names.insert(preset.id.to_lowercase());
        if let Some(filename) = &preset.filename {
            names.insert(filename.to_lowercase());
            if let Some(stem) = Path::new(filename).file_stem().and_then(|s| s.to_str()) {
                names.insert(stem.to_lowercase());
            }
        }
        for artifact in &preset.artifacts {
            names.insert(artifact.filename.to_lowercase());
            if let Some(stem) = Path::new(&artifact.filename)
                .file_stem()
                .and_then(|s| s.to_str())
            {
                names.insert(stem.to_lowercase());
            }
        }
        if let Some(fc) = &preset.file_config {
            if let Some(model) = &fc.model {
                names.insert(model.to_lowercase());
                if let Some(stem) = Path::new(model).file_stem().and_then(|s| s.to_str()) {
                    names.insert(stem.to_lowercase());
                }
            }
            if let Some(mmproj) = &fc.mmproj {
                names.insert(mmproj.to_lowercase());
                if let Some(stem) = Path::new(mmproj).file_stem().and_then(|s| s.to_str()) {
                    names.insert(stem.to_lowercase());
                }
            }
        }
    }
    names
});

/// Returns true if the file or model identifier belongs to an ASR, companion,
/// or other non-LLM model (e.g. speech recognition, multimodal projector, VAD, diarization).
pub fn is_non_llm_model_file(name_or_path: &str) -> bool {
    let raw_name = Path::new(name_or_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(name_or_path);
    let lower_name = raw_name.to_lowercase();
    let stem = Path::new(raw_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(raw_name)
        .to_lowercase();

    // 1. Matches any known preset artifact/file from preset-models.json (ASR, VAD, etc.)
    if PRESET_NON_LLM_FILENAMES.contains(&lower_name) || PRESET_NON_LLM_FILENAMES.contains(&stem) {
        return true;
    }

    // 2. Multimodal projector files (mmproj-*) used as companions for ASR or vision
    if lower_name.starts_with("mmproj") || stem.starts_with("mmproj") {
        return true;
    }

    // 3. Clear speech pipeline / non-LLM keywords
    let speech_indicators = [
        "asr",
        "whisper",
        "sensevoice",
        "sense-voice",
        "paraformer",
        "silero",
        "punct",
        "diariz",
    ];

    for indicator in speech_indicators {
        if lower_name.contains(indicator) || stem.contains(indicator) {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_models_manifest_loads_successfully() {
        let models = local_llm_models();
        assert!(!models.is_empty());
        let qwen = find_local_llm_model("qwen3.5-4b");
        assert!(qwen.is_some());
        let qwen = qwen.unwrap();
        assert_eq!(qwen.model, "Qwen/Qwen3.5-4B");
        assert_eq!(qwen.filename, "Qwen3.5-4B-Q4_K_M.gguf");
        assert_eq!(qwen.backend, "llama.cpp");
        assert!(qwen.download.is_some());
    }

    #[test]
    fn find_by_model_name() {
        let found = find_local_llm_model("Qwen/Qwen3.5-4B");
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, "qwen3.5-4b");
    }

    #[test]
    fn converts_to_preset_model() {
        let qwen = find_local_llm_model("qwen3.5-4b").unwrap();
        let preset = qwen.to_preset_model();
        assert_eq!(preset.id, "qwen3.5-4b");
        assert_eq!(preset.model_type, "llm");
        assert_eq!(preset.filename, Some("Qwen3.5-4B-Q4_K_M.gguf".to_string()));
        assert_eq!(preset.artifacts.len(), 1);
        assert_eq!(preset.artifacts[0].filename, "Qwen3.5-4B-Q4_K_M.gguf");
        assert_eq!(preset.version_label, Some("Q4_K_M".to_string()));
    }

    #[test]
    fn identifies_non_llm_model_files_correctly() {
        // Preset ASR GGUF files
        assert!(is_non_llm_model_file("Qwen3-ASR-0.6B-Q8_0.gguf"));
        assert!(is_non_llm_model_file("mmproj-Qwen3-ASR-0.6B-Q8_0.gguf"));
        assert!(is_non_llm_model_file("Qwen3-ASR-1.7B-Q8_0.gguf"));
        assert!(is_non_llm_model_file("mmproj-Qwen3-ASR-1.7B-Q8_0.gguf"));
        assert!(is_non_llm_model_file("qwen3-asr-0.6b-q8-gguf"));

        // General ASR / speech indicators
        assert!(is_non_llm_model_file("custom-asr-model.gguf"));
        assert!(is_non_llm_model_file("whisper-large-v3.gguf"));
        assert!(is_non_llm_model_file("mmproj-custom.gguf"));
        assert!(is_non_llm_model_file("silero-vad.onnx"));

        // Real LLMs must NOT be flagged as non-LLM
        assert!(!is_non_llm_model_file("Qwen3.5-4B-Q4_K_M.gguf"));
        assert!(!is_non_llm_model_file("Qwen/Qwen3.5-4B"));
        assert!(!is_non_llm_model_file("gemma-4-E2B-it-Q4_K_M.gguf"));
        assert!(!is_non_llm_model_file("google/gemma-4-e2b"));
        assert!(!is_non_llm_model_file("Llama-3.2-3B-Instruct.gguf"));
        assert!(!is_non_llm_model_file("DeepSeek-R1-Distill-Qwen-7B.gguf"));
    }

    #[test]
    fn checks_preset_installation_in_directory() {
        let temp_dir =
            std::env::temp_dir().join(format!("sona_test_models_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let qwen = find_local_llm_model("qwen3.5-4b").unwrap();
        assert!(!qwen.is_installed_in_dir(&temp_dir));
        assert_eq!(qwen.find_installed_path(&temp_dir), None);

        let target_file = temp_dir.join(&qwen.filename);
        std::fs::write(&target_file, b"test").unwrap();

        assert!(qwen.is_installed_in_dir(&temp_dir));
        assert_eq!(qwen.find_installed_path(&temp_dir), Some(target_file));

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn converts_to_model_summary_with_reasoning() {
        let qwen = find_local_llm_model("qwen3.5-4b").unwrap();
        let summary = qwen.to_model_summary();
        assert_eq!(summary.supports_reasoning, Some(true));
        assert!(matches!(
            summary.reasoning_mode,
            Some(crate::llm::runtime::ReasoningMode::Effort { .. })
        ));
        assert_eq!(summary.supported_thinking_levels.len(), 6);
        assert_eq!(summary.supports_temperature, Some(true));
    }
}
