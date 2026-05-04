use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::HashSet;

const CURRENT_CONFIG_VERSION: i64 = 6;
const DEFAULT_POLISH_PRESET_ID: &str = "general";
const DEFAULT_SUMMARY_TEMPLATE_ID: &str = "general";
const DEFAULT_LLM_PROVIDER: &str = "google_translate_free";

const BUILTIN_POLISH_PRESET_IDS: [&str; 6] = [
    "general",
    "customer_service",
    "meeting",
    "interview",
    "lecture",
    "podcast",
];
const BUILTIN_SUMMARY_TEMPLATE_IDS: [&str; 3] = ["general", "meeting", "lecture"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MigrationResult {
    pub config: Value,
    pub migrated: bool,
}

#[tauri::command(rename_all = "camelCase")]
pub fn migrate_app_config(
    saved_config: Option<Value>,
    legacy_config: Option<Value>,
    default_rule_set_name: String,
) -> MigrationResult {
    migrate_app_config_inner(saved_config, legacy_config, &default_rule_set_name)
}

#[tauri::command(rename_all = "camelCase")]
pub fn resolve_effective_config(global_config: Value, project: Option<Value>) -> Value {
    resolve_effective_config_inner(global_config, project.as_ref())
}

fn migrate_app_config_inner(
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

fn resolve_effective_config_inner(global_config: Value, project: Option<&Value>) -> Value {
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
    config.insert(
        "polishPresetId".to_string(),
        Value::String(
            defaults
                .and_then(|value| value.get("polishPresetId"))
                .and_then(non_empty_str)
                .or_else(|| config.get("polishPresetId").and_then(non_empty_str))
                .unwrap_or(DEFAULT_POLISH_PRESET_ID)
                .to_string(),
        ),
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
        if !config.get(key).and_then(non_empty_str).is_some()
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

    false
}

fn normalize_current_config(existing: Value) -> Value {
    let original = existing.clone();
    let mut config = merge_default(existing);
    let summary_custom_templates =
        normalize_summary_custom_templates(config.get("summaryCustomTemplates"));
    let summary_template_id = coerce_summary_template_id(
        config.get("summaryTemplateId").and_then(Value::as_str),
        Some(&summary_custom_templates),
    );
    let polish_custom_presets = normalize_polish_custom_presets(config.get("polishCustomPresets"));
    let polish_preset_id = coerce_polish_preset_id(
        config.get("polishPresetId").and_then(Value::as_str),
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
    set(&mut config, "speakerProfiles", speaker_profiles);
    set(&mut config, "__original", original);
    config
}

fn remove_internal_marker(mut value: Value) -> Value {
    if let Some(object) = value.as_object_mut() {
        object.remove("__original");
    }
    value
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
    existing.get("logLevel") != normalized.get("logLevel")
}

fn upgrade_config(parsed: Value, default_rule_set_name: &str) -> Value {
    let mut config = merge_default(parsed.clone());
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
        (
            "streamingModelPath",
            json!(first_string(
                &parsed,
                &[
                    "streamingModelPath",
                    "recognitionModelPath",
                    "offlineModelPath",
                    "modelPath"
                ]
            )
            .unwrap_or_default()),
        ),
        (
            "offlineModelPath",
            json!(first_string(
                &parsed,
                &["offlineModelPath", "recognitionModelPath", "modelPath"]
            )
            .unwrap_or_default()),
        ),
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
    ] {
        set(&mut config, key, value);
    }

    if config
        .get("textReplacementSets")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty)
    {
        if let Some(rules) = parsed.get("textReplacements").and_then(Value::as_array) {
            if !rules.is_empty() {
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
        }
    }

    if config
        .get("hotwordSets")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty)
    {
        if let Some(words) = parsed.get("hotwords").and_then(Value::as_array) {
            if !words.is_empty() {
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
        }
    }

    config
}

fn default_config() -> Value {
    let mut config = Map::new();
    for (key, value) in [
        ("configVersion", json!(CURRENT_CONFIG_VERSION)),
        ("appLanguage", json!("auto")),
        ("theme", json!("auto")),
        ("font", json!("system")),
        ("minimizeToTrayOnExit", json!(true)),
        ("autoCheckUpdates", json!(true)),
        ("logLevel", json!("info")),
        ("liveRecordShortcut", json!("Ctrl + Space")),
        ("microphoneId", json!("default")),
        ("systemAudioDeviceId", json!("default")),
        ("muteDuringRecording", json!(false)),
        ("streamingModelPath", json!("")),
        ("offlineModelPath", json!("")),
        ("punctuationModelPath", json!("")),
        ("vadModelPath", json!("")),
        ("speakerSegmentationModelPath", json!("")),
        ("speakerEmbeddingModelPath", json!("")),
        ("lockWindow", json!(false)),
        ("alwaysOnTop", json!(true)),
        ("startOnLaunch", json!(false)),
        ("captionWindowWidth", json!(800)),
        ("captionFontSize", json!(24)),
        ("captionFontColor", json!("#ffffff")),
        ("captionBackgroundOpacity", json!(0.6)),
        ("language", json!("auto")),
        ("enableTimeline", json!(false)),
        ("enableITN", json!(true)),
        ("vadBufferSize", json!(5)),
        ("maxConcurrent", json!(2)),
        ("llmSettings", create_llm_settings(DEFAULT_LLM_PROVIDER)),
        ("summaryEnabled", json!(true)),
        ("summaryTemplateId", json!(DEFAULT_SUMMARY_TEMPLATE_ID)),
        ("summaryCustomTemplates", json!([])),
        ("translationLanguage", json!("zh")),
        ("polishKeywords", json!("")),
        ("polishPresetId", json!(DEFAULT_POLISH_PRESET_ID)),
        ("polishCustomPresets", json!([])),
        ("autoPolish", json!(false)),
        ("autoPolishFrequency", json!(5)),
        ("voiceTypingEnabled", json!(false)),
        ("voiceTypingShortcut", json!("Alt+V")),
        ("voiceTypingMode", json!("hold")),
        ("textReplacementSets", json!([])),
        ("hotwordSets", json!([])),
        ("polishKeywordSets", json!([])),
        ("speakerProfiles", json!([])),
        ("hotwords", json!([])),
    ] {
        config.insert(key.to_string(), value);
    }
    Value::Object(config)
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
    if let Some(custom_id) = template_id {
        if custom_templates
            .and_then(Value::as_array)
            .is_some_and(|templates| {
                templates
                    .iter()
                    .any(|template| template.get("id").and_then(Value::as_str) == Some(custom_id))
            })
        {
            return custom_id.to_string();
        }
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
    if let Some(custom_id) = preset_id {
        if custom_presets
            .and_then(Value::as_array)
            .is_some_and(|presets| {
                presets
                    .iter()
                    .any(|preset| preset.get("id").and_then(Value::as_str) == Some(custom_id))
            })
        {
            return custom_id.to_string();
        }
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
    let current_provider = normalize_provider(
        source
            .pointer("/llmSettings/activeProvider")
            .or_else(|| source.get("llmServiceType"))
            .or_else(|| source.pointer("/llm/provider")),
    );
    let mut providers = normalize_stored_providers(source.pointer("/llmSettings/providers"));

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
                ),
            );
        }
    }

    providers.insert(
        current_provider.clone(),
        sanitize_provider_setting(&current_provider, providers.get(&current_provider).cloned()),
    );

    let models = normalize_stored_models(source.pointer("/llmSettings/models"));
    let model_order =
        normalize_stored_model_order(source.pointer("/llmSettings/modelOrder"), &models);
    let selections =
        normalize_stored_selections(source.pointer("/llmSettings/selections"), &models);

    let mut settings = json!({
        "activeProvider": current_provider,
        "providers": providers,
        "models": models,
        "modelOrder": model_order,
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
    match provider {
        "anthropic"
        | "azure_openai"
        | "deep_seek"
        | "gemini"
        | "kimi"
        | "ollama"
        | "open_ai"
        | "open_ai_responses"
        | "open_ai_compatible"
        | "google_translate"
        | "google_translate_free"
        | "silicon_flow"
        | "qwen"
        | "qwen_portal"
        | "minimax_global"
        | "minimax_cn"
        | "openrouter"
        | "lm_studio"
        | "groq"
        | "x_ai"
        | "mistral_ai"
        | "perplexity"
        | "volcengine"
        | "chatglm" => provider.to_string(),
        "azure_open_ai" => "azure_openai".to_string(),
        "deepseek" => "deep_seek".to_string(),
        "moonshot" => "kimi".to_string(),
        "openai" => "open_ai".to_string(),
        "openai_compatible" => "open_ai_compatible".to_string(),
        "siliconflow" => "silicon_flow".to_string(),
        _ => DEFAULT_LLM_PROVIDER.to_string(),
    }
}

fn provider_defaults(provider: &str) -> Map<String, Value> {
    let (api_host, api_path, api_version) = match provider {
        "google_translate_free" => (
            "https://translate.googleapis.com/translate_a/single",
            None,
            None,
        ),
        "google_translate" => (
            "https://translation.googleapis.com/language/translate/v2",
            None,
            None,
        ),
        "open_ai" | "open_ai_responses" => (
            "https://api.openai.com",
            if provider == "open_ai_responses" {
                Some("/v1/responses")
            } else {
                None
            },
            None,
        ),
        "azure_openai" => ("", None, Some("2024-10-21")),
        "anthropic" => ("https://api.anthropic.com", None, None),
        "gemini" => ("https://generativelanguage.googleapis.com", None, None),
        "ollama" => ("http://127.0.0.1:11434", None, None),
        "deep_seek" => ("https://api.deepseek.com", None, None),
        "kimi" => ("https://api.moonshot.cn", None, None),
        "silicon_flow" => ("https://api.siliconflow.cn", None, None),
        "qwen" => (
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
            None,
            None,
        ),
        "qwen_portal" => ("https://portal.qwen.ai/v1", None, None),
        "minimax_global" => ("https://api.minimaxi.chat/v1", None, None),
        "minimax_cn" => ("https://api.minimax.chat/v1", None, None),
        "openrouter" => ("https://openrouter.ai/api/v1", None, None),
        "lm_studio" => ("http://localhost:1234/v1", None, None),
        "groq" => ("https://api.groq.com/openai", None, None),
        "x_ai" => ("https://api.x.ai", None, None),
        "mistral_ai" => ("https://api.mistral.ai/v1", None, None),
        "perplexity" => ("https://api.perplexity.ai", Some("/chat/completions"), None),
        "volcengine" => (
            "https://ark.cn-beijing.volces.com",
            Some("/api/v3/chat/completions"),
            None,
        ),
        "chatglm" => ("https://open.bigmodel.cn/api/paas/v4/", None, None),
        _ => ("", None, None),
    };
    let mut defaults = Map::new();
    defaults.insert("apiHost".to_string(), json!(api_host));
    defaults.insert("apiKey".to_string(), json!(""));
    if let Some(api_path) = api_path {
        defaults.insert("apiPath".to_string(), json!(api_path));
    }
    if let Some(api_version) = api_version {
        defaults.insert("apiVersion".to_string(), json!(api_version));
    }
    defaults
}

fn sanitize_provider_setting(provider: &str, setting: Option<Value>) -> Value {
    let mut defaults = provider_defaults(provider);
    if let Some(setting_object) = setting.and_then(|value| value.as_object().cloned()) {
        for key in ["apiHost", "apiKey", "apiPath", "apiVersion"] {
            if let Some(value) = setting_object.get(key) {
                defaults.insert(key.to_string(), value.clone());
            }
        }
    }
    Value::Object(defaults)
}

fn create_llm_settings(active_provider: &str) -> Value {
    let mut providers = Map::new();
    providers.insert(
        active_provider.to_string(),
        sanitize_provider_setting(active_provider, None),
    );
    json!({
        "activeProvider": active_provider,
        "providers": providers,
        "models": {},
        "modelOrder": [],
        "selections": {},
    })
}

fn normalize_stored_providers(value: Option<&Value>) -> Map<String, Value> {
    let mut providers = Map::new();
    if let Some(object) = value.and_then(Value::as_object) {
        for (raw_provider, raw_setting) in object {
            let provider = normalize_provider(Some(&Value::String(raw_provider.clone())));
            providers.insert(
                provider.clone(),
                sanitize_provider_setting(&provider, Some(raw_setting.clone())),
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
        models.insert(
            id.to_string(),
            json!({ "id": id, "provider": provider, "model": model }),
        );
    }
    models
}

fn normalize_stored_model_order(value: Option<&Value>, models: &Map<String, Value>) -> Vec<Value> {
    let mut seen = HashSet::new();
    let mut ordered = Vec::new();
    if let Some(order) = value.and_then(Value::as_array) {
        for entry in order {
            if let Some(id) = entry.as_str().filter(|id| models.contains_key(*id)) {
                if seen.insert(id.to_string()) {
                    ordered.push(json!(id));
                }
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
    if provider != "google_translate" && provider != "google_translate_free" {
        set_pointer(&mut settings, "/selections/polishModelId", json!(model_id));
    } else if legacy_model_present {
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
        assert_eq!(result.config["configVersion"], 6);
        assert_eq!(result.config["streamingModelPath"], "models/recognition");
        assert_eq!(result.config["offlineModelPath"], "models/recognition");
        assert_eq!(result.config["logLevel"], "info");
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
                "activeProvider": "open_ai",
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
            }
        });

        let result = migrate_app_config(Some(saved), None, "Default Rules".to_string());

        assert!(!result.migrated);
        assert_eq!(result.config["configVersion"], 6);
        assert_eq!(result.config["summaryTemplateId"], "meeting");
        assert_eq!(result.config["polishPresetId"], "meeting");
        assert_eq!(result.config["logLevel"], "debug");
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
}
