use serde_json::json;
use sona_core::config::{
    AppConfig, AppLanguagePreference, CURRENT_CONFIG_VERSION, DEFAULT_LLM_PROVIDER, default_config,
    validate_app_config,
};
use sona_core::config::{migrate_app_config, resolve_effective_config};
use sona_core::llm::providers::find_llm_provider_by_id_or_alias;

#[test]
fn default_config_and_llm_provider_manifest_are_core_owned() {
    let config = default_config();
    let provider = find_llm_provider_by_id_or_alias("openai").expect("openai alias should exist");

    assert_eq!(config["configVersion"], json!(CURRENT_CONFIG_VERSION));
    assert_eq!(
        config["llmSettings"]["activeProvider"],
        DEFAULT_LLM_PROVIDER
    );
    assert_eq!(provider.id, "open_ai");
    assert_eq!(provider.defaults.api_host, "https://api.openai.com");
}

#[test]
fn default_config_conforms_to_the_typed_app_config_contract() {
    let config = default_config();
    let typed: AppConfig =
        serde_json::from_value(config).expect("default config should match AppConfig");

    assert_eq!(typed.app_language, Some(AppLanguagePreference::Auto));
    assert_eq!(typed.streaming_model_path.as_deref(), Some(""));
    assert_eq!(typed.batch_model_path.as_deref(), Some(""));
    assert_eq!(typed.language.as_deref(), Some("auto"));
}

#[test]
fn typed_app_config_preserves_unknown_future_fields() {
    let mut config = default_config();
    config["futureHostSetting"] = json!({ "enabled": true });

    let typed: AppConfig =
        serde_json::from_value(config).expect("unknown config fields should be preserved");
    let encoded = serde_json::to_value(typed).expect("typed config should serialize");

    assert_eq!(encoded["futureHostSetting"]["enabled"], true);
}

#[test]
fn typed_app_config_validation_rejects_invalid_known_fields() {
    let mut config = default_config();
    config["appLanguage"] = json!(42);

    assert!(validate_app_config(&config).is_err());
}

#[test]
fn current_config_migration_emits_typed_string_ids() {
    let result = migrate_app_config(Some(default_config()), None, "Default Rules".to_string());

    validate_app_config(&result.config).expect("normalized config should match AppConfig");
    assert_eq!(result.config["summaryTemplateId"], "general");
    assert_eq!(result.config["polishPresetId"], "general");
}

#[test]
fn current_config_migration_repairs_invalid_typed_scalar_fields() {
    let mut config = default_config();
    config["appLanguage"] = json!(42);
    config["theme"] = json!("neon");
    config["captionWindowWidth"] = json!("wide");
    config["voiceTypingMode"] = json!("press");

    let result = migrate_app_config(Some(config), None, "Default Rules".to_string());

    validate_app_config(&result.config).expect("migration output should match AppConfig");
    assert_eq!(result.config["appLanguage"], "auto");
    assert_eq!(result.config["theme"], "auto");
    assert_eq!(result.config["captionWindowWidth"], 800);
    assert_eq!(result.config["voiceTypingMode"], "hold");
}

#[test]
fn migration_normalizes_llm_provider_aliases_from_core_manifest() {
    let mut saved = default_config();
    saved["configVersion"] = json!(1);
    saved["llmSettings"]["activeProvider"] = json!("openai");

    let result = migrate_app_config(Some(saved), None, "Default Rules".to_string());

    assert!(result.migrated);
    assert_eq!(result.config["llmSettings"]["activeProvider"], "open_ai");
    assert_eq!(
        result.config["llmSettings"]["providers"]["open_ai"]["apiHost"],
        "https://api.openai.com"
    );
}

#[test]
fn effective_config_resolution_uses_core_config_types() {
    let global = json!({
        "summaryTemplateId": "meeting",
        "summaryCustomTemplates": []
    });
    let project = json!({
        "defaults": {
            "translationLanguage": "ja"
        }
    });

    let resolved = resolve_effective_config(global, Some(project));

    assert_eq!(resolved["summaryTemplateId"], "meeting");
    assert_eq!(resolved["polishPresetId"], "general");
    assert_eq!(resolved["translationLanguage"], "ja");
}
