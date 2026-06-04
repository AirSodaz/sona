use serde_json::{Map, Value, json};

use crate::core::domain::{
    BuiltinPolishPresetId, BuiltinSummaryTemplateId,
    PolishPresetId, SummaryTemplateId,
};
use crate::integrations::asr_providers::online_asr_providers;

pub const CURRENT_CONFIG_VERSION: i64 = 7;
pub const DEFAULT_POLISH_PRESET_ID: &str = "general";
pub const DEFAULT_SUMMARY_TEMPLATE_ID: &str = "general";
pub const DEFAULT_LLM_PROVIDER: &str = "google_translate_free";
pub const LEGACY_OPENAI_COMPATIBLE_PROVIDER: &str = "custom-openai-compatible";
pub const LEGACY_OPENAI_COMPATIBLE_CREATED_AT: &str = "2026-05-18T00:00:00.000Z";

pub const BUILTIN_POLISH_PRESET_IDS: [&str; 6] = [
    "general",
    "customer_service",
    "meeting",
    "interview",
    "lecture",
    "podcast",
];
pub const BUILTIN_SUMMARY_TEMPLATE_IDS: [&str; 3] = ["general", "meeting", "lecture"];

pub fn default_config() -> Value {
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
        ("asr", default_asr_config()),
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
        ("batchVadEnabled", json!(true)),
        ("vadBufferSize", json!(5)),
        ("maxConcurrent", json!(2)),
        ("llmSettings", create_llm_settings()),
        ("summaryEnabled", json!(true)),
        (
            "summaryTemplateId",
            json!(SummaryTemplateId::Builtin(
                BuiltinSummaryTemplateId::General
            )),
        ),
        ("summaryCustomTemplates", json!([])),
        ("translationLanguage", json!("zh")),
        ("polishKeywords", json!("")),
        (
            "polishPresetId",
            json!(PolishPresetId::Builtin(BuiltinPolishPresetId::General)),
        ),
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
        ("httpServerEnabled", json!(false)),
        ("httpServerPort", json!(14200)),
        ("httpServerHost", json!("127.0.0.1")),
        ("httpServerApiKey", json!("")),
    ] {
        config.insert(key.to_string(), value);
    }
    Value::Object(config)
}

pub fn default_asr_config() -> Value {
    let mut config = Map::new();
    let mut selections = Map::new();
    for key in ["live", "caption", "voiceTyping"] {
        selections.insert(
            key.to_string(),
            json!({
                "engine": "local-sherpa",
                "mode": "streaming",
                "modelId": null,
                "modelPath": ""
            }),
        );
    }
    selections.insert(
        "batch".to_string(),
        json!({
            "engine": "local-sherpa",
            "mode": "offline",
            "modelId": null,
            "modelPath": ""
        }),
    );
    config.insert("selections".to_string(), Value::Object(selections));

    let mut online_providers = Map::new();
    for provider in online_asr_providers() {
        online_providers.insert(provider.id.clone(), provider.defaults.clone());
    }
    let mut providers = Map::new();
    providers.insert("online".to_string(), Value::Object(online_providers));
    config.insert("providers".to_string(), Value::Object(providers));

    Value::Object(config)
}

pub fn create_llm_settings() -> Value {
    json!({
        "activeProvider": DEFAULT_LLM_PROVIDER,
        "customProviders": {},
        "providers": {
            DEFAULT_LLM_PROVIDER: {
                "apiHost": "https://translate.googleapis.com/translate_a/single",
                "apiKey": ""
            }
        },
        "models": {},
        "modelOrder": [],
        "selections": {}
    })
}
