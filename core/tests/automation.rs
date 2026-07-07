use serde_json::json;
use sona_core::automation::{
    AutomationRule, AutomationRuleActivationEnvironment, AutomationRuleExportConfig,
    AutomationRuleStageConfig, resolve_batch_offline_model_path, validate_rule_activation,
};

#[test]
fn valid_online_asr_and_custom_llm_rule_passes_activation_validation() {
    let rule = AutomationRule {
        name: "Import calls".to_string(),
        project_id: "project-1".to_string(),
        watch_directory: "C:\\watch".to_string(),
        stage_config: AutomationRuleStageConfig {
            auto_polish: true,
            auto_translate: false,
        },
        export_config: AutomationRuleExportConfig {
            directory: "C:\\exports".to_string(),
            mode: "original".to_string(),
        },
    };
    let config = json!({
        "asr": {
            "selections": {
                "batch": {
                    "engine": "online",
                    "providerId": "groq-whisper"
                }
            },
            "providers": {
                "online": {
                    "groq-whisper": {
                        "apiKey": "groq-key"
                    }
                }
            }
        },
        "llmSettings": {
            "customProviders": {
                "custom-acme": {
                    "id": "custom-acme",
                    "strategy": "openai_responses"
                }
            },
            "providers": {
                "custom-acme": {
                    "apiHost": "https://gateway.example.com",
                    "apiKey": "test-key"
                }
            },
            "models": {
                "model-1": {
                    "provider": "custom-acme",
                    "model": "gpt-4o"
                }
            },
            "selections": {
                "polishModelId": "model-1"
            }
        }
    });

    let result = validate_rule_activation(
        &rule,
        &config,
        Some(&json!({ "id": "project-1" })),
        AutomationRuleActivationEnvironment {
            watch_directory_exists: true,
            export_directory_ready: true,
            offline_model_path_exists: false,
        },
    );

    assert!(result.valid, "{result:?}");
    assert_eq!(result.code, None);
}

#[test]
fn resolves_trimmed_batch_offline_model_path() {
    let config = json!({
        "asr": {
            "selections": {
                "batch": {
                    "engine": "local",
                    "modelPath": "  C:\\models\\sherpa  "
                }
            }
        },
        "offlineModelPath": "C:\\legacy"
    });

    assert_eq!(
        resolve_batch_offline_model_path(&config).as_deref(),
        Some("C:\\models\\sherpa")
    );
}
