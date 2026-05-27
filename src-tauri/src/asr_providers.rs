use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::OnceLock;

const ONLINE_ASR_PROVIDERS_JSON: &str = include_str!("../../src/shared/online-asr-providers.json");

pub const VOLCENGINE_DOUBAO_PROVIDER_ID: &str = "volcengine-doubao";
pub const VOLCENGINE_DOUBAO_LEGACY_PROVIDER_KEY: &str = "volcengineDoubao";
pub const VOLCENGINE_DOUBAO_LEGACY_ENGINE_ID: &str = "volcengine-doubao";

pub const GROQ_WHISPER_PROVIDER_ID: &str = "groq-whisper";

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
pub struct VolcengineDoubaoDefaults {
    pub api_key: String,
    pub streaming_endpoint: String,
    pub streaming_resource_id: String,
    pub batch_endpoint: String,
    pub batch_resource_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroqWhisperDefaults {
    pub api_key: String,
    pub batch_endpoint: String,
    pub model: String,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolcengineDoubaoConfigFields {
    pub api_key: String,
    pub streaming_endpoint: String,
    pub streaming_resource_id: String,
    pub batch_endpoint: String,
    pub batch_resource_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroqWhisperConfigFields {
    pub api_key: String,
    pub batch_endpoint: String,
    pub model: String,
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
        manifest
    })
}

pub fn online_asr_providers() -> &'static [OnlineAsrProvider] {
    manifest().providers.as_slice()
}

pub fn find_online_asr_provider(provider_id: &str) -> Option<&'static OnlineAsrProvider> {
    online_asr_providers()
        .iter()
        .find(|provider| provider.id == provider_id)
}

pub fn volcengine_doubao_provider() -> &'static OnlineAsrProvider {
    find_online_asr_provider(VOLCENGINE_DOUBAO_PROVIDER_ID)
        .expect("Volcengine Doubao provider should exist in shared online ASR manifest")
}

pub fn volcengine_doubao_defaults() -> VolcengineDoubaoDefaults {
    serde_json::from_value(volcengine_doubao_provider().defaults.clone())
        .expect("valid Volcengine Doubao defaults")
}

pub fn is_volcengine_doubao_provider_id(provider_id: &str) -> bool {
    provider_id == volcengine_doubao_provider().id
}

pub fn volcengine_doubao_profile_id() -> &'static str {
    volcengine_doubao_provider().profile_id.as_str()
}

pub fn groq_whisper_provider() -> &'static OnlineAsrProvider {
    find_online_asr_provider(GROQ_WHISPER_PROVIDER_ID)
        .expect("Groq Whisper provider should exist in shared online ASR manifest")
}

pub fn groq_whisper_defaults() -> GroqWhisperDefaults {
    serde_json::from_value(groq_whisper_provider().defaults.clone())
        .expect("valid Groq Whisper defaults")
}

pub fn is_groq_whisper_provider_id(provider_id: &str) -> bool {
    provider_id == groq_whisper_provider().id
}

pub fn groq_whisper_profile_id() -> &'static str {
    "groq-whisper"
}

pub fn default_groq_whisper_provider_json() -> Value {
    let defaults = groq_whisper_defaults();
    json!({
        "apiKey": defaults.api_key,
        "batchEndpoint": defaults.batch_endpoint,
        "model": defaults.model,
    })
}

pub fn default_volcengine_doubao_provider_json() -> Value {
    let defaults = volcengine_doubao_defaults();
    json!({
        "apiKey": defaults.api_key,
        "streamingEndpoint": defaults.streaming_endpoint,
        "streamingResourceId": defaults.streaming_resource_id,
        "batchEndpoint": defaults.batch_endpoint,
        "batchResourceId": defaults.batch_resource_id,
    })
}

pub fn online_provider_from_providers<'a>(
    providers: Option<&'a Value>,
    provider_id: &str,
) -> Option<&'a Value> {
    providers
        .and_then(|value| value.get("online"))
        .and_then(|value| value.get(provider_id))
}

pub fn volcengine_provider_from_providers(providers: Option<&Value>) -> Option<&Value> {
    let modern = online_provider_from_providers(providers, VOLCENGINE_DOUBAO_PROVIDER_ID);
    let legacy = providers.and_then(|value| value.get(VOLCENGINE_DOUBAO_LEGACY_PROVIDER_KEY));
    if let (Some(modern), Some(legacy)) = (modern, legacy) {
        if normalize_volcengine_doubao_provider_json(Some(modern))
            == default_volcengine_doubao_provider_json()
        {
            return Some(legacy);
        }
    }
    modern.or(legacy)
}

pub fn normalize_volcengine_doubao_provider_json(existing: Option<&Value>) -> Value {
    let fields = fill_volcengine_doubao_config_fields(
        existing.and_then(|value| string_field(value, "apiKey")),
        existing.and_then(|value| string_field(value, "streamingEndpoint")),
        existing.and_then(|value| string_field(value, "streamingResourceId")),
        existing.and_then(|value| string_field(value, "batchEndpoint")),
        existing.and_then(|value| string_field(value, "batchResourceId")),
    );
    let batch =
        if is_volcengine_local_file_batch_mode(&fields.batch_endpoint, &fields.batch_resource_id) {
            (fields.batch_endpoint, fields.batch_resource_id)
        } else {
            let defaults = volcengine_doubao_defaults();
            (
                defaults.batch_endpoint,
                defaults.batch_resource_id,
            )
        };

    json!({
        "apiKey": fields.api_key,
        "streamingEndpoint": fields.streaming_endpoint,
        "streamingResourceId": fields.streaming_resource_id,
        "batchEndpoint": batch.0,
        "batchResourceId": batch.1,
    })
}

pub fn fill_volcengine_doubao_config_fields(
    api_key: Option<&str>,
    streaming_endpoint: Option<&str>,
    streaming_resource_id: Option<&str>,
    batch_endpoint: Option<&str>,
    batch_resource_id: Option<&str>,
) -> VolcengineDoubaoConfigFields {
    let defaults = volcengine_doubao_defaults();
    VolcengineDoubaoConfigFields {
        api_key: trim_or_default(api_key, &defaults.api_key),
        streaming_endpoint: trim_or_default(streaming_endpoint, &defaults.streaming_endpoint),
        streaming_resource_id: trim_or_default(
            streaming_resource_id,
            &defaults.streaming_resource_id,
        ),
        batch_endpoint: trim_or_default(batch_endpoint, &defaults.batch_endpoint),
        batch_resource_id: trim_or_default(batch_resource_id, &defaults.batch_resource_id),
    }
}

pub fn is_volcengine_local_file_batch_mode(endpoint: &str, resource_id: &str) -> bool {
    let local_file_mode = &volcengine_doubao_provider().batch.local_file_mode;
    normalized_endpoint(endpoint) == normalized_endpoint(&local_file_mode.endpoint)
        && resource_id.trim() == local_file_mode.resource_id
}

pub fn volcengine_local_file_batch_unsupported_message() -> &'static str {
    volcengine_doubao_provider()
        .batch
        .local_file_mode
        .unsupported_message
        .as_str()
}

pub fn is_volcengine_doubao_selection(selection: Option<&Value>) -> bool {
    match selection
        .and_then(|selection| string_field(selection, "engine"))
        .map(str::trim)
    {
        Some("online") => selection
            .and_then(|selection| string_field(selection, "providerId"))
            .map(str::trim)
            .is_some_and(is_volcengine_doubao_provider_id),
        Some(VOLCENGINE_DOUBAO_LEGACY_ENGINE_ID) => true,
        _ => false,
    }
}

pub fn is_volcengine_streaming_config_fields_complete(
    config: &VolcengineDoubaoConfigFields,
) -> bool {
    let definition = volcengine_doubao_provider();
    has_api_key(definition.streaming.requires_api_key, &config.api_key)
        && has_required_config_fields(config, &definition.streaming.required_config_fields)
}

pub fn is_volcengine_local_file_batch_config_fields_complete(
    config: &VolcengineDoubaoConfigFields,
) -> bool {
    let definition = volcengine_doubao_provider();
    has_api_key(definition.batch.requires_api_key, &config.api_key)
        && has_required_config_fields(config, &definition.batch.required_config_fields)
        && definition.batch.local_file_mode.supported
        && is_volcengine_local_file_batch_mode(&config.batch_endpoint, &config.batch_resource_id)
}

pub fn is_volcengine_local_file_batch_config_complete(provider: Option<&Value>) -> bool {
    let fields = fields_from_provider_value(provider);
    is_volcengine_local_file_batch_config_fields_complete(&fields)
}

fn fields_from_provider_value(provider: Option<&Value>) -> VolcengineDoubaoConfigFields {
    fill_volcengine_doubao_config_fields(
        provider.and_then(|value| string_field(value, "apiKey")),
        provider.and_then(|value| string_field(value, "streamingEndpoint")),
        provider.and_then(|value| string_field(value, "streamingResourceId")),
        provider.and_then(|value| string_field(value, "batchEndpoint")),
        provider.and_then(|value| string_field(value, "batchResourceId")),
    )
}

pub fn is_groq_whisper_batch_config_fields_complete(
    config: &GroqWhisperConfigFields,
) -> bool {
    let definition = groq_whisper_provider();
    has_api_key(definition.batch.requires_api_key, &config.api_key)
        && !config.batch_endpoint.trim().is_empty()
        && !config.model.trim().is_empty()
}

pub fn groq_whisper_provider_from_providers(providers: Option<&Value>) -> Option<&Value> {
    online_provider_from_providers(providers, GROQ_WHISPER_PROVIDER_ID)
}

pub fn normalize_groq_whisper_provider_json(existing: Option<&Value>) -> Value {
    let fields = fill_groq_whisper_config_fields(
        existing.and_then(|value| string_field(value, "apiKey")),
        existing.and_then(|value| string_field(value, "batchEndpoint")),
        existing.and_then(|value| string_field(value, "model")),
    );
    json!({
        "apiKey": fields.api_key,
        "batchEndpoint": fields.batch_endpoint,
        "model": fields.model,
    })
}

pub fn fill_groq_whisper_config_fields(
    api_key: Option<&str>,
    batch_endpoint: Option<&str>,
    model: Option<&str>,
) -> GroqWhisperConfigFields {
    let defaults = groq_whisper_defaults();
    GroqWhisperConfigFields {
        api_key: trim_or_default(api_key, &defaults.api_key),
        batch_endpoint: trim_or_default(batch_endpoint, &defaults.batch_endpoint),
        model: trim_or_default(model, &defaults.model),
    }
}

fn has_api_key(requires_api_key: bool, api_key: &str) -> bool {
    !requires_api_key || !api_key.trim().is_empty()
}

fn has_required_config_fields(
    fields: &VolcengineDoubaoConfigFields,
    required_fields: &[String],
) -> bool {
    required_fields.iter().all(|field| {
        let value = match field.as_str() {
            "streamingEndpoint" => &fields.streaming_endpoint,
            "streamingResourceId" => &fields.streaming_resource_id,
            "batchEndpoint" => &fields.batch_endpoint,
            "batchResourceId" => &fields.batch_resource_id,
            _ => return false,
        };
        !value.trim().is_empty()
    })
}

fn string_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn trim_or_default(value: Option<&str>, default: &str) -> String {
    let trimmed = value.map(str::trim).unwrap_or_default();
    if trimmed.is_empty() {
        default.to_string()
    } else {
        trimmed.to_string()
    }
}

fn normalized_endpoint(endpoint: &str) -> String {
    endpoint.trim().trim_end_matches('/').to_string()
}
