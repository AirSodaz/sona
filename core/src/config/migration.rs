use super::defaults::*;
use super::types::MigrationResult;
use crate::ports::asr::{
    VOLCENGINE_DOUBAO_LEGACY_PROVIDER_KEY, VOLCENGINE_DOUBAO_PROVIDER_ID, online_asr_providers,
};
use serde_json::{Map, Value, json};
use std::collections::HashSet;

pub(crate) fn migrate_app_config_inner(
    saved_config: Option<Value>,
    legacy_config: Option<Value>,
    default_rule_set_name: &str,
) -> MigrationResult {
    let mut is_legacy_migration = false;
    let Some(source) = saved_config.or_else(|| {
        if legacy_config.as_ref().is_some_and(Value::is_object) {
            is_legacy_migration = true;
        }
        legacy_config
    }) else {
        return MigrationResult {
            config: default_config(),
            migrated: false,
        };
    };

    let needs_upgrade = should_upgrade_config(&source, is_legacy_migration);
    if !needs_upgrade {
        let normalized = normalize_current_config(source.clone());
        let config = remove_internal_marker(normalized);
        return MigrationResult {
            migrated: current_config_needs_persist(&source, &config),
            config,
        };
    }

    MigrationResult {
        config: upgrade_config(source, default_rule_set_name),
        migrated: true,
    }
}

pub(crate) fn resolve_effective_config_inner(
    global_config: Value,
    project: Option<&Value>,
) -> Value {
    let mut config = as_object_clone(&global_config).unwrap_or_default();
    let global_summary_template_id = string_at(&Value::Object(config.clone()), "summaryTemplateId");
    let custom_templates = config.get("summaryCustomTemplates");

    let Some(project) = project.and_then(Value::as_object) else {
        config.insert(
            "summaryTemplateId".to_string(),
            Value::String(coerce_summary_template_id(
                global_summary_template_id.as_deref(),
                custom_templates,
            )),
        );
        return Value::Object(config);
    };
    let defaults = project.get("defaults").and_then(Value::as_object);

    config.insert(
        "summaryTemplateId".to_string(),
        Value::String(coerce_summary_template_id(
            defaults
                .and_then(|value| value.get("summaryTemplateId"))
                .and_then(Value::as_str)
                .or(global_summary_template_id.as_deref()),
            custom_templates,
        )),
    );
    config.insert(
        "translationLanguage".to_string(),
        Value::String(
            defaults
                .and_then(|value| value.get("translationLanguage"))
                .and_then(non_empty_str)
                .or_else(|| config.get("translationLanguage").and_then(non_empty_str))
                .unwrap_or("zh")
                .to_string(),
        ),
    );
    let selected_polish_preset_id = defaults
        .and_then(|value| value.get("polishPresetId"))
        .and_then(flatten_id_value)
        .or_else(|| config.get("polishPresetId").and_then(flatten_id_value));
    let polish_preset_id = coerce_polish_preset_id(
        selected_polish_preset_id.as_deref(),
        config.get("polishCustomPresets"),
    );
    config.insert(
        "polishPresetId".to_string(),
        Value::String(polish_preset_id),
    );

    for (field, defaults_field) in [
        ("textReplacementSets", "enabledTextReplacementSetIds"),
        ("hotwordSets", "enabledHotwordSetIds"),
        ("polishKeywordSets", "enabledPolishKeywordSetIds"),
        ("speakerProfiles", "enabledSpeakerProfileIds"),
    ] {
        let enabled_ids = defaults
            .and_then(|value| value.get(defaults_field))
            .and_then(array_strings)
            .unwrap_or_default();
        let sets = config.get(field).cloned().unwrap_or_else(|| json!([]));
        config.insert(field.to_string(), set_enabled_by_ids(&sets, &enabled_ids));
    }

    Value::Object(config)
}

fn should_upgrade_config(config: &Value, is_config_migrated: bool) -> bool {
    if is_config_migrated {
        return true;
    }

    let version = config
        .get("configVersion")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    if version < CURRENT_CONFIG_VERSION {
        return true;
    }

    if config.get("summaryEnabled").is_none() {
        return true;
    }

    if config.get("httpServerEnabled").is_none() {
        return true;
    }

    if config.get("keepMicrophoneActive").is_none() {
        return true;
    }

    for key in [
        "polishCustomPresets",
        "polishKeywordSets",
        "summaryCustomTemplates",
        "speakerProfiles",
    ] {
        if !config.get(key).is_some_and(Value::is_array) {
            return true;
        }
    }

    if normalize_log_level(config.get("logLevel"))
        != config
            .get("logLevel")
            .and_then(Value::as_str)
            .unwrap_or("info")
    {
        return true;
    }

    for key in [
        "speakerSegmentationModelPath",
        "speakerEmbeddingModelPath",
        "polishPresetId",
        "summaryTemplateId",
    ] {
        if config.get(key).and_then(flatten_id_value).is_none()
            && (key == "polishPresetId" || key == "summaryTemplateId")
        {
            return true;
        }
        if (key == "speakerSegmentationModelPath" || key == "speakerEmbeddingModelPath")
            && !config.get(key).is_some_and(Value::is_string)
        {
            return true;
        }
    }

    if !has_valid_or_legacy_current_asr_config(config) {
        return true;
    }

    false
}

fn normalize_current_config(existing: Value) -> Value {
    let original = existing.clone();
    let mut config = merge_default(existing);
    let summary_custom_templates =
        normalize_summary_custom_templates(config.get("summaryCustomTemplates"));
    let summary_template_id = coerce_summary_template_id(
        config.get("summaryTemplateId").and_then(flatten_id_value),
        Some(&summary_custom_templates),
    );
    let polish_custom_presets = normalize_polish_custom_presets(config.get("polishCustomPresets"));
    let polish_preset_id = coerce_polish_preset_id(
        config.get("polishPresetId").and_then(flatten_id_value),
        Some(&polish_custom_presets),
    );
    let polish_keyword_sets = migrate_legacy_polish_keywords(
        config.get("polishKeywords").and_then(Value::as_str),
        Some(&normalize_polish_keyword_sets(
            config.get("polishKeywordSets"),
        )),
    );

    set(&mut config, "configVersion", json!(CURRENT_CONFIG_VERSION));
    let llm_settings = ensure_llm_state(&config);
    let summary_enabled = config
        .get("summaryEnabled")
        .and_then(Value::as_bool)
        .map(Value::Bool)
        .unwrap_or(json!(true));
    let log_level = normalize_log_level(config.get("logLevel"));
    let speaker_segmentation_model_path =
        string_at(&config, "speakerSegmentationModelPath").unwrap_or_default();
    let speaker_embedding_model_path =
        string_at(&config, "speakerEmbeddingModelPath").unwrap_or_default();
    let speaker_profiles = normalize_speaker_profiles(config.get("speakerProfiles"));
    let asr_config = normalize_asr_config(&config);

    set(&mut config, "llmSettings", llm_settings);
    set(&mut config, "summaryEnabled", summary_enabled);
    set(&mut config, "summaryTemplateId", json!(summary_template_id));
    set(
        &mut config,
        "summaryCustomTemplates",
        summary_custom_templates,
    );
    set(&mut config, "polishKeywords", json!(""));
    set(&mut config, "polishPresetId", json!(polish_preset_id));
    set(&mut config, "polishCustomPresets", polish_custom_presets);
    set(&mut config, "polishKeywordSets", polish_keyword_sets);
    set(&mut config, "logLevel", json!(log_level));
    set(
        &mut config,
        "speakerSegmentationModelPath",
        json!(speaker_segmentation_model_path),
    );
    set(
        &mut config,
        "speakerEmbeddingModelPath",
        json!(speaker_embedding_model_path),
    );
    let http_server_enabled = bool_at(&config, "httpServerEnabled").unwrap_or(false);
    let http_server_port = number_or_default(&config, "httpServerPort", 14200);
    let http_server_host = string_or_default(&config, "httpServerHost", "127.0.0.1");
    let http_server_api_key = string_or_default(&config, "httpServerApiKey", "");
    let keep_microphone_active = bool_at(&config, "keepMicrophoneActive").unwrap_or(false);

    set(&mut config, "speakerProfiles", speaker_profiles);
    set(&mut config, "asr", asr_config);
    set(&mut config, "httpServerEnabled", json!(http_server_enabled));
    set(&mut config, "httpServerPort", json!(http_server_port));
    set(&mut config, "httpServerHost", json!(http_server_host));
    set(&mut config, "httpServerApiKey", json!(http_server_api_key));
    set(
        &mut config,
        "keepMicrophoneActive",
        json!(keep_microphone_active),
    );
    sanitize_typed_config_fields(&mut config);
    set(&mut config, "__original", original);
    config
}

fn remove_internal_marker(mut value: Value) -> Value {
    if let Some(object) = value.as_object_mut() {
        object.remove("__original");
    }
    value
}

fn flatten_id_value(value: &Value) -> Option<&str> {
    value.as_str().or_else(|| {
        value
            .get("Builtin")
            .or_else(|| value.get("Custom"))
            .and_then(Value::as_str)
    })
}

fn current_config_needs_persist(existing: &Value, normalized: &Value) -> bool {
    if existing.get("llmSettings") != normalized.get("llmSettings") {
        return true;
    }
    if existing.get("summaryEnabled") != normalized.get("summaryEnabled") {
        return true;
    }
    if existing.get("polishCustomPresets") != normalized.get("polishCustomPresets")
        || existing.get("polishPresetId") != normalized.get("polishPresetId")
        || existing.get("configVersion") != normalized.get("configVersion")
    {
        return true;
    }
    if existing.get("summaryCustomTemplates") != normalized.get("summaryCustomTemplates")
        || existing.get("summaryTemplateId") != normalized.get("summaryTemplateId")
    {
        return true;
    }
    if existing.get("polishKeywordSets") != normalized.get("polishKeywordSets")
        || existing
            .get("polishKeywords")
            .and_then(Value::as_str)
            .unwrap_or_default()
            != normalized
                .get("polishKeywords")
                .and_then(Value::as_str)
                .unwrap_or_default()
    {
        return true;
    }
    if existing.get("speakerProfiles") != normalized.get("speakerProfiles")
        || !existing
            .get("speakerSegmentationModelPath")
            .is_some_and(Value::is_string)
        || !existing
            .get("speakerEmbeddingModelPath")
            .is_some_and(Value::is_string)
    {
        return true;
    }
    if asr_config_needs_persist(existing.get("asr"), normalized.get("asr")) {
        return true;
    }
    if existing.get("httpServerEnabled") != normalized.get("httpServerEnabled")
        || existing.get("httpServerPort") != normalized.get("httpServerPort")
        || existing.get("httpServerHost") != normalized.get("httpServerHost")
        || existing.get("httpServerApiKey") != normalized.get("httpServerApiKey")
    {
        return true;
    }
    if existing.get("keepMicrophoneActive") != normalized.get("keepMicrophoneActive") {
        return true;
    }
    existing.get("logLevel") != normalized.get("logLevel")
}

fn asr_config_needs_persist(existing: Option<&Value>, normalized: Option<&Value>) -> bool {
    if existing == normalized {
        return false;
    }
    let Some(existing) = existing else {
        return normalized.is_some();
    };
    let Some(normalized) = normalized else {
        return true;
    };
    if existing.get("selections") != normalized.get("selections") {
        return true;
    }

    let existing_providers = existing.get("providers").and_then(|v| v.get("online"));
    let normalized_providers = normalized.get("providers").and_then(|v| v.get("online"));

    for provider in online_asr_providers() {
        let existing_provider = existing_providers.and_then(|v| v.get(&provider.id));
        let normalized_provider = normalized_providers.and_then(|v| v.get(&provider.id));

        // Handle legacy Volcengine provider key lookup
        let existing_provider =
            if provider.id == VOLCENGINE_DOUBAO_PROVIDER_ID && existing_provider.is_none() {
                existing
                    .get("providers")
                    .and_then(|v| v.get(VOLCENGINE_DOUBAO_LEGACY_PROVIDER_KEY))
            } else {
                existing_provider
            };

        if existing_provider != normalized_provider {
            if let (None, Some(norm_prov)) = (existing_provider, normalized_provider) {
                if *norm_prov != provider.defaults {
                    return true;
                }
            } else {
                return true;
            }
        }
    }

    false
}

fn upgrade_config(parsed: Value, default_rule_set_name: &str) -> Value {
    let mut config = merge_default(parsed.clone());
    let streaming_model_path = first_string(
        &parsed,
        &[
            "streamingModelPath",
            "recognitionModelPath",
            "offlineModelPath",
            "modelPath",
        ],
    )
    .unwrap_or_default();
    let batch_model_path = first_string(
        &parsed,
        &["offlineModelPath", "recognitionModelPath", "modelPath"],
    )
    .unwrap_or_default();
    let normalized_polish_custom_presets =
        normalize_polish_custom_presets(parsed.get("polishCustomPresets"));
    let normalized_summary_custom_templates =
        normalize_summary_custom_templates(parsed.get("summaryCustomTemplates"));
    let normalized_polish_keyword_sets =
        normalize_polish_keyword_sets(parsed.get("polishKeywordSets"));
    let (polish_preset_id, polish_custom_presets) = migrate_legacy_polish_selection(
        parsed.get("polishPresetId").and_then(Value::as_str),
        parsed.get("polishScenario").and_then(Value::as_str),
        parsed.get("polishContext").and_then(Value::as_str),
        &normalized_polish_custom_presets,
        Some("Imported Preset"),
    );

    for (key, value) in [
        ("configVersion", json!(CURRENT_CONFIG_VERSION)),
        ("streamingModelPath", json!(streaming_model_path.clone())),
        ("batchModelPath", json!(batch_model_path.clone())),
        (
            "punctuationModelPath",
            json!(string_at(&parsed, "punctuationModelPath").unwrap_or_default()),
        ),
        (
            "vadModelPath",
            json!(string_at(&parsed, "vadModelPath").unwrap_or_default()),
        ),
        (
            "enableITN",
            json!(bool_at(&parsed, "enableITN").unwrap_or(true)),
        ),
        (
            "vadBufferSize",
            json!(number_or_default(&parsed, "vadBufferSize", 5)),
        ),
        (
            "batchVadEnabled",
            json!(bool_at(&parsed, "batchVadEnabled").unwrap_or(true)),
        ),
        (
            "maxConcurrent",
            json!(number_or_default(&parsed, "maxConcurrent", 2)),
        ),
        (
            "appLanguage",
            json!(string_or_default(&parsed, "appLanguage", "auto")),
        ),
        ("theme", json!(string_or_default(&parsed, "theme", "auto"))),
        ("font", json!(string_or_default(&parsed, "font", "system"))),
        (
            "language",
            json!(string_or_default(&parsed, "language", "auto")),
        ),
        (
            "enableTimeline",
            json!(bool_at(&parsed, "enableTimeline").unwrap_or(false)),
        ),
        (
            "minimizeToTrayOnExit",
            json!(bool_at(&parsed, "minimizeToTrayOnExit").unwrap_or(true)),
        ),
        (
            "lockWindow",
            json!(bool_at(&parsed, "lockWindow").unwrap_or(false)),
        ),
        (
            "alwaysOnTop",
            json!(bool_at(&parsed, "alwaysOnTop").unwrap_or(true)),
        ),
        (
            "microphoneId",
            json!(string_or_default(&parsed, "microphoneId", "default")),
        ),
        (
            "microphoneBoost",
            json!(number_f64_or_default(&parsed, "microphoneBoost", 1.0)),
        ),
        (
            "systemAudioDeviceId",
            json!(string_or_default(&parsed, "systemAudioDeviceId", "default")),
        ),
        (
            "muteDuringRecording",
            json!(bool_at(&parsed, "muteDuringRecording").unwrap_or(false)),
        ),
        (
            "keepMicrophoneActive",
            json!(bool_at(&parsed, "keepMicrophoneActive").unwrap_or(false)),
        ),
        (
            "startOnLaunch",
            json!(bool_at(&parsed, "startOnLaunch").unwrap_or(false)),
        ),
        (
            "captionWindowWidth",
            json!(number_or_default(&parsed, "captionWindowWidth", 800)),
        ),
        (
            "captionFontSize",
            json!(number_or_default(&parsed, "captionFontSize", 24)),
        ),
        (
            "captionFontColor",
            json!(string_or_default(&parsed, "captionFontColor", "#ffffff")),
        ),
        (
            "captionBackgroundOpacity",
            json!(number_f64_or_default(
                &parsed,
                "captionBackgroundOpacity",
                0.6
            )),
        ),
        ("llmSettings", ensure_llm_state(&parsed)),
        (
            "summaryEnabled",
            json!(bool_at(&parsed, "summaryEnabled").unwrap_or(true)),
        ),
        (
            "summaryTemplateId",
            json!(coerce_summary_template_id(
                parsed
                    .get("summaryTemplateId")
                    .and_then(Value::as_str)
                    .or_else(|| parsed.get("summaryTemplate").and_then(Value::as_str)),
                Some(&normalized_summary_custom_templates)
            )),
        ),
        (
            "summaryCustomTemplates",
            normalized_summary_custom_templates,
        ),
        (
            "translationLanguage",
            json!(string_or_default(&parsed, "translationLanguage", "zh")),
        ),
        ("polishKeywords", json!("")),
        ("polishPresetId", json!(polish_preset_id)),
        ("polishCustomPresets", polish_custom_presets),
        (
            "polishKeywordSets",
            migrate_legacy_polish_keywords(
                parsed.get("polishKeywords").and_then(Value::as_str),
                Some(&normalized_polish_keyword_sets),
            ),
        ),
        (
            "autoPolish",
            json!(bool_at(&parsed, "autoPolish").unwrap_or(false)),
        ),
        (
            "autoPolishFrequency",
            json!(number_or_default(&parsed, "autoPolishFrequency", 5)),
        ),
        (
            "autoCheckUpdates",
            json!(bool_at(&parsed, "autoCheckUpdates").unwrap_or(true)),
        ),
        (
            "logLevel",
            json!(normalize_log_level(parsed.get("logLevel"))),
        ),
        (
            "textReplacementSets",
            parsed
                .get("textReplacementSets")
                .filter(|v| v.is_array())
                .cloned()
                .unwrap_or_else(|| json!([])),
        ),
        (
            "hotwordSets",
            parsed
                .get("hotwordSets")
                .filter(|v| v.is_array())
                .cloned()
                .unwrap_or_else(|| json!([])),
        ),
        (
            "speakerProfiles",
            normalize_speaker_profiles(parsed.get("speakerProfiles")),
        ),
        (
            "hotwords",
            parsed
                .get("hotwords")
                .filter(|v| v.is_array())
                .cloned()
                .unwrap_or_else(|| json!([])),
        ),
        (
            "liveRecordShortcut",
            json!(string_or_default(
                &parsed,
                "liveRecordShortcut",
                "Ctrl + Space"
            )),
        ),
        (
            "voiceTypingEnabled",
            json!(bool_at(&parsed, "voiceTypingEnabled").unwrap_or(false)),
        ),
        (
            "voiceTypingShortcut",
            json!(string_or_default(&parsed, "voiceTypingShortcut", "Alt+V")),
        ),
        (
            "voiceTypingMode",
            json!(string_or_default(&parsed, "voiceTypingMode", "hold")),
        ),
        (
            "speakerSegmentationModelPath",
            json!(string_at(&parsed, "speakerSegmentationModelPath").unwrap_or_default()),
        ),
        (
            "speakerEmbeddingModelPath",
            json!(string_at(&parsed, "speakerEmbeddingModelPath").unwrap_or_default()),
        ),
        (
            "httpServerEnabled",
            json!(bool_at(&parsed, "httpServerEnabled").unwrap_or(false)),
        ),
        (
            "httpServerPort",
            json!(number_or_default(&parsed, "httpServerPort", 14200)),
        ),
        (
            "httpServerHost",
            json!(string_or_default(&parsed, "httpServerHost", "127.0.0.1")),
        ),
        (
            "httpServerApiKey",
            json!(string_or_default(&parsed, "httpServerApiKey", "")),
        ),
    ] {
        set(&mut config, key, value);
    }
    let mut asr_source = config.clone();
    if let Some(asr) = parsed.get("asr") {
        set(&mut asr_source, "asr", asr.clone());
    }
    let asr_config = normalize_asr_config(&asr_source);
    set(&mut config, "asr", asr_config);

    if config
        .get("textReplacementSets")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty)
        && let Some(rules) = parsed.get("textReplacements").and_then(Value::as_array)
        && !rules.is_empty()
    {
        set(
            &mut config,
            "textReplacementSets",
            json!([{
                "id": "default-set",
                "name": default_rule_set_name,
                "enabled": true,
                "ignoreCase": false,
                "rules": map_legacy_text_replacement_rules(rules),
            }]),
        );
    }

    if config
        .get("hotwordSets")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty)
        && let Some(words) = parsed.get("hotwords").and_then(Value::as_array)
        && !words.is_empty()
    {
        let rules = words
            .iter()
            .enumerate()
            .filter_map(|(index, value)| {
                value
                    .as_str()
                    .map(|text| json!({ "id": format!("hw-{index}"), "text": text }))
            })
            .collect::<Vec<_>>();
        set(
            &mut config,
            "hotwordSets",
            json!([{
                "id": "default-hotword-set",
                "name": default_rule_set_name,
                "enabled": true,
                "rules": rules,
            }]),
        );
    }

    sanitize_typed_config_fields(&mut config);
    config
}

fn sanitize_typed_config_fields(config: &mut Value) {
    let Some(config) = config.as_object_mut() else {
        return;
    };
    let defaults = default_config().as_object().cloned().unwrap_or_default();

    for (key, allowed) in [
        (
            "appLanguage",
            &["auto", "en", "zh", "zh-TW", "ja", "ko"][..],
        ),
        ("theme", &["auto", "light", "dark"][..]),
        (
            "font",
            &["system", "serif", "sans", "mono", "arial", "georgia"][..],
        ),
        ("logLevel", &["trace", "debug", "info", "warn", "error"][..]),
        ("projectsViewMode", &["list", "grid", "table"][..]),
        ("voiceTypingMode", &["hold", "toggle"][..]),
        (
            "gpuAcceleration",
            &["auto", "cpu", "cuda", "coreml", "directml"][..],
        ),
    ] {
        repair_field_type(config, &defaults, key, |value| {
            value.as_str().is_some_and(|value| allowed.contains(&value))
        });
    }

    for key in [
        "liveRecordShortcut",
        "microphoneId",
        "systemAudioDeviceId",
        "streamingModelPath",
        "batchModelPath",
        "punctuationModelPath",
        "vadModelPath",
        "speakerSegmentationModelPath",
        "speakerEmbeddingModelPath",
        "modelDownloadMirror",
        "captionFontColor",
        "language",
        "summaryTemplateId",
        "translationLanguage",
        "polishKeywords",
        "polishPresetId",
        "polishContext",
        "polishScenario",
        "voiceTypingShortcut",
        "httpServerHost",
        "httpServerApiKey",
        "httpServerIpWhitelist",
    ] {
        repair_field_type(config, &defaults, key, Value::is_string);
    }

    for key in [
        "minimizeToTrayOnExit",
        "autoCheckUpdates",
        "muteDuringRecording",
        "keepMicrophoneActive",
        "lockWindow",
        "alwaysOnTop",
        "startOnLaunch",
        "enableTimeline",
        "enableITN",
        "batchVadEnabled",
        "summaryEnabled",
        "autoPolish",
        "voiceTypingEnabled",
        "httpServerEnabled",
    ] {
        repair_field_type(config, &defaults, key, Value::is_boolean);
    }

    for key in [
        "configVersion",
        "maxConcurrent",
        "autoPolishFrequency",
        "httpServerPort",
        "httpServerMaxConcurrent",
        "httpServerMaxQueueSize",
        "httpServerMaxStreaming",
        "httpServerMaxUploadSizeMB",
        "httpServerJobTtlMinutes",
    ] {
        repair_field_type(config, &defaults, key, |value| value.as_i64().is_some());
    }

    for key in [
        "microphoneBoost",
        "captionWindowWidth",
        "captionFontSize",
        "captionBackgroundOpacity",
        "vadBufferSize",
        "llmRequestTimeoutSeconds",
    ] {
        repair_field_type(config, &defaults, key, Value::is_number);
    }

    for key in [
        "summaryCustomTemplates",
        "polishCustomPresets",
        "textReplacementSets",
        "hotwordSets",
        "polishKeywordSets",
        "speakerProfiles",
        "hotwords",
        "textReplacements",
    ] {
        repair_field_type(config, &defaults, key, Value::is_array);
    }

    repair_field_type(config, &defaults, "historyAudioRetentionDays", |value| {
        value.is_null() || value.as_i64().is_some()
    });
}

fn repair_field_type(
    config: &mut Map<String, Value>,
    defaults: &Map<String, Value>,
    key: &str,
    is_valid: impl Fn(&Value) -> bool,
) {
    if !config.get(key).is_some_and(|value| !is_valid(value)) {
        return;
    }
    if let Some(default) = defaults.get(key) {
        config.insert(key.to_string(), default.clone());
    } else {
        config.remove(key);
    }
}

fn merge_default(source: Value) -> Value {
    let mut base = as_object_clone(&default_config()).unwrap_or_default();
    if let Some(source_object) = source.as_object() {
        for (key, value) in source_object {
            base.insert(key.clone(), value.clone());
        }
    }
    Value::Object(base)
}

fn is_online_asr_engine(engine: Option<&str>) -> bool {
    matches!(engine, Some("online") | Some("volcengine-doubao"))
}

fn normalize_asr_providers(providers: Option<&Value>) -> Value {
    let mut online = Map::new();
    let online_providers_val = providers.and_then(|v| v.get("online"));

    for provider in online_asr_providers() {
        let existing = online_providers_val.and_then(|v| v.get(&provider.id));

        let existing = if provider.id == VOLCENGINE_DOUBAO_PROVIDER_ID && existing.is_none() {
            providers.and_then(|v| v.get(VOLCENGINE_DOUBAO_LEGACY_PROVIDER_KEY))
        } else {
            existing
        };

        let mut norm = match provider.defaults.as_object() {
            Some(obj) => obj.clone(),
            None => {
                eprintln!(
                    "[ConfigMigration] Provider defaults is not a JSON object: {}",
                    provider.id
                );
                continue;
            }
        };
        if let Some(existing_obj) = existing.and_then(Value::as_object) {
            for (k, default_v) in &norm.clone() {
                if let Some(existing_v) = existing_obj.get(k)
                    && std::mem::discriminant(existing_v) == std::mem::discriminant(default_v)
                {
                    if let (Some(ext_s), Some(def_s)) = (existing_v.as_str(), default_v.as_str()) {
                        let ext_s = ext_s.trim();
                        if ext_s.is_empty() && !def_s.is_empty() {
                            continue;
                        }
                        if provider.id == VOLCENGINE_DOUBAO_PROVIDER_ID {
                            if k == "batchEndpoint"
                                && (ext_s.contains("idle/submit") || ext_s.ends_with("/submit"))
                            {
                                continue;
                            }
                            if k == "batchResourceId"
                                && (ext_s == "volc.bigasr.auc_idle" || ext_s == "volc.seedasr.auc")
                            {
                                continue;
                            }
                        }
                        norm.insert(k.clone(), Value::String(ext_s.to_string()));
                    } else {
                        norm.insert(k.clone(), existing_v.clone());
                    }
                }
            }
        }
        online.insert(provider.id.clone(), Value::Object(norm));
    }

    json!({
        "online": online
    })
}

fn has_valid_asr_config(config: &Value) -> bool {
    let Some(selections) = config
        .get("asr")
        .and_then(|value| value.get("selections"))
        .and_then(Value::as_object)
    else {
        return false;
    };

    for (slot, expected_mode) in [
        ("live", "streaming"),
        ("caption", "streaming"),
        ("voiceTyping", "streaming"),
        ("batch", "batch"),
    ] {
        let Some(selection) = selections.get(slot).and_then(Value::as_object) else {
            return false;
        };
        let engine = selection.get("engine").and_then(Value::as_str);
        if !matches!(engine, Some("local-sherpa") | Some("online")) {
            return false;
        }
        if selection.get("mode").and_then(Value::as_str) != Some(expected_mode) {
            return false;
        }
        if !selection.get("modelPath").is_some_and(Value::is_string) {
            return false;
        }
        if engine == Some("online") {
            let provider_id = selection.get("providerId").and_then(Value::as_str);
            if provider_id.is_none()
                || !online_asr_providers()
                    .iter()
                    .any(|p| Some(p.id.as_str()) == provider_id)
            {
                return false;
            }
            if !selection.get("profileId").is_some_and(Value::is_string) {
                return false;
            }
        }
    }

    if config
        .get("asr")
        .and_then(|v| v.get("providers"))
        .and_then(Value::as_object)
        .is_none()
    {
        return false;
    }

    true
}

fn has_valid_or_legacy_current_asr_config(config: &Value) -> bool {
    if has_valid_asr_config(config) {
        return true;
    }
    let Some(asr) = config.get("asr") else {
        return false;
    };
    let Some(selections) = asr.get("selections") else {
        return false;
    };
    for (slot, expected_mode) in [
        ("live", "streaming"),
        ("caption", "streaming"),
        ("voiceTyping", "streaming"),
        ("batch", "batch"),
    ] {
        let Some(selection) = selections.get(slot).and_then(Value::as_object) else {
            return false;
        };
        if selection.get("engine").and_then(Value::as_str) != Some("local-sherpa") {
            return false;
        }
        if selection.get("mode").and_then(Value::as_str) != Some(expected_mode) {
            return false;
        }
        if !selection.get("modelPath").is_some_and(Value::is_string) {
            return false;
        }
    }
    asr.get("providers").is_none()
}

fn normalize_asr_selection(
    existing: Option<&Value>,
    expected_mode: &str,
    fallback_model_path: String,
) -> Value {
    if is_online_asr_engine(
        existing
            .and_then(|value| value.get("engine"))
            .and_then(Value::as_str),
    ) {
        let provider_id = existing
            .and_then(|value| value.get("providerId"))
            .and_then(non_empty_str)
            .filter(|value| online_asr_providers().iter().any(|p| p.id == *value))
            .unwrap_or(VOLCENGINE_DOUBAO_PROVIDER_ID);
        let profile_id = existing
            .and_then(|value| value.get("profileId"))
            .and_then(non_empty_str)
            .map(str::to_string)
            .unwrap_or_else(|| {
                online_asr_providers()
                    .iter()
                    .find(|p| p.id == provider_id)
                    .map(|p| p.profile_id.clone())
                    .unwrap_or_default()
            });
        return json!({
            "engine": "online",
            "mode": expected_mode,
            "modelId": Value::Null,
            "modelPath": "",
            "providerId": provider_id,
            "profileId": profile_id,
        });
    }

    let model_id = existing
        .and_then(|value| value.get("modelId"))
        .filter(|value| value.is_string() || value.is_null())
        .cloned()
        .unwrap_or(Value::Null);
    let model_path = existing
        .and_then(|value| value.get("modelPath"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .unwrap_or(fallback_model_path);

    json!({
        "engine": "local-sherpa",
        "mode": expected_mode,
        "modelId": model_id,
        "modelPath": model_path,
    })
}

fn normalize_asr_config(config: &Value) -> Value {
    let streaming_model_path = string_at(config, "streamingModelPath").unwrap_or_default();
    let batch_model_path = string_at(config, "batchModelPath")
        .or_else(|| string_at(config, "offlineModelPath"))
        .unwrap_or_default();
    let selections = config.get("asr").and_then(|value| value.get("selections"));
    let providers = config.get("asr").and_then(|value| value.get("providers"));

    let existing_selection = |slot: &str| selections.and_then(|value| value.get(slot));

    json!({
        "selections": {
            "live": normalize_asr_selection(
                existing_selection("live"),
                "streaming",
                streaming_model_path.clone(),
            ),
            "caption": normalize_asr_selection(
                existing_selection("caption"),
                "streaming",
                streaming_model_path.clone(),
            ),
            "voiceTyping": normalize_asr_selection(
                existing_selection("voiceTyping"),
                "streaming",
                streaming_model_path,
            ),
            "batch": normalize_asr_selection(
                existing_selection("batch"),
                "batch",
                batch_model_path,
            ),
        },
        "providers": normalize_asr_providers(providers),
    })
}

fn normalize_log_level(value: Option<&Value>) -> &'static str {
    match value.and_then(Value::as_str) {
        Some("trace") => "trace",
        Some("debug") => "debug",
        Some("info") => "info",
        Some("warn") => "warn",
        Some("error") => "error",
        _ => "info",
    }
}

fn normalize_summary_custom_templates(value: Option<&Value>) -> Value {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    let Some(templates) = value.and_then(Value::as_array) else {
        return json!([]);
    };

    for (index, template) in templates.iter().enumerate() {
        let Some(object) = template.as_object() else {
            continue;
        };
        let id = object
            .get("id")
            .and_then(non_empty_str)
            .map(str::to_string)
            .unwrap_or_else(|| {
                format!(
                    "summary-template-{}",
                    hash_string(&format!(
                        "{}-{}-{index}",
                        object
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or_default(),
                        object
                            .get("instructions")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                    ))
                )
            });
        if !seen.insert(id.clone()) {
            continue;
        }
        let name = object
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim();
        let instructions = object
            .get("instructions")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim();
        if name.is_empty() || instructions.is_empty() {
            continue;
        }
        normalized.push(json!({ "id": id, "name": name, "instructions": instructions }));
    }

    Value::Array(normalized)
}

fn coerce_summary_template_id(
    template_id: Option<&str>,
    custom_templates: Option<&Value>,
) -> String {
    if template_id.is_some_and(|id| BUILTIN_SUMMARY_TEMPLATE_IDS.contains(&id)) {
        return template_id.unwrap().to_string();
    }
    if let Some(custom_id) = template_id
        && custom_templates
            .and_then(Value::as_array)
            .is_some_and(|templates| {
                templates
                    .iter()
                    .any(|template| template.get("id").and_then(Value::as_str) == Some(custom_id))
            })
    {
        return custom_id.to_string();
    }
    DEFAULT_SUMMARY_TEMPLATE_ID.to_string()
}

fn normalize_polish_custom_presets(value: Option<&Value>) -> Value {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    let Some(presets) = value.and_then(Value::as_array) else {
        return json!([]);
    };

    for (index, preset) in presets.iter().enumerate() {
        let Some(object) = preset.as_object() else {
            continue;
        };
        let id = object
            .get("id")
            .and_then(non_empty_str)
            .map(str::to_string)
            .unwrap_or_else(|| {
                format!(
                    "custom-{}",
                    hash_string(&format!(
                        "{}-{}-{index}",
                        object
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or_default(),
                        object
                            .get("context")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                    ))
                )
            });
        if !seen.insert(id.clone()) {
            continue;
        }
        let context = object
            .get("context")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim();
        if context.is_empty() {
            continue;
        }
        let name = object
            .get("name")
            .and_then(non_empty_str)
            .map(str::to_string)
            .unwrap_or_else(|| build_imported_preset_name(None, context));
        normalized.push(json!({ "id": id, "name": name, "context": context }));
    }

    Value::Array(normalized)
}

fn coerce_polish_preset_id(preset_id: Option<&str>, custom_presets: Option<&Value>) -> String {
    if preset_id.is_some_and(|id| BUILTIN_POLISH_PRESET_IDS.contains(&id)) {
        return preset_id.unwrap().to_string();
    }
    if let Some(custom_id) = preset_id
        && custom_presets
            .and_then(Value::as_array)
            .is_some_and(|presets| {
                presets
                    .iter()
                    .any(|preset| preset.get("id").and_then(Value::as_str) == Some(custom_id))
            })
    {
        return custom_id.to_string();
    }
    DEFAULT_POLISH_PRESET_ID.to_string()
}

fn migrate_legacy_polish_selection(
    preset_id: Option<&str>,
    scenario: Option<&str>,
    context: Option<&str>,
    existing_custom_presets: &Value,
    preferred_name: Option<&str>,
) -> (String, Value) {
    let custom_presets = normalize_polish_custom_presets(Some(existing_custom_presets));
    let coerced = coerce_polish_preset_id(preset_id, Some(&custom_presets));
    if preset_id.is_some_and(|id| id == coerced) {
        return (coerced, custom_presets);
    }
    if scenario
        .is_some_and(|value| BUILTIN_POLISH_PRESET_IDS.contains(&value) && value != "general")
    {
        return (scenario.unwrap().to_string(), custom_presets);
    }
    let context = context.unwrap_or_default().trim();
    if context.is_empty() {
        return (DEFAULT_POLISH_PRESET_ID.to_string(), custom_presets);
    }
    ensure_polish_custom_preset(&custom_presets, context, preferred_name)
}

fn ensure_polish_custom_preset(
    existing_custom_presets: &Value,
    context: &str,
    preferred_name: Option<&str>,
) -> (String, Value) {
    let normalized_context = context.trim();
    let mut custom_presets = normalize_polish_custom_presets(Some(existing_custom_presets))
        .as_array()
        .cloned()
        .unwrap_or_default();
    if let Some(existing) = custom_presets.iter().find(|preset| {
        preset
            .get("context")
            .and_then(Value::as_str)
            .is_some_and(|value| value.trim().eq_ignore_ascii_case(normalized_context))
    }) {
        return (
            existing
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or(DEFAULT_POLISH_PRESET_ID)
                .to_string(),
            Value::Array(custom_presets),
        );
    }

    let preset_id = format!("custom-{}", hash_string(normalized_context));
    custom_presets.push(json!({
        "id": preset_id,
        "name": build_imported_preset_name(preferred_name, normalized_context),
        "context": normalized_context,
    }));
    (preset_id, Value::Array(custom_presets))
}

fn build_imported_preset_name(preferred_name: Option<&str>, context: &str) -> String {
    let base = preferred_name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Imported Preset");
    format!("{} ({})", base, &hash_string(context)[0..6])
}

fn normalize_polish_keyword_sets(value: Option<&Value>) -> Value {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    let Some(sets) = value.and_then(Value::as_array) else {
        return json!([]);
    };

    for (index, set_value) in sets.iter().enumerate() {
        let Some(object) = set_value.as_object() else {
            continue;
        };
        let keywords = object
            .get("keywords")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let name = object
            .get("name")
            .and_then(non_empty_str)
            .map(str::to_string)
            .unwrap_or_else(|| build_fallback_polish_keyword_set_name(keywords, index));
        let id = object
            .get("id")
            .and_then(non_empty_str)
            .map(str::to_string)
            .unwrap_or_else(|| {
                format!(
                    "polish-keywords-{}",
                    hash_string(&format!("{name}-{keywords}-{index}"))
                )
            });
        if !seen.insert(id.clone()) {
            continue;
        }
        normalized.push(json!({
            "id": id,
            "name": name,
            "enabled": object.get("enabled").and_then(Value::as_bool).unwrap_or(true),
            "keywords": keywords,
        }));
    }

    Value::Array(normalized)
}

fn migrate_legacy_polish_keywords(
    legacy_keywords: Option<&str>,
    existing_sets: Option<&Value>,
) -> Value {
    let normalized_sets = normalize_polish_keyword_sets(existing_sets);
    let normalized_keywords = legacy_keywords.unwrap_or_default().trim();
    if normalized_keywords.is_empty() {
        return normalized_sets;
    }

    let mut sets = normalized_sets.as_array().cloned().unwrap_or_default();
    if let Some(existing_index) = sets.iter().position(|set| {
        set.get("keywords")
            .and_then(Value::as_str)
            .is_some_and(|keywords| keywords.trim() == normalized_keywords)
    }) {
        if sets[existing_index].get("enabled").and_then(Value::as_bool) == Some(true) {
            return Value::Array(sets);
        }
        if let Some(object) = sets[existing_index].as_object_mut() {
            object.insert("enabled".to_string(), json!(true));
        }
        return Value::Array(sets);
    }

    sets.push(json!({
        "id": format!("polish-keywords-{}", hash_string(normalized_keywords)),
        "name": format!("Imported Keywords ({})", &hash_string(normalized_keywords)[0..6]),
        "enabled": true,
        "keywords": normalized_keywords,
    }));
    Value::Array(sets)
}

fn build_fallback_polish_keyword_set_name(keywords: &str, index: usize) -> String {
    let trimmed = keywords.trim();
    if trimmed.is_empty() {
        format!("Untitled Keywords {}", index + 1)
    } else {
        format!("Imported Keywords ({})", &hash_string(trimmed)[0..6])
    }
}

fn normalize_speaker_profiles(value: Option<&Value>) -> Value {
    let profiles = value
        .and_then(Value::as_array)
        .map(|profiles| {
            profiles
                .iter()
                .filter_map(normalize_speaker_profile)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Value::Array(profiles)
}

fn normalize_speaker_profile(value: &Value) -> Option<Value> {
    let object = value.as_object()?;
    let id = object.get("id").and_then(non_empty_str)?.to_string();
    let samples = object
        .get("samples")
        .and_then(Value::as_array)
        .map(|samples| {
            samples
                .iter()
                .filter_map(normalize_speaker_profile_sample)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Some(json!({
        "id": id,
        "name": object.get("name").and_then(non_empty_str).unwrap_or("Speaker Profile"),
        "enabled": object.get("enabled").and_then(Value::as_bool).unwrap_or(true),
        "samples": samples,
    }))
}

fn normalize_speaker_profile_sample(value: &Value) -> Option<Value> {
    let object = value.as_object()?;
    let id = object.get("id").and_then(non_empty_str)?.to_string();
    let file_path = object.get("filePath").and_then(non_empty_str)?.to_string();
    Some(json!({
        "id": id,
        "filePath": file_path,
        "sourceName": object.get("sourceName").and_then(non_empty_str).unwrap_or("Sample"),
        "durationSeconds": object.get("durationSeconds").and_then(Value::as_f64).filter(|value| value.is_finite()).unwrap_or(0.0).max(0.0),
    }))
}

fn map_legacy_text_replacement_rules(rules: &[Value]) -> Vec<Value> {
    rules
        .iter()
        .map(|rule| {
            json!({
                "id": rule.get("id").cloned().unwrap_or(Value::Null),
                "from": rule.get("from").cloned().unwrap_or(Value::Null),
                "to": rule.get("to").cloned().unwrap_or(Value::Null),
            })
        })
        .collect()
}

fn ensure_llm_state(source: &Value) -> Value {
    let mut custom_providers =
        normalize_stored_custom_providers(source.pointer("/llmSettings/customProviders"));
    if needs_legacy_openai_compatible_provider(source) {
        custom_providers.insert(
            LEGACY_OPENAI_COMPATIBLE_PROVIDER.to_string(),
            json!({
                "id": LEGACY_OPENAI_COMPATIBLE_PROVIDER,
                "name": "OpenAI Compatible",
                "strategy": "openai_compatible",
                "createdAt": LEGACY_OPENAI_COMPATIBLE_CREATED_AT,
            }),
        );
    }

    let current_provider = normalize_provider(
        source
            .pointer("/llmSettings/activeProvider")
            .or_else(|| source.get("llmServiceType"))
            .or_else(|| source.pointer("/llm/provider")),
    );
    let mut providers = normalize_stored_providers(
        source.pointer("/llmSettings/providers"),
        Some(&custom_providers),
    );

    if let Some(llm) = source.get("llm").and_then(Value::as_object) {
        let provider = normalize_provider(llm.get("provider"));
        providers.insert(
            provider.clone(),
            sanitize_provider_setting(
                &provider,
                Some(provider_setting_from_values(
                    llm.get("baseUrl").and_then(trimmed_string),
                    llm.get("apiKey").and_then(trimmed_string),
                    llm.get("apiPath").and_then(trimmed_string),
                    llm.get("apiVersion").and_then(trimmed_string),
                )),
                Some(&custom_providers),
            ),
        );
    } else {
        let legacy_setting = provider_setting_from_values(
            first_trimmed(source, &["llmBaseUrl", "aiBaseUrl", "baseUrl"]),
            first_trimmed(source, &["llmApiKey", "aiApiKey", "apiKey"]),
            first_trimmed(source, &["llmApiPath", "aiApiPath", "apiPath"]),
            first_trimmed(source, &["llmApiVersion", "aiApiVersion", "apiVersion"]),
        );
        if legacy_setting.as_object().is_some_and(|setting| {
            setting
                .values()
                .any(|value| value.as_str().is_some_and(|text| !text.is_empty()))
        }) {
            let current = providers.get(&current_provider).cloned();
            providers.insert(
                current_provider.clone(),
                sanitize_provider_setting(
                    &current_provider,
                    merge_object_values(legacy_setting, current),
                    Some(&custom_providers),
                ),
            );
        }
    }

    providers.insert(
        current_provider.clone(),
        sanitize_provider_setting(
            &current_provider,
            providers.get(&current_provider).cloned(),
            Some(&custom_providers),
        ),
    );

    let models = normalize_stored_models(source.pointer("/llmSettings/models"));
    let model_order =
        normalize_stored_model_order(source.pointer("/llmSettings/modelOrder"), &models);
    let model_discovery =
        normalize_stored_model_discovery(source.pointer("/llmSettings/modelDiscovery"));
    let selections =
        normalize_stored_selections(source.pointer("/llmSettings/selections"), &models);

    let mut settings = json!({
        "activeProvider": current_provider,
        "customProviders": custom_providers,
        "providers": providers,
        "models": models,
        "modelOrder": model_order,
        "modelDiscovery": model_discovery,
        "selections": selections,
    });

    settings = bootstrap_missing_model_selections(settings, resolve_legacy_bootstrap_model(source));
    settings = apply_legacy_temperature(settings, source.pointer("/llm/temperature"));
    ensure_summary_model_selection(settings)
}

fn normalize_provider(value: Option<&Value>) -> String {
    let Some(provider) = value.and_then(Value::as_str) else {
        return DEFAULT_LLM_PROVIDER.to_string();
    };
    normalize_provider_str(provider)
}

fn normalize_provider_str(provider: &str) -> String {
    if is_custom_provider_id(provider) {
        return provider.to_string();
    }
    if provider == "google_translate" || provider == "google_translate_free" {
        return provider.to_string();
    }
    if provider == "openai_compatible" || provider == "open_ai_compatible" {
        return LEGACY_OPENAI_COMPATIBLE_PROVIDER.to_string();
    }

    if let Some(llm_provider) = crate::llm::providers::find_llm_provider_by_id_or_alias(provider) {
        return llm_provider.id.clone();
    }

    DEFAULT_LLM_PROVIDER.to_string()
}

fn is_custom_provider_id(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("custom-") else {
        return false;
    };
    !rest.is_empty()
        && rest
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
        && !rest.starts_with('-')
        && !rest.ends_with('-')
}

fn provider_strategy_from_custom_providers<'a>(
    provider: &str,
    custom_providers: Option<&'a Map<String, Value>>,
) -> Option<&'a str> {
    custom_providers?
        .get(provider)?
        .get("strategy")
        .and_then(Value::as_str)
}

fn provider_defaults(
    provider: &str,
    custom_providers: Option<&Map<String, Value>>,
) -> Map<String, Value> {
    if is_custom_provider_id(provider) {
        let api_path = match provider_strategy_from_custom_providers(provider, custom_providers) {
            Some("openai_responses") => Some("/v1/responses"),
            Some("anthropic") | Some("gemini") => None,
            _ => Some("/v1/chat/completions"),
        };
        let mut defaults = Map::new();
        defaults.insert("apiHost".to_string(), json!(""));
        defaults.insert("apiKey".to_string(), json!(""));
        if let Some(api_path) = api_path {
            defaults.insert("apiPath".to_string(), json!(api_path));
        }
        return defaults;
    }

    if provider == "google_translate_free" {
        let mut defaults = Map::new();
        defaults.insert(
            "apiHost".to_string(),
            json!("https://translate.googleapis.com/translate_a/single"),
        );
        defaults.insert("apiKey".to_string(), json!(""));
        return defaults;
    }

    if provider == "google_translate" {
        let mut defaults = Map::new();
        defaults.insert(
            "apiHost".to_string(),
            json!("https://translation.googleapis.com/language/translate/v2"),
        );
        defaults.insert("apiKey".to_string(), json!(""));
        return defaults;
    }

    if let Some(llm_provider) = crate::llm::providers::find_llm_provider_by_id_or_alias(provider) {
        let mut defaults = Map::new();
        defaults.insert("apiHost".to_string(), json!(llm_provider.defaults.api_host));
        defaults.insert("apiKey".to_string(), json!(""));
        if let Some(api_path) = &llm_provider.defaults.api_path {
            defaults.insert("apiPath".to_string(), json!(api_path));
        }
        if let Some(api_version) = &llm_provider.defaults.api_version {
            defaults.insert("apiVersion".to_string(), json!(api_version));
        }
        return defaults;
    }

    let mut defaults = Map::new();
    defaults.insert("apiHost".to_string(), json!(""));
    defaults.insert("apiKey".to_string(), json!(""));
    defaults
}

fn sanitize_provider_setting(
    provider: &str,
    setting: Option<Value>,
    custom_providers: Option<&Map<String, Value>>,
) -> Value {
    let defaults = provider_defaults(provider, custom_providers);

    // Google Translate Free is a fixed built-in endpoint. Persisted values are
    // ignored so a stale custom gateway URL (e.g. https://api2.apiaqi.com)
    // cannot break the free translation path.
    if provider == "google_translate_free" {
        return Value::Object(defaults);
    }

    let mut merged = defaults;
    if let Some(setting_object) = setting.and_then(|value| value.as_object().cloned()) {
        for key in ["apiHost", "apiKey", "apiPath", "apiVersion"] {
            if let Some(value) = setting_object.get(key) {
                merged.insert(key.to_string(), value.clone());
            }
        }
    }
    Value::Object(merged)
}

fn normalize_stored_custom_providers(value: Option<&Value>) -> Map<String, Value> {
    let mut custom_providers = Map::new();
    let Some(object) = value.and_then(Value::as_object) else {
        return custom_providers;
    };

    for (raw_provider, raw_setting) in object {
        let Some(setting) = raw_setting.as_object() else {
            continue;
        };
        let provider = setting
            .get("id")
            .and_then(Value::as_str)
            .map(normalize_provider_str)
            .unwrap_or_else(|| normalize_provider_str(raw_provider));
        if !is_custom_provider_id(&provider) {
            continue;
        }
        let Some(strategy) = setting
            .get("strategy")
            .and_then(Value::as_str)
            .filter(|strategy| {
                matches!(
                    *strategy,
                    "openai_compatible" | "openai_responses" | "anthropic" | "gemini"
                )
            })
        else {
            continue;
        };
        let name = setting
            .get("name")
            .and_then(non_empty_str)
            .unwrap_or(&provider);
        let created_at = setting
            .get("createdAt")
            .and_then(non_empty_str)
            .unwrap_or(LEGACY_OPENAI_COMPATIBLE_CREATED_AT);
        custom_providers.insert(
            provider.clone(),
            json!({
                "id": provider,
                "name": name,
                "strategy": strategy,
                "createdAt": created_at,
            }),
        );
    }

    custom_providers
}

fn needs_legacy_openai_compatible_provider(source: &Value) -> bool {
    let is_legacy_provider = |value: Option<&Value>| {
        matches!(
            value.and_then(Value::as_str),
            Some("open_ai_compatible" | "openai_compatible")
        )
    };

    if is_legacy_provider(source.pointer("/llmSettings/activeProvider"))
        || is_legacy_provider(source.pointer("/llm/provider"))
        || is_legacy_provider(source.get("llmServiceType"))
    {
        return true;
    }

    if source
        .pointer("/llmSettings/providers")
        .and_then(Value::as_object)
        .is_some_and(|providers| {
            providers
                .keys()
                .any(|provider| provider == "open_ai_compatible" || provider == "openai_compatible")
        })
    {
        return true;
    }

    source
        .pointer("/llmSettings/models")
        .and_then(Value::as_object)
        .is_some_and(|models| {
            models.values().any(|entry| {
                matches!(
                    entry.get("provider").and_then(Value::as_str),
                    Some("open_ai_compatible" | "openai_compatible")
                )
            })
        })
}

fn normalize_stored_providers(
    value: Option<&Value>,
    custom_providers: Option<&Map<String, Value>>,
) -> Map<String, Value> {
    let mut providers = Map::new();
    if let Some(object) = value.and_then(Value::as_object) {
        for (raw_provider, raw_setting) in object {
            let provider = normalize_provider(Some(&Value::String(raw_provider.clone())));
            providers.insert(
                provider.clone(),
                sanitize_provider_setting(&provider, Some(raw_setting.clone()), custom_providers),
            );
        }
    }
    providers
}

fn normalize_stored_models(value: Option<&Value>) -> Map<String, Value> {
    let mut models = Map::new();
    let Some(object) = value.and_then(Value::as_object) else {
        return models;
    };
    for raw_entry in object.values() {
        let Some(entry) = raw_entry.as_object() else {
            continue;
        };
        let Some(id) = entry.get("id").and_then(non_empty_str) else {
            continue;
        };
        let Some(model) = entry.get("model").and_then(non_empty_str) else {
            continue;
        };
        let provider = normalize_provider(entry.get("provider"));
        let mut normalized = Map::new();
        normalized.insert("id".to_string(), json!(id));
        normalized.insert("provider".to_string(), json!(provider));
        normalized.insert("model".to_string(), json!(model));
        if let Some(source) = entry
            .get("source")
            .and_then(Value::as_str)
            .filter(|source| matches!(*source, "manual" | "discovered"))
        {
            normalized.insert("source".to_string(), json!(source));
        }
        if let Some(metadata) = normalize_model_metadata(entry.get("metadata")) {
            normalized.insert("metadata".to_string(), metadata);
        }
        if let Some(overrides) = normalize_model_metadata_overrides(entry.get("metadataOverrides"))
        {
            normalized.insert("metadataOverrides".to_string(), overrides);
        }
        models.insert(id.to_string(), Value::Object(normalized));
    }
    models
}

const MODEL_METADATA_KEYS: [&str; 17] = [
    "displayName",
    "inputPrice",
    "outputPrice",
    "cacheReadPrice",
    "cacheWritePrice",
    "contextWindow",
    "maxOutputTokens",
    "knowledgeCutoff",
    "releaseDate",
    "lastUpdated",
    "inputModalities",
    "outputModalities",
    "supportsMultimodal",
    "supportsTools",
    "supportsReasoning",
    "supportsStructuredOutput",
    "supportsPromptCaching",
];

fn normalize_model_metadata(value: Option<&Value>) -> Option<Value> {
    let source = value?.as_object()?;
    let mut metadata = Map::new();
    for key in [
        "displayName",
        "knowledgeCutoff",
        "releaseDate",
        "lastUpdated",
    ] {
        if let Some(value) = source.get(key).and_then(non_empty_str) {
            metadata.insert(key.to_string(), json!(value));
        }
    }
    for key in [
        "inputPrice",
        "outputPrice",
        "cacheReadPrice",
        "cacheWritePrice",
    ] {
        if let Some(value) = source
            .get(key)
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite() && *value >= 0.0)
        {
            metadata.insert(key.to_string(), json!(value));
        }
    }
    for key in ["contextWindow", "maxOutputTokens"] {
        if let Some(value) = source.get(key).and_then(Value::as_u64) {
            metadata.insert(key.to_string(), json!(value));
        }
    }
    for key in [
        "supportsMultimodal",
        "supportsTools",
        "supportsReasoning",
        "supportsStructuredOutput",
        "supportsPromptCaching",
    ] {
        if let Some(value) = source.get(key).and_then(Value::as_bool) {
            metadata.insert(key.to_string(), json!(value));
        }
    }
    for key in ["inputModalities", "outputModalities"] {
        if let Some(values) =
            normalize_string_values(source.get(key), &["text", "image", "audio", "video", "pdf"])
        {
            metadata.insert(key.to_string(), values);
        }
    }
    if let Some(values) =
        normalize_string_values(source.get("metadataSources"), &["provider", "models_dev"])
    {
        metadata.insert("metadataSources".to_string(), values);
    }
    (!metadata.is_empty()).then_some(Value::Object(metadata))
}

fn normalize_model_metadata_overrides(value: Option<&Value>) -> Option<Value> {
    let source = value?.as_object()?;
    let mut overrides = Map::new();
    for key in MODEL_METADATA_KEYS {
        if source.get(key).and_then(Value::as_bool) == Some(true) {
            overrides.insert(key.to_string(), Value::Bool(true));
        }
    }
    (!overrides.is_empty()).then_some(Value::Object(overrides))
}

fn normalize_string_values(value: Option<&Value>, allowed: &[&str]) -> Option<Value> {
    let mut seen = HashSet::new();
    let values = value?
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .filter(|value| allowed.contains(value))
        .filter(|value| seen.insert((*value).to_string()))
        .map(|value| json!(value))
        .collect::<Vec<_>>();
    (!values.is_empty()).then_some(Value::Array(values))
}

fn normalize_stored_model_discovery(value: Option<&Value>) -> Map<String, Value> {
    let mut discovery = Map::new();
    let Some(source) = value.and_then(Value::as_object) else {
        return discovery;
    };
    for (raw_provider, raw_status) in source {
        let Some(status) = raw_status.as_object() else {
            continue;
        };
        let (Some(fetched_at), Some(expires_at)) = (
            status.get("fetchedAt").and_then(non_empty_str),
            status.get("expiresAt").and_then(non_empty_str),
        ) else {
            continue;
        };
        let provider = normalize_provider(Some(&Value::String(raw_provider.clone())));
        discovery.insert(
            provider,
            json!({ "fetchedAt": fetched_at, "expiresAt": expires_at }),
        );
    }
    discovery
}

fn normalize_stored_model_order(value: Option<&Value>, models: &Map<String, Value>) -> Vec<Value> {
    let mut seen = HashSet::new();
    let mut ordered = Vec::new();
    if let Some(order) = value.and_then(Value::as_array) {
        for entry in order {
            if let Some(id) = entry.as_str().filter(|id| models.contains_key(*id))
                && seen.insert(id.to_string())
            {
                ordered.push(json!(id));
            }
        }
    }
    for id in models.keys() {
        if seen.insert(id.clone()) {
            ordered.push(json!(id));
        }
    }
    ordered
}

fn normalize_stored_selections(
    value: Option<&Value>,
    models: &Map<String, Value>,
) -> Map<String, Value> {
    let mut selections = Map::new();
    let Some(object) = value.and_then(Value::as_object) else {
        return selections;
    };
    for key in ["polishModelId", "translationModelId", "summaryModelId"] {
        if let Some(id) = object
            .get(key)
            .and_then(Value::as_str)
            .filter(|id| models.contains_key(*id))
        {
            selections.insert(key.to_string(), json!(id));
        }
    }
    for key in [
        "polishTemperature",
        "translationTemperature",
        "summaryTemperature",
    ] {
        if let Some(value) = normalize_temperature(object.get(key)) {
            selections.insert(key.to_string(), json!(value));
        }
    }
    for key in [
        "polishReasoningEnabled",
        "translationReasoningEnabled",
        "summaryReasoningEnabled",
    ] {
        if let Some(value) = object.get(key).and_then(Value::as_bool) {
            selections.insert(key.to_string(), json!(value));
        }
    }
    for key in [
        "polishReasoningLevel",
        "translationReasoningLevel",
        "summaryReasoningLevel",
    ] {
        if let Some(value) = object
            .get(key)
            .and_then(Value::as_str)
            .filter(|value| matches!(*value, "low" | "medium" | "high"))
        {
            selections.insert(key.to_string(), json!(value));
        }
    }
    selections
}

fn resolve_legacy_bootstrap_model(source: &Value) -> Option<(String, String)> {
    if let Some(providers) = source
        .pointer("/llmSettings/providers")
        .and_then(Value::as_object)
    {
        for (provider, setting) in providers {
            if let Some(model) = setting.get("model").and_then(trimmed_string) {
                return Some((
                    normalize_provider(Some(&Value::String(provider.clone()))),
                    model,
                ));
            }
        }
    }
    let provider = normalize_provider(
        source
            .pointer("/llmSettings/activeProvider")
            .or_else(|| source.pointer("/llm/provider"))
            .or_else(|| source.get("llmServiceType")),
    );
    let model = source
        .pointer("/llm/model")
        .and_then(trimmed_string)
        .or_else(|| first_trimmed(source, &["llmModel", "aiModel", "model"]))?;
    Some((provider, model))
}

fn bootstrap_missing_model_selections(
    mut settings: Value,
    legacy_model: Option<(String, String)>,
) -> Value {
    if settings
        .get("modelOrder")
        .and_then(Value::as_array)
        .is_some_and(|order| !order.is_empty())
    {
        return settings;
    }
    let legacy_model_present = legacy_model.is_some();
    let (provider, model) =
        legacy_model.unwrap_or_else(|| (DEFAULT_LLM_PROVIDER.to_string(), "default".to_string()));
    let model_id = create_model_id(&provider, &model);
    let mut models = Map::new();
    models.insert(
        model_id.clone(),
        json!({ "id": model_id, "provider": provider, "model": model.trim() }),
    );
    set_pointer(&mut settings, "/models", Value::Object(models));
    set_pointer(&mut settings, "/modelOrder", json!([model_id.clone()]));
    set_pointer(
        &mut settings,
        "/selections/translationModelId",
        json!(model_id.clone()),
    );
    if (provider != "google_translate" && provider != "google_translate_free")
        || legacy_model_present
    {
        set_pointer(&mut settings, "/selections/polishModelId", json!(model_id));
    }
    settings
}

fn apply_legacy_temperature(mut settings: Value, legacy_temperature: Option<&Value>) -> Value {
    if let Some(temperature) = normalize_temperature(legacy_temperature) {
        if settings.pointer("/selections/polishTemperature").is_none() {
            set_pointer(
                &mut settings,
                "/selections/polishTemperature",
                json!(temperature),
            );
        }
        if settings
            .pointer("/selections/translationTemperature")
            .is_none()
        {
            set_pointer(
                &mut settings,
                "/selections/translationTemperature",
                json!(temperature),
            );
        }
    }
    settings
}

fn ensure_summary_model_selection(mut settings: Value) -> Value {
    if settings.pointer("/selections/summaryModelId").is_some() {
        return settings;
    }
    let Some(polish_model_id) = settings
        .pointer("/selections/polishModelId")
        .and_then(Value::as_str)
        .map(str::to_string)
    else {
        return settings;
    };
    let Some(provider) = settings
        .pointer(&format!("/models/{polish_model_id}/provider"))
        .and_then(Value::as_str)
        .map(str::to_string)
    else {
        return settings;
    };
    if provider != "google_translate" && provider != "google_translate_free" {
        set_pointer(
            &mut settings,
            "/selections/summaryModelId",
            json!(polish_model_id),
        );
    }
    settings
}

fn set_enabled_by_ids(sets: &Value, enabled_ids: &[String]) -> Value {
    let enabled: HashSet<&str> = enabled_ids.iter().map(String::as_str).collect();
    Value::Array(
        sets.as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .filter_map(|set| {
                let mut object = set.as_object()?.clone();
                let is_enabled = object
                    .get("id")
                    .and_then(Value::as_str)
                    .is_some_and(|id| enabled.contains(id));
                object.insert("enabled".to_string(), json!(is_enabled));
                Some(Value::Object(object))
            })
            .collect(),
    )
}

fn hash_string(value: &str) -> String {
    let mut hash: u32 = 5381;
    for unit in value.encode_utf16() {
        hash = hash.wrapping_shl(5).wrapping_add(hash) ^ u32::from(unit);
    }
    format!("{hash:08x}")
}

fn create_model_id(provider: &str, model: &str) -> String {
    let mut normalized = String::new();
    let mut last_dash = false;
    for ch in model.trim().to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch);
            last_dash = false;
        } else if !last_dash {
            normalized.push('-');
            last_dash = true;
        }
    }
    let normalized = normalized.trim_matches('-');
    if normalized.is_empty() {
        format!("{provider}-model")
    } else {
        format!("{provider}-{normalized}")
    }
}

fn set(config: &mut Value, key: &str, value: Value) {
    if let Some(object) = config.as_object_mut() {
        object.insert(key.to_string(), value);
    }
}

fn set_pointer(value: &mut Value, pointer: &str, next_value: Value) {
    let parts = pointer
        .trim_start_matches('/')
        .split('/')
        .collect::<Vec<_>>();
    if parts.is_empty() {
        *value = next_value;
        return;
    }
    let mut current = value;
    for part in &parts[..parts.len() - 1] {
        if !current.is_object() {
            *current = json!({});
        }
        let object = current.as_object_mut().expect("object just created");
        current = object
            .entry((*part).to_string())
            .or_insert_with(|| json!({}));
    }
    if let Some(object) = current.as_object_mut() {
        object.insert(parts[parts.len() - 1].to_string(), next_value);
    }
}

fn non_empty_str(value: &Value) -> Option<&str> {
    value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn trimmed_string(value: &Value) -> Option<String> {
    non_empty_str(value).map(str::to_string)
}

fn string_at(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

fn string_or_default(value: &Value, key: &str, default: &str) -> String {
    value
        .get(key)
        .and_then(non_empty_str)
        .unwrap_or(default)
        .to_string()
}

fn first_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| string_at(value, key).filter(|text| !text.is_empty()))
}

fn first_trimmed(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(trimmed_string))
}

fn bool_at(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

fn number_or_default(value: &Value, key: &str, default: i64) -> i64 {
    value
        .get(key)
        .and_then(Value::as_i64)
        .filter(|value| *value != 0)
        .unwrap_or(default)
}

fn number_f64_or_default(value: &Value, key: &str, default: f64) -> f64 {
    value.get(key).and_then(Value::as_f64).unwrap_or(default)
}

fn normalize_temperature(value: Option<&Value>) -> Option<f64> {
    value
        .and_then(Value::as_f64)
        .filter(|temperature| *temperature >= 0.0 && *temperature <= 2.0)
}

fn array_strings(value: &Value) -> Option<Vec<String>> {
    Some(
        value
            .as_array()?
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
    )
}

fn as_object_clone(value: &Value) -> Option<Map<String, Value>> {
    value.as_object().cloned()
}

fn provider_setting_from_values(
    api_host: Option<String>,
    api_key: Option<String>,
    api_path: Option<String>,
    api_version: Option<String>,
) -> Value {
    let mut object = Map::new();
    if let Some(value) = api_host {
        object.insert("apiHost".to_string(), json!(value));
    }
    if let Some(value) = api_key {
        object.insert("apiKey".to_string(), json!(value));
    }
    if let Some(value) = api_path {
        object.insert("apiPath".to_string(), json!(value));
    }
    if let Some(value) = api_version {
        object.insert("apiVersion".to_string(), json!(value));
    }
    Value::Object(object)
}

fn merge_object_values(first: Value, second: Option<Value>) -> Option<Value> {
    let mut object = first.as_object().cloned().unwrap_or_default();
    if let Some(second_object) = second.and_then(|value| value.as_object().cloned()) {
        for (key, value) in second_object {
            object.insert(key, value);
        }
    }
    Some(Value::Object(object))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{migrate_app_config, resolve_effective_config};

    #[test]
    fn config_core_migrates_legacy_config_to_current_shape() {
        let legacy = json!({
            "configVersion": 1,
            "modelPath": "models/base",
            "recognitionModelPath": "models/recognition",
            "llmModel": "gpt-4.1-mini",
            "llmServiceType": "openai",
            "logLevel": "verbose",
            "microphoneBoost": 0.0,
            "captionBackgroundOpacity": 0.0,
            "summaryTemplate": "invalid-old-template",
            "polishScenario": "custom",
            "polishContext": "Use concise academic Chinese.",
            "polishKeywords": "Sona\nASR",
            "textReplacements": [
                { "id": "r1", "from": "foo", "to": "bar", "enabled": false }
            ],
            "hotwords": ["Sona", "Tauri"],
            "speakerProfiles": [
                {
                    "id": " speaker-1 ",
                    "name": " Alice ",
                    "samples": [
                        {
                            "id": " sample-1 ",
                            "filePath": " C:/audio.wav ",
                            "sourceName": "",
                            "durationSeconds": -4
                        }
                    ]
                },
                { "name": "Missing id" }
            ]
        });

        let result = migrate_app_config(None, Some(legacy), "Default Rules".to_string());

        assert!(result.migrated);
        assert_eq!(result.config["configVersion"], 7);
        assert_eq!(result.config["streamingModelPath"], "models/recognition");
        assert_eq!(result.config["batchModelPath"], "models/recognition");
        assert_eq!(
            result.config["asr"]["selections"]["live"],
            json!({
                "engine": "local-sherpa",
                "mode": "streaming",
                "modelId": null,
                "modelPath": "models/recognition"
            })
        );
        assert_eq!(
            result.config["asr"]["selections"]["batch"],
            json!({
                "engine": "local-sherpa",
                "mode": "batch",
                "modelId": null,
                "modelPath": "models/recognition"
            })
        );
        assert_eq!(result.config["logLevel"], "info");
        assert_eq!(result.config["keepMicrophoneActive"], false);
        assert_eq!(result.config["microphoneBoost"], 0.0);
        assert_eq!(result.config["captionBackgroundOpacity"], 0.0);
        assert_eq!(result.config["summaryTemplateId"], "general");
        assert_eq!(result.config["polishKeywords"], "");
        assert_eq!(result.config["polishPresetId"], "custom-9158016c");
        assert_eq!(
            result.config["polishCustomPresets"][0],
            json!({
                "id": "custom-9158016c",
                "name": "Imported Preset (915801)",
                "context": "Use concise academic Chinese."
            })
        );
        assert_eq!(
            result.config["polishKeywordSets"][0],
            json!({
                "id": "polish-keywords-d74b61fc",
                "name": "Imported Keywords (d74b61)",
                "enabled": true,
                "keywords": "Sona\nASR"
            })
        );
        assert_eq!(
            result.config["textReplacementSets"][0]["name"],
            "Default Rules"
        );
        assert_eq!(
            result.config["textReplacementSets"][0]["rules"][0],
            json!({ "id": "r1", "from": "foo", "to": "bar" })
        );
        assert_eq!(
            result.config["hotwordSets"][0]["rules"][1],
            json!({ "id": "hw-1", "text": "Tauri" })
        );
        assert_eq!(
            result.config["speakerProfiles"][0],
            json!({
                "id": "speaker-1",
                "name": "Alice",
                "enabled": true,
                "samples": [
                    {
                        "id": "sample-1",
                        "filePath": "C:/audio.wav",
                        "sourceName": "Sample",
                        "durationSeconds": 0.0
                    }
                ]
            })
        );
        assert_eq!(result.config["llmSettings"]["activeProvider"], "open_ai");
        assert_eq!(
            result.config["llmSettings"]["selections"]["summaryModelId"],
            "open_ai-gpt-4-1-mini"
        );
    }

    #[test]
    fn config_core_normalizes_saved_current_config_without_false_migration() {
        let saved = json!({
            "configVersion": 7,
            "asr": {
                "selections": {
                    "live": { "engine": "local-sherpa", "mode": "streaming", "modelId": null, "modelPath": "C:/models/live" },
                    "caption": { "engine": "local-sherpa", "mode": "streaming", "modelId": null, "modelPath": "C:/models/live" },
                    "voiceTyping": { "engine": "local-sherpa", "mode": "streaming", "modelId": null, "modelPath": "C:/models/live" },
                    "batch": { "engine": "local-sherpa", "mode": "batch", "modelId": null, "modelPath": "C:/models/offline" }
                }
            },
            "streamingModelPath": "C:/models/live",
            "offlineModelPath": "C:/models/offline",
            "summaryEnabled": true,
            "summaryTemplateId": "meeting",
            "summaryCustomTemplates": [],
            "polishPresetId": "meeting",
            "polishCustomPresets": [],
            "polishKeywordSets": [],
            "speakerProfiles": [],
            "speakerSegmentationModelPath": "",
            "speakerEmbeddingModelPath": "",
            "logLevel": "debug",
            "llmSettings": {
                "activeProvider": "open_ai",
                "customProviders": {},
                "modelDiscovery": {},
                "providers": {
                    "open_ai": { "apiHost": "https://api.openai.com", "apiKey": "" }
                },
                "models": {
                    "model-1": { "id": "model-1", "provider": "open_ai", "model": "gpt-4.1-mini" }
                },
                "modelOrder": ["model-1"],
                "selections": {
                    "polishModelId": "model-1",
                    "translationModelId": "model-1",
                    "summaryModelId": "model-1"
                }
            },
            "httpServerEnabled": false,
            "httpServerHost": "127.0.0.1",
            "httpServerPort": 14200,
            "httpServerApiKey": "",
            "keepMicrophoneActive": true
        });

        let result = migrate_app_config(Some(saved), None, "Default Rules".to_string());

        assert!(!result.migrated);
        assert_eq!(result.config["configVersion"], 7);
        assert_eq!(
            result.config["asr"]["selections"]["voiceTyping"]["modelPath"],
            "C:/models/live"
        );
        assert_eq!(result.config["summaryTemplateId"], "meeting");
        assert_eq!(result.config["polishPresetId"], "meeting");
        assert_eq!(result.config["logLevel"], "debug");
        assert_eq!(result.config["keepMicrophoneActive"], true);
    }

    #[test]
    fn config_core_upgrades_current_config_without_asr_to_new_asr_shape() {
        let saved = json!({
            "configVersion": 6,
            "streamingModelPath": "C:/models/live",
            "offlineModelPath": "C:/models/offline",
            "summaryEnabled": true,
            "summaryTemplateId": "meeting",
            "summaryCustomTemplates": [],
            "polishPresetId": "meeting",
            "polishCustomPresets": [],
            "polishKeywordSets": [],
            "speakerProfiles": [],
            "speakerSegmentationModelPath": "",
            "speakerEmbeddingModelPath": "",
            "logLevel": "info",
            "llmSettings": {
                "activeProvider": "google_translate_free",
                "providers": {
                    "google_translate_free": {
                        "apiHost": "https://translate.googleapis.com/translate_a/single",
                        "apiKey": ""
                    }
                },
                "models": {},
                "modelOrder": [],
                "selections": {}
            }
        });

        let result = migrate_app_config(Some(saved), None, "Default Rules".to_string());

        assert!(result.migrated);
        assert_eq!(result.config["configVersion"], 7);
        assert_eq!(
            result.config["asr"]["selections"]["caption"]["modelPath"],
            "C:/models/live"
        );
        assert_eq!(
            result.config["asr"]["selections"]["batch"]["modelPath"],
            "C:/models/offline"
        );
        assert_eq!(result.config["streamingModelPath"], "C:/models/live");
        assert_eq!(result.config["offlineModelPath"], "C:/models/offline");
    }

    #[test]
    fn config_core_migrates_volcengine_asr_selection_to_online_provider_shape() {
        let saved = json!({
            "configVersion": 7,
            "asr": {
                "selections": {
                    "live": {
                        "engine": "volcengine-doubao",
                        "mode": "streaming",
                        "modelId": null,
                        "modelPath": "",
                        "providerId": "volcengine-doubao",
                        "profileId": "volcengine-doubao-default"
                    },
                    "caption": {
                        "engine": "volcengine-doubao",
                        "mode": "streaming",
                        "modelId": null,
                        "modelPath": "",
                        "providerId": "volcengine-doubao",
                        "profileId": "volcengine-doubao-default"
                    },
                    "voiceTyping": {
                        "engine": "volcengine-doubao",
                        "mode": "streaming",
                        "modelId": null,
                        "modelPath": "",
                        "providerId": "volcengine-doubao",
                        "profileId": "volcengine-doubao-default"
                    },
                    "batch": {
                        "engine": "volcengine-doubao",
                        "mode": "batch",
                        "modelId": null,
                        "modelPath": "",
                        "providerId": "volcengine-doubao",
                        "profileId": "volcengine-doubao-default"
                    }
                },
                "providers": {
                    "volcengineDoubao": {
                        "apiKey": " volc-test-key ",
                        "streamingEndpoint": "",
                        "streamingResourceId": "",
                        "batchEndpoint": "",
                        "batchResourceId": ""
                    }
                }
            },
            "streamingModelPath": "C:/models/live",
            "offlineModelPath": "C:/models/offline",
            "summaryEnabled": true,
            "summaryTemplateId": "meeting",
            "summaryCustomTemplates": [],
            "polishPresetId": "meeting",
            "polishCustomPresets": [],
            "polishKeywordSets": [],
            "speakerProfiles": [],
            "speakerSegmentationModelPath": "",
            "speakerEmbeddingModelPath": "",
            "logLevel": "info",
            "llmSettings": {
                "activeProvider": "google_translate_free",
                "providers": {
                    "google_translate_free": {
                        "apiHost": "https://translate.googleapis.com/translate_a/single",
                        "apiKey": ""
                    }
                },
                "models": {},
                "modelOrder": [],
                "selections": {}
            }
        });

        let result = migrate_app_config(Some(saved), None, "Default Rules".to_string());

        assert!(result.migrated);
        assert_eq!(
            result.config["asr"]["selections"]["batch"],
            json!({
                "engine": "online",
                "mode": "batch",
                "modelId": null,
                "modelPath": "",
                "providerId": "volcengine-doubao",
                "profileId": "volcengine-doubao-default"
            })
        );
        assert_eq!(
            result.config["asr"]["providers"]["online"]["volcengine-doubao"],
            json!({
                "apiKey": "volc-test-key",
                "streamingEndpoint": "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async",
                "streamingResourceId": "volc.seedasr.sauc.duration",
                "batchEndpoint": "https://openspeech.bytedance.com/api/v3/auc/bigmodel/recognize/flash",
                "batchResourceId": "volc.bigasr.auc_turbo"
            })
        );
        assert_eq!(result.config["streamingModelPath"], "C:/models/live");
        assert_eq!(result.config["offlineModelPath"], "C:/models/offline");
    }

    #[test]
    fn config_core_normalizes_saved_volcengine_async_batch_provider_to_flash() {
        let saved = json!({
            "configVersion": 7,
            "asr": {
                "selections": {
                    "batch": {
                        "engine": "volcengine-doubao",
                        "mode": "batch",
                        "modelId": null,
                        "modelPath": "",
                        "providerId": "volcengine-doubao",
                        "profileId": "volcengine-doubao-default"
                    }
                },
                "providers": {
                    "volcengineDoubao": {
                        "apiKey": "volc-test-key",
                        "streamingEndpoint": "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async",
                        "streamingResourceId": "volc.seedasr.sauc.duration",
                        "batchEndpoint": "https://openspeech.bytedance.com/api/v3/auc/bigmodel/idle/submit",
                        "batchResourceId": "volc.bigasr.auc_idle"
                    }
                }
            }
        });

        let result = migrate_app_config(Some(saved), None, "Default Rules".to_string());

        assert!(result.migrated);
        assert_eq!(
            result.config["asr"]["providers"]["online"]["volcengine-doubao"]["batchEndpoint"],
            "https://openspeech.bytedance.com/api/v3/auc/bigmodel/recognize/flash"
        );
        assert_eq!(
            result.config["asr"]["providers"]["online"]["volcengine-doubao"]["batchResourceId"],
            "volc.bigasr.auc_turbo"
        );
    }

    #[test]
    fn config_core_normalizes_current_null_summary_enabled_to_true() {
        let saved = json!({
            "configVersion": 6,
            "summaryEnabled": null,
            "summaryTemplateId": "meeting",
            "summaryCustomTemplates": [],
            "polishPresetId": "meeting",
            "polishCustomPresets": [],
            "polishKeywordSets": [],
            "speakerProfiles": [],
            "speakerSegmentationModelPath": "",
            "speakerEmbeddingModelPath": "",
            "logLevel": "info",
            "llmSettings": {
                "activeProvider": "google_translate_free",
                "providers": {
                    "google_translate_free": {
                        "apiHost": "https://translate.googleapis.com/translate_a/single",
                        "apiKey": ""
                    }
                },
                "models": {},
                "modelOrder": [],
                "selections": {}
            }
        });

        let result = migrate_app_config(Some(saved), None, "Default Rules".to_string());

        assert!(result.migrated);
        assert_eq!(result.config["summaryEnabled"], true);
    }

    #[test]
    fn config_core_preserves_custom_llm_providers() {
        let saved = json!({
            "configVersion": 6,
            "summaryEnabled": true,
            "summaryTemplateId": "meeting",
            "summaryCustomTemplates": [],
            "polishPresetId": "meeting",
            "polishCustomPresets": [],
            "polishKeywordSets": [],
            "speakerProfiles": [],
            "speakerSegmentationModelPath": "",
            "speakerEmbeddingModelPath": "",
            "logLevel": "debug",
            "llmSettings": {
                "activeProvider": "custom-acme",
                "customProviders": {
                    "custom-acme": {
                        "id": "custom-acme",
                        "name": "Acme Gateway",
                        "strategy": "openai_responses",
                        "createdAt": "2026-05-18T08:00:00.000Z"
                    }
                },
                "providers": {
                    "custom-acme": {
                        "apiHost": "https://gateway.example.com",
                        "apiKey": "test-key"
                    }
                },
                "models": {
                    "model-1": { "id": "model-1", "provider": "custom-acme", "model": "gpt-4o" }
                },
                "modelOrder": ["model-1"],
                "selections": {
                    "polishModelId": "model-1",
                    "translationModelId": "model-1",
                    "summaryModelId": "model-1"
                }
            }
        });

        let result = migrate_app_config(Some(saved), None, "Default Rules".to_string());

        assert!(result.migrated);
        assert_eq!(
            result.config["llmSettings"]["activeProvider"],
            "custom-acme"
        );
        assert_eq!(
            result.config["llmSettings"]["customProviders"]["custom-acme"]["strategy"],
            "openai_responses"
        );
        assert_eq!(
            result.config["llmSettings"]["providers"]["custom-acme"]["apiPath"],
            "/v1/responses"
        );
        assert_eq!(
            result.config["llmSettings"]["models"]["model-1"]["provider"],
            "custom-acme"
        );
    }

    #[test]
    fn llm_state_preserves_runtime_metadata_discovery_and_reasoning() {
        let normalized = ensure_llm_state(&json!({
            "llmSettings": {
                "activeProvider": "open_ai",
                "providers": {
                    "open_ai": { "apiHost": "https://api.openai.com", "apiKey": "key" }
                },
                "models": {
                    "model-1": {
                        "id": "model-1",
                        "provider": "open_ai",
                        "model": "gpt-test",
                        "source": "discovered",
                        "metadata": {
                            "displayName": "GPT Test",
                            "cacheReadPrice": 0.5,
                            "inputModalities": ["text", "image", "invalid"],
                            "supportsStructuredOutput": true,
                            "metadataSources": ["provider", "models_dev"]
                        },
                        "metadataOverrides": {
                            "cacheReadPrice": true,
                            "metadataSources": true,
                            "unknown": true
                        }
                    }
                },
                "modelOrder": ["model-1"],
                "modelDiscovery": {
                    "open_ai": {
                        "fetchedAt": "2026-07-15T00:00:00Z",
                        "expiresAt": "2026-07-16T00:00:00Z"
                    }
                },
                "selections": {
                    "polishModelId": "model-1",
                    "polishReasoningEnabled": true,
                    "polishReasoningLevel": "high"
                }
            }
        }));

        let metadata = &normalized["models"]["model-1"]["metadata"];
        assert_eq!(
            (
                metadata["displayName"].as_str(),
                metadata["inputModalities"].as_array().map(Vec::len),
                metadata["supportsStructuredOutput"].as_bool(),
            ),
            (Some("GPT Test"), Some(2), Some(true))
        );
        assert_eq!(
            (
                normalized["selections"]["polishReasoningEnabled"].as_bool(),
                normalized["selections"]["polishReasoningLevel"].as_str(),
                normalized["modelDiscovery"]["open_ai"]["expiresAt"].as_str(),
            ),
            (Some(true), Some("high"), Some("2026-07-16T00:00:00Z"))
        );
        assert_eq!(
            normalized["models"]["model-1"]["metadataOverrides"],
            json!({ "cacheReadPrice": true })
        );
    }

    #[test]
    fn config_core_migrates_legacy_openai_compatible_to_custom_provider() {
        let saved = json!({
            "configVersion": 6,
            "summaryEnabled": true,
            "summaryTemplateId": "meeting",
            "summaryCustomTemplates": [],
            "polishPresetId": "meeting",
            "polishCustomPresets": [],
            "polishKeywordSets": [],
            "speakerProfiles": [],
            "speakerSegmentationModelPath": "",
            "speakerEmbeddingModelPath": "",
            "logLevel": "info",
            "llmSettings": {
                "activeProvider": "open_ai_compatible",
                "providers": {
                    "open_ai_compatible": {
                        "apiHost": "https://compat.example.com",
                        "apiKey": "compat-key"
                    }
                },
                "models": {
                    "model-1": { "id": "model-1", "provider": "open_ai_compatible", "model": "compat-model" }
                },
                "modelOrder": ["model-1"],
                "selections": {
                    "polishModelId": "model-1",
                    "translationModelId": "model-1",
                    "summaryModelId": "model-1"
                }
            }
        });

        let result = migrate_app_config(Some(saved), None, "Default Rules".to_string());

        assert!(result.migrated);
        assert_eq!(
            result.config["llmSettings"]["activeProvider"],
            "custom-openai-compatible"
        );
        assert_eq!(
            result.config["llmSettings"]["customProviders"]["custom-openai-compatible"],
            json!({
                "id": "custom-openai-compatible",
                "name": "OpenAI Compatible",
                "strategy": "openai_compatible",
                "createdAt": "2026-05-18T00:00:00.000Z"
            })
        );
        assert_eq!(
            result.config["llmSettings"]["providers"]["custom-openai-compatible"]["apiHost"],
            "https://compat.example.com"
        );
        assert_eq!(
            result.config["llmSettings"]["models"]["model-1"]["provider"],
            "custom-openai-compatible"
        );
    }

    #[test]
    fn config_core_resolves_project_effective_config() {
        let global = json!({
            "summaryTemplateId": "missing",
            "summaryCustomTemplates": [
                { "id": "custom-summary", "name": "Custom Summary", "instructions": "Do it." }
            ],
            "translationLanguage": "zh",
            "polishPresetId": "general",
            "textReplacementSets": [
                { "id": "tr-a", "enabled": true, "rules": [] },
                { "id": "tr-b", "enabled": true, "rules": [] }
            ],
            "hotwordSets": [
                { "id": "hw-a", "enabled": true, "rules": [] },
                { "id": "hw-b", "enabled": false, "rules": [] }
            ],
            "polishKeywordSets": [
                { "id": "kw-a", "enabled": true, "keywords": "a" },
                { "id": "kw-b", "enabled": true, "keywords": "b" }
            ],
            "speakerProfiles": [
                { "id": "sp-a", "enabled": true, "samples": [] },
                { "id": "sp-b", "enabled": true, "samples": [] }
            ]
        });
        let project = json!({
            "defaults": {
                "summaryTemplateId": "custom-summary",
                "translationLanguage": "ja",
                "polishPresetId": "meeting",
                "enabledTextReplacementSetIds": ["tr-b"],
                "enabledHotwordSetIds": ["hw-b"],
                "enabledPolishKeywordSetIds": ["kw-a"],
                "enabledSpeakerProfileIds": ["sp-b"]
            }
        });

        let resolved = resolve_effective_config(global, Some(project));

        assert_eq!(resolved["summaryTemplateId"], "custom-summary");
        assert_eq!(resolved["translationLanguage"], "ja");
        assert_eq!(resolved["polishPresetId"], "meeting");
        assert_eq!(resolved["textReplacementSets"][0]["id"], "tr-a");
        assert_eq!(resolved["textReplacementSets"][0]["enabled"], false);
        assert_eq!(resolved["textReplacementSets"][1]["enabled"], true);
        assert_eq!(resolved["hotwordSets"][0]["enabled"], false);
        assert_eq!(resolved["hotwordSets"][1]["enabled"], true);
        assert_eq!(resolved["polishKeywordSets"][0]["enabled"], true);
        assert_eq!(resolved["polishKeywordSets"][1]["enabled"], false);
        assert_eq!(resolved["speakerProfiles"][0]["enabled"], false);
        assert_eq!(resolved["speakerProfiles"][1]["enabled"], true);
    }

    #[test]
    fn config_core_cleans_up_dirty_data_in_online_providers_and_falls_back_to_defaults() {
        let saved = json!({
            "configVersion": 7,
            "asr": {
                "selections": {
                    "live": { "engine": "online", "mode": "streaming", "modelId": null, "modelPath": "", "providerId": "volcengine-doubao", "profileId": "volcengine-doubao-default" },
                    "caption": { "engine": "online", "mode": "streaming", "modelId": null, "modelPath": "", "providerId": "volcengine-doubao", "profileId": "volcengine-doubao-default" },
                    "voiceTyping": { "engine": "online", "mode": "streaming", "modelId": null, "modelPath": "", "providerId": "volcengine-doubao", "profileId": "volcengine-doubao-default" },
                    "batch": { "engine": "online", "mode": "batch", "modelId": null, "modelPath": "", "providerId": "volcengine-doubao", "profileId": "volcengine-doubao-default" }
                },
                "providers": {
                    "online": {
                        "volcengine-doubao": {
                            "apiKey": 12345, // invalid type
                            "streamingEndpoint": "custom-endpoint",
                            "unknownKey": "garbage"
                        },
                        "groq-whisper": {
                            "apiKey": "groq-key"
                        },
                        "garbage-provider": {
                            "apiKey": "should-be-deleted"
                        }
                    }
                }
            }
        });

        let result = migrate_app_config(Some(saved), None, "Default Rules".to_string());

        assert!(result.migrated);

        // Volcengine cleanup: invalid apiKey type falls back to default empty string, unknownKey is removed, valid string is kept
        let volc = &result.config["asr"]["providers"]["online"]["volcengine-doubao"];
        assert_eq!(volc["apiKey"], "");
        assert_eq!(volc["streamingEndpoint"], "custom-endpoint");
        assert!(volc.get("unknownKey").is_none());

        // Groq kept its api key, other defaults populated
        let groq = &result.config["asr"]["providers"]["online"]["groq-whisper"];
        assert_eq!(groq["apiKey"], "groq-key");
        assert_eq!(
            groq["batchEndpoint"],
            "https://api.groq.com/openai/v1/audio/transcriptions"
        );

        // Unknown providers are garbage collected
        assert!(
            result.config["asr"]["providers"]["online"]
                .get("garbage-provider")
                .is_none()
        );

        // Mistral should be fully hydrated from defaults
        let mistral = &result.config["asr"]["providers"]["online"]["mistral-voxtral"];
        assert_eq!(mistral["apiKey"], "");
        assert_eq!(
            mistral["batchEndpoint"],
            "https://api.mistral.ai/v1/audio/transcriptions"
        );
    }
}
