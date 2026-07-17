use std::collections::HashSet;

use serde_json::{Map, Value};

use crate::config::defaults::CURRENT_CONFIG_VERSION;
use crate::ports::time::UnixMillisClock;
use crate::runtime::config::ServeConfigSection;
use crate::runtime::gpu::DEFAULT_GPU_ACCELERATION;
use crate::runtime::serve::{
    DEFAULT_JOB_TTL_MINUTES, DEFAULT_MAX_CONCURRENT, DEFAULT_MAX_QUEUE_SIZE, DEFAULT_MAX_STREAMING,
    DEFAULT_MAX_UPLOAD_SIZE_MB, DEFAULT_SERVE_HOST, DEFAULT_SERVE_IP_WHITELIST, DEFAULT_SERVE_PORT,
    ServeStartupSettings,
};

use super::{
    AppConfigLibrary, AppConfigRepositorySnapshot, AppConfigStartupProjection, AppConfigStore,
    AppConfigStoredState, HotwordRuleRecord, HotwordSetRecord, PolishKeywordSetRecord,
    PolishPresetRecord, SpeakerProfileRecord, SpeakerProfileSampleRecord, SummaryTemplateRecord,
    TextReplacementRuleRecord, TextReplacementSetRecord,
};

const SUMMARY_TEMPLATES_KEY: &str = "summaryCustomTemplates";
const POLISH_PRESETS_KEY: &str = "polishCustomPresets";
const TEXT_REPLACEMENT_SETS_KEY: &str = "textReplacementSets";
const HOTWORD_SETS_KEY: &str = "hotwordSets";
const POLISH_KEYWORD_SETS_KEY: &str = "polishKeywordSets";
const SPEAKER_PROFILES_KEY: &str = "speakerProfiles";

pub struct AppConfigRepositoryService<'a> {
    store: &'a dyn AppConfigStore,
    clock: &'a dyn UnixMillisClock,
}

impl<'a> AppConfigRepositoryService<'a> {
    pub fn new(store: &'a dyn AppConfigStore, clock: &'a dyn UnixMillisClock) -> Self {
        Self { store, clock }
    }

    pub fn load_config(&self) -> Result<Option<Value>, String> {
        self.store
            .load_state()?
            .map(app_config_value_from_stored_state)
            .transpose()
    }

    pub fn inspect_state(&self) -> Result<Option<AppConfigRepositorySnapshot>, String> {
        let Some(state) = self.store.load_state()? else {
            return Ok(None);
        };
        let summary_template_count = state.library.summary_templates.len() as u64;
        let polish_preset_count = state.library.polish_presets.len() as u64;
        let vocabulary_set_count = (state.library.text_replacement_sets.len()
            + state.library.hotword_sets.len()
            + state.library.polish_keyword_sets.len()) as u64;
        let speaker_profile_count = state.library.speaker_profiles.len() as u64;
        let config_version = state.config_version;
        let updated_at = state.updated_at;
        let config = app_config_value_from_stored_state(state)?;

        Ok(Some(AppConfigRepositorySnapshot {
            config,
            config_version,
            updated_at,
            summary_template_count,
            polish_preset_count,
            vocabulary_set_count,
            speaker_profile_count,
        }))
    }

    pub fn save_config(&self, config: &Value) -> Result<(), String> {
        let updated_at = self.clock.now_ms().unwrap_or_default() as i64;
        self.store
            .replace_state(app_config_stored_state_from_value(config, updated_at)?)
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<Value>, String> {
        self.store
            .load_setting_json(key)?
            .map(|json| serde_json::from_str(&json).map_err(serialization_error))
            .transpose()
    }

    pub fn set_setting(&self, key: &str, value: &Value) -> Result<(), String> {
        let value_json = serde_json::to_string(value).map_err(serialization_error)?;
        self.store.set_setting_json(key, value_json)
    }

    pub fn load_app_config_payload(&self) -> Result<Option<Value>, String> {
        self.store
            .load_base_config_json()?
            .map(|base_config_json| {
                let config =
                    serde_json::from_str(&base_config_json).map_err(serialization_error)?;
                Ok(app_config_payload_owned(config))
            })
            .transpose()
    }

    pub fn load_serve_startup_settings(&self) -> Result<Option<ServeStartupSettings>, String> {
        Ok(self
            .store
            .load_startup_projection()?
            .map(startup_settings_from_projection))
    }
}

pub fn app_config_stored_state_from_value(
    config: &Value,
    updated_at: i64,
) -> Result<AppConfigStoredState, String> {
    let startup_projection = AppConfigStartupProjection::from_config(config);
    let config_version = config_version_from_config(config);
    let (base_config, library) = extract_library_config(config);
    let base_config_json = serde_json::to_string(&base_config).map_err(serialization_error)?;

    Ok(AppConfigStoredState {
        base_config_json,
        library,
        config_version,
        updated_at,
        startup_projection,
    })
}

impl AppConfigStartupProjection {
    fn from_config(value: &Value) -> Self {
        let config = app_config_payload(value);
        Self {
            http_server_enabled: bool_field(config, "httpServerEnabled", false),
            host: string_field(config, "httpServerHost", DEFAULT_SERVE_HOST),
            port: integer_field(config, "httpServerPort", i64::from(DEFAULT_SERVE_PORT)),
            api_key: string_field(config, "httpServerApiKey", ""),
            max_concurrent: integer_field(
                config,
                "httpServerMaxConcurrent",
                DEFAULT_MAX_CONCURRENT as i64,
            ),
            max_queue_size: integer_field(
                config,
                "httpServerMaxQueueSize",
                DEFAULT_MAX_QUEUE_SIZE as i64,
            ),
            max_upload_size_mb: integer_field(
                config,
                "httpServerMaxUploadSizeMB",
                DEFAULT_MAX_UPLOAD_SIZE_MB as i64,
            ),
            job_ttl_minutes: integer_field(
                config,
                "httpServerJobTtlMinutes",
                DEFAULT_JOB_TTL_MINUTES as i64,
            ),
            max_streaming: integer_field(
                config,
                "httpServerMaxStreaming",
                DEFAULT_MAX_STREAMING as i64,
            ),
            ip_whitelist: string_field(config, "httpServerIpWhitelist", DEFAULT_SERVE_IP_WHITELIST),
            gpu_acceleration: string_field(config, "gpuAcceleration", DEFAULT_GPU_ACCELERATION),
        }
    }
}

pub fn app_config_value_from_stored_state(state: AppConfigStoredState) -> Result<Value, String> {
    let config = serde_json::from_str(&state.base_config_json).map_err(serialization_error)?;
    inject_library_config(config, state.library)
}

fn extract_library_config(config: &Value) -> (Value, AppConfigLibrary) {
    let mut base_config = config.clone();
    let payload = app_config_payload_mut(&mut base_config);
    let mut raw_library = RawLibraryConfig {
        summary_templates: take_array_field(payload, SUMMARY_TEMPLATES_KEY),
        polish_presets: take_array_field(payload, POLISH_PRESETS_KEY),
        text_replacement_sets: take_array_field(payload, TEXT_REPLACEMENT_SETS_KEY),
        hotword_sets: take_array_field(payload, HOTWORD_SETS_KEY),
        polish_keyword_sets: take_array_field(payload, POLISH_KEYWORD_SETS_KEY),
        speaker_profiles: take_array_field(payload, SPEAKER_PROFILES_KEY),
    };
    repair_library_config_ids(&mut raw_library);
    (base_config, typed_library(raw_library))
}

fn inject_library_config(mut config: Value, library: AppConfigLibrary) -> Result<Value, String> {
    let payload = ensure_object(app_config_payload_mut(&mut config));
    insert_serialized(payload, SUMMARY_TEMPLATES_KEY, library.summary_templates)?;
    insert_serialized(payload, POLISH_PRESETS_KEY, library.polish_presets)?;
    insert_serialized(
        payload,
        TEXT_REPLACEMENT_SETS_KEY,
        library.text_replacement_sets,
    )?;
    insert_serialized(payload, HOTWORD_SETS_KEY, library.hotword_sets)?;
    insert_serialized(
        payload,
        POLISH_KEYWORD_SETS_KEY,
        library.polish_keyword_sets,
    )?;
    insert_serialized(payload, SPEAKER_PROFILES_KEY, library.speaker_profiles)?;
    Ok(config)
}

fn insert_serialized(
    payload: &mut Map<String, Value>,
    key: &str,
    records: impl serde::Serialize,
) -> Result<(), String> {
    payload.insert(
        key.to_string(),
        serde_json::to_value(records).map_err(serialization_error)?,
    );
    Ok(())
}

fn typed_library(raw: RawLibraryConfig) -> AppConfigLibrary {
    AppConfigLibrary {
        summary_templates: raw
            .summary_templates
            .iter()
            .map(|value| SummaryTemplateRecord {
                id: string_field(value, "id", ""),
                name: string_field(value, "name", ""),
                instructions: string_field(value, "instructions", ""),
            })
            .collect(),
        polish_presets: raw
            .polish_presets
            .iter()
            .map(|value| PolishPresetRecord {
                id: string_field(value, "id", ""),
                name: string_field(value, "name", ""),
                context: string_field(value, "context", ""),
            })
            .collect(),
        text_replacement_sets: raw
            .text_replacement_sets
            .iter()
            .map(|value| TextReplacementSetRecord {
                id: string_field(value, "id", ""),
                name: string_field(value, "name", ""),
                enabled: bool_field(value, "enabled", true),
                ignore_case: bool_field(value, "ignoreCase", false),
                rules: array_field(value, "rules")
                    .iter()
                    .filter(|rule| rule.is_object())
                    .map(|rule| TextReplacementRuleRecord {
                        id: string_field(rule, "id", ""),
                        from: string_field(rule, "from", ""),
                        to: string_field(rule, "to", ""),
                    })
                    .collect(),
            })
            .collect(),
        hotword_sets: raw
            .hotword_sets
            .iter()
            .map(|value| HotwordSetRecord {
                id: string_field(value, "id", ""),
                name: string_field(value, "name", ""),
                enabled: bool_field(value, "enabled", true),
                rules: array_field(value, "rules")
                    .iter()
                    .filter(|rule| rule.is_object())
                    .map(|rule| HotwordRuleRecord {
                        id: string_field(rule, "id", ""),
                        text: string_field(rule, "text", ""),
                    })
                    .collect(),
            })
            .collect(),
        polish_keyword_sets: raw
            .polish_keyword_sets
            .iter()
            .map(|value| PolishKeywordSetRecord {
                id: string_field(value, "id", ""),
                name: string_field(value, "name", ""),
                enabled: bool_field(value, "enabled", true),
                keywords: string_field(value, "keywords", ""),
            })
            .collect(),
        speaker_profiles: raw
            .speaker_profiles
            .iter()
            .map(|value| SpeakerProfileRecord {
                id: string_field(value, "id", ""),
                name: string_field(value, "name", ""),
                enabled: bool_field(value, "enabled", true),
                samples: array_field(value, "samples")
                    .iter()
                    .filter(|sample| sample.is_object())
                    .map(|sample| SpeakerProfileSampleRecord {
                        id: string_field(sample, "id", ""),
                        file_path: string_field(sample, "filePath", ""),
                        source_name: string_field(sample, "sourceName", ""),
                        duration_seconds: float_field(sample, "durationSeconds", 0.0),
                    })
                    .collect(),
            })
            .collect(),
    }
}

#[derive(Default)]
struct RawLibraryConfig {
    summary_templates: Vec<Value>,
    polish_presets: Vec<Value>,
    text_replacement_sets: Vec<Value>,
    hotword_sets: Vec<Value>,
    polish_keyword_sets: Vec<Value>,
    speaker_profiles: Vec<Value>,
}

fn repair_library_config_ids(library: &mut RawLibraryConfig) {
    ensure_unique_ids(&mut library.summary_templates, "summary-template");
    ensure_unique_ids(&mut library.polish_presets, "polish-preset");
    ensure_unique_ids(&mut library.text_replacement_sets, "text-replacement-set");
    ensure_nested_unique_ids(
        &mut library.text_replacement_sets,
        "rules",
        "text-replacement-rule",
    );
    ensure_unique_ids(&mut library.hotword_sets, "hotword-set");
    ensure_nested_unique_ids(&mut library.hotword_sets, "rules", "hotword-rule");
    ensure_unique_ids(&mut library.polish_keyword_sets, "polish-keyword-set");
    ensure_unique_ids(&mut library.speaker_profiles, "speaker-profile");
    ensure_nested_unique_ids(&mut library.speaker_profiles, "samples", "speaker-sample");
}

fn ensure_unique_ids(values: &mut Vec<Value>, id_prefix: &str) {
    values.retain(Value::is_object);
    let mut seen = HashSet::new();
    for (index, value) in values.iter_mut().enumerate() {
        let current_id = value.get("id").and_then(non_empty_str).map(str::to_string);
        let next_id = if let Some(id) = current_id {
            if seen.insert(id.clone()) {
                id
            } else {
                unique_generated_id(id_prefix, value, index, &mut seen)
            }
        } else {
            unique_generated_id(id_prefix, value, index, &mut seen)
        };

        if value.get("id").and_then(Value::as_str) != Some(next_id.as_str())
            && let Some(object) = value.as_object_mut()
        {
            object.insert("id".to_string(), Value::String(next_id));
        }
    }
}

fn ensure_nested_unique_ids(values: &mut [Value], field: &str, id_prefix: &str) {
    for value in values {
        let Some(object) = value.as_object_mut() else {
            continue;
        };
        let mut nested = object
            .remove(field)
            .and_then(|value| value.as_array().cloned())
            .unwrap_or_default();
        ensure_unique_ids(&mut nested, id_prefix);
        object.insert(field.to_string(), Value::Array(nested));
    }
}

fn unique_generated_id(
    id_prefix: &str,
    value: &Value,
    index: usize,
    seen: &mut HashSet<String>,
) -> String {
    let serialized = serde_json::to_string(value).unwrap_or_default();
    let base = format!(
        "{id_prefix}-{}",
        hash_string(&format!("{serialized}-{index}"))
    );
    let mut candidate = base.clone();
    let mut suffix = 2;
    while !seen.insert(candidate.clone()) {
        candidate = format!("{base}-{suffix}");
        suffix += 1;
    }
    candidate
}

fn hash_string(value: &str) -> String {
    let mut hash: u32 = 5381;
    for unit in value.encode_utf16() {
        hash = hash.wrapping_shl(5).wrapping_add(hash) ^ u32::from(unit);
    }
    format!("{hash:08x}")
}

fn non_empty_str(value: &Value) -> Option<&str> {
    value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn startup_settings_from_projection(
    projection: AppConfigStartupProjection,
) -> ServeStartupSettings {
    ServeStartupSettings {
        enabled: projection.http_server_enabled,
        config: ServeConfigSection {
            host: Some(projection.host),
            port: Some(u16_or_default(projection.port, DEFAULT_SERVE_PORT)),
            api_key: Some(projection.api_key),
            models_dir: None,
            max_concurrent: Some(usize_or_default(
                projection.max_concurrent,
                DEFAULT_MAX_CONCURRENT,
            )),
            max_queue_size: Some(usize_or_default(
                projection.max_queue_size,
                DEFAULT_MAX_QUEUE_SIZE,
            )),
            max_upload_size_mb: Some(usize_or_default(
                projection.max_upload_size_mb,
                DEFAULT_MAX_UPLOAD_SIZE_MB,
            )),
            job_ttl_minutes: Some(u64_or_default(
                projection.job_ttl_minutes,
                DEFAULT_JOB_TTL_MINUTES,
            )),
            max_streaming: Some(usize_or_default(
                projection.max_streaming,
                DEFAULT_MAX_STREAMING,
            )),
            ip_whitelist: Some(projection.ip_whitelist),
            gpu_acceleration: Some(projection.gpu_acceleration),
            vad_model_id: None,
            punctuation_model_id: None,
        },
    }
}

pub(super) fn app_config_payload(value: &Value) -> &Value {
    value
        .get("sona-config")
        .filter(|value| value.is_object())
        .or_else(|| value.get("sona_config"))
        .filter(|value| value.is_object())
        .or_else(|| value.get("config"))
        .filter(|value| value.is_object())
        .unwrap_or(value)
}

fn app_config_payload_owned(value: Value) -> Value {
    app_config_payload(&value).clone()
}

fn app_config_payload_mut(value: &mut Value) -> &mut Value {
    let nested_key = if value.get("sona-config").is_some_and(Value::is_object) {
        Some("sona-config")
    } else if value.get("sona_config").is_some_and(Value::is_object) {
        Some("sona_config")
    } else if value.get("config").is_some_and(Value::is_object) {
        Some("config")
    } else {
        None
    };
    if let Some(key) = nested_key {
        value.get_mut(key).unwrap()
    } else {
        value
    }
}

fn ensure_object(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = Value::Object(Map::new());
    }
    value.as_object_mut().unwrap()
}

fn take_array_field(value: &mut Value, key: &str) -> Vec<Value> {
    value
        .as_object_mut()
        .and_then(|object| object.remove(key))
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default()
}

fn array_field(value: &Value, key: &str) -> Vec<Value> {
    value
        .get(key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn config_version_from_config(value: &Value) -> i64 {
    integer_field(
        app_config_payload(value),
        "configVersion",
        CURRENT_CONFIG_VERSION,
    )
}

fn bool_field(value: &Value, key: &str, default: bool) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(default)
}

fn string_field(value: &Value, key: &str, default: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_string()
}

fn integer_field(value: &Value, key: &str, default: i64) -> i64 {
    value
        .get(key)
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_u64().and_then(|n| i64::try_from(n).ok()))
                .or_else(|| value.as_f64().map(|n| n.round() as i64))
        })
        .unwrap_or(default)
}

fn float_field(value: &Value, key: &str, default: f64) -> f64 {
    value.get(key).and_then(Value::as_f64).unwrap_or(default)
}

fn u16_or_default(value: i64, default: u16) -> u16 {
    u16::try_from(value).unwrap_or(default)
}

fn usize_or_default(value: i64, default: usize) -> usize {
    usize::try_from(value).unwrap_or(default)
}

fn u64_or_default(value: i64, default: u64) -> u64 {
    u64::try_from(value).unwrap_or(default)
}

fn serialization_error(error: serde_json::Error) -> String {
    format!("Serialization error: {error}")
}
