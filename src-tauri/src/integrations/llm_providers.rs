use serde::Deserialize;
use std::sync::OnceLock;

const LLM_PROVIDERS_JSON: &str = include_str!("../../../src/shared/llm-providers.json");

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LlmProviderManifest {
    schema_version: u32,
    providers: Vec<LlmProvider>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmProvider {
    pub id: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub defaults: LlmProviderDefaults,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmProviderDefaults {
    #[serde(default)]
    pub api_host: String,
    pub api_path: Option<String>,
    pub api_version: Option<String>,
}

static LLM_PROVIDER_MANIFEST: OnceLock<LlmProviderManifest> = OnceLock::new();

fn manifest() -> &'static LlmProviderManifest {
    LLM_PROVIDER_MANIFEST.get_or_init(|| {
        let manifest: LlmProviderManifest = serde_json::from_str(LLM_PROVIDERS_JSON)
            .expect("shared LLM providers JSON should be valid");
        assert_eq!(
            manifest.schema_version, 1,
            "shared LLM providers schema version should be supported"
        );
        manifest
    })
}

pub fn llm_providers() -> &'static [LlmProvider] {
    manifest().providers.as_slice()
}

pub fn find_llm_provider_by_id_or_alias(id_or_alias: &str) -> Option<&'static LlmProvider> {
    llm_providers().iter().find(|provider| {
        provider.id == id_or_alias || provider.aliases.iter().any(|alias| alias == id_or_alias)
    })
}
