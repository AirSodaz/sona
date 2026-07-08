use serde_json::json;
use sona_core::config::{CURRENT_CONFIG_VERSION, DEFAULT_LLM_PROVIDER, default_config};
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
    assert_eq!(resolved["translationLanguage"], "ja");
}
