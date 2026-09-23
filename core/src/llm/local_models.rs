use std::sync::LazyLock;
use serde::{Deserialize, Serialize};
#[cfg(feature = "specta")]
use specta::Type;

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
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LocalModelsManifest {
    #[allow(dead_code)]
    schema_version: u32,
    models: Vec<LocalLlmPreset>,
}

static LOCAL_MODELS: LazyLock<Vec<LocalLlmPreset>> = LazyLock::new(|| {
    let manifest: LocalModelsManifest = serde_json::from_str(LOCAL_MODELS_JSON)
        .expect("local LLM models JSON must be valid");
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
}
