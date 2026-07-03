use serde::Deserialize;
use serde_json::Value;
use std::sync::OnceLock;

const ONLINE_ASR_PROVIDERS_JSON: &str =
    include_str!("../../../src/shared/online-asr-providers.json");

pub const VOLCENGINE_DOUBAO_PROVIDER_ID: &str = "volcengine-doubao";
pub const VOLCENGINE_DOUBAO_LEGACY_PROVIDER_KEY: &str = "volcengineDoubao";

pub const GROQ_WHISPER_PROVIDER_ID: &str = "groq-whisper";
pub const MISTRAL_VOXTRAL_PROVIDER_ID: &str = "mistral-voxtral";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OnlineAsrProviderManifest {
    schema_version: u32,
    providers: Vec<OnlineAsrProvider>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineAsrProvider {
    pub id: String,
    pub profile_id: String,
    pub defaults: Value,
    pub streaming: OnlineAsrCapability,
    pub batch: OnlineAsrBatchCapability,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineAsrCapability {
    pub supported: Option<bool>,
    pub requires_api_key: bool,
    pub required_config_fields: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineAsrBatchCapability {
    pub requires_api_key: bool,
    pub required_config_fields: Vec<String>,
    pub local_file_mode: OnlineAsrLocalFileBatchMode,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineAsrLocalFileBatchMode {
    pub supported: bool,
    pub endpoint: String,
    pub resource_id: String,
    pub unsupported_message: String,
}

static ONLINE_ASR_PROVIDER_MANIFEST: OnceLock<OnlineAsrProviderManifest> = OnceLock::new();

fn manifest() -> &'static OnlineAsrProviderManifest {
    ONLINE_ASR_PROVIDER_MANIFEST.get_or_init(|| {
        let manifest: OnlineAsrProviderManifest = serde_json::from_str(ONLINE_ASR_PROVIDERS_JSON)
            .expect("shared online ASR providers JSON should be valid");
        assert_eq!(
            manifest.schema_version, 1,
            "shared online ASR providers schema version should be supported"
        );
        assert!(
            manifest
                .providers
                .iter()
                .any(|provider| provider.id == VOLCENGINE_DOUBAO_PROVIDER_ID),
            "shared online ASR providers JSON should include Volcengine Doubao"
        );
        for provider in &manifest.providers {
            assert!(
                !provider.profile_id.trim().is_empty(),
                "online ASR provider profile id should not be empty"
            );
            validate_capability_config_fields(
                &provider.id,
                "streaming",
                &provider.streaming.required_config_fields,
            );
            validate_capability_config_fields(
                &provider.id,
                "batch",
                &provider.batch.required_config_fields,
            );
            if provider.streaming.requires_api_key || provider.batch.requires_api_key {
                assert!(
                    provider.defaults.get("apiKey").is_some()
                        || provider
                            .streaming
                            .required_config_fields
                            .iter()
                            .chain(provider.batch.required_config_fields.iter())
                            .any(|field| field == "apiKey"),
                    "online ASR provider requiring an API key should declare apiKey"
                );
            }
            if provider.batch.local_file_mode.supported {
                assert!(
                    !provider.batch.local_file_mode.endpoint.trim().is_empty(),
                    "online ASR local file mode endpoint should not be empty"
                );
            } else {
                assert!(
                    !provider
                        .batch
                        .local_file_mode
                        .unsupported_message
                        .trim()
                        .is_empty(),
                    "online ASR local file mode unsupported message should not be empty"
                );
            }
        }
        manifest
    })
}

fn validate_capability_config_fields(provider_id: &str, label: &str, fields: &[String]) {
    for field in fields {
        assert!(
            !field.trim().is_empty(),
            "online ASR provider {provider_id} {label} config field should not be empty"
        );
    }
}

pub fn online_asr_providers() -> &'static [OnlineAsrProvider] {
    manifest().providers.as_slice()
}

pub fn find_online_asr_provider(provider_id: &str) -> Option<&'static OnlineAsrProvider> {
    online_asr_providers()
        .iter()
        .find(|provider| provider.id == provider_id)
}
