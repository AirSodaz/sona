use serde::{Deserialize, Serialize};
#[cfg(feature = "specta")]
use specta::Type;
use std::sync::LazyLock;

use crate::llm::provider_protocol::{LlmModality, LlmModelSummary};

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
    pub languages: Vec<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    pub is_recommended: bool,
    pub download: Option<LocalLlmDownload>,
}

impl LocalLlmPreset {
    pub fn to_model_summary(&self) -> LlmModelSummary {
        LlmModelSummary {
            model: self.model.clone(),
            display_name: Some(self.name.clone()),
            context_window: Some(self.context_window),
            max_output_tokens: Some(self.max_output_tokens),
            input_modalities: vec![LlmModality::Text],
            output_modalities: vec![LlmModality::Text],
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
}
