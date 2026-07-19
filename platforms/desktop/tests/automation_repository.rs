use serde_json::{Value, json};
use sona_core::automation::{
    AutomationRule, AutomationRuleExportConfig, AutomationRuleStageConfig,
    normalize_automation_path,
};
use std::fs;
use tauri_appsona_lib::platform::automation_repository::validate_rule_activation_inner;
use tempfile::tempdir;

fn sample_rule(
    watch_directory: String,
    export_directory: String,
    overrides: impl FnOnce(&mut AutomationRule),
) -> AutomationRule {
    let mut rule = AutomationRule {
        name: "Meeting Inbox".to_string(),
        save_history: true,
        tag_ids: vec!["tag-1".to_string()],
        watch_directory,
        stage_config: AutomationRuleStageConfig {
            auto_polish: false,
            auto_translate: false,
        },
        export_config: AutomationRuleExportConfig {
            directory: export_directory,
            mode: "original".to_string(),
        },
    };
    overrides(&mut rule);
    rule
}

fn valid_tags() -> Vec<Value> {
    vec![json!({ "id": "tag-1" })]
}

fn valid_global_config(batch_model_path: &str) -> Value {
    json!({
        "batchModelPath": batch_model_path,
        "asr": {
            "selections": {
                "batch": {
                    "engine": "local",
                    "mode": "batch",
                    "modelId": null,
                    "modelPath": batch_model_path,
                    "providerId": null,
                    "profileId": null
                }
            },
            "providers": {}
        },
        "llmSettings": {
            "activeProvider": "open_ai",
            "providers": {
                "open_ai": {
                    "apiHost": "https://api.openai.com",
                    "apiKey": "test-key"
                },
                "google_translate_free": {}
            },
            "models": {
                "polish-open-ai": {
                    "id": "polish-open-ai",
                    "provider": "open_ai",
                    "model": "gpt-4o-mini"
                },
                "translate-free": {
                    "id": "translate-free",
                    "provider": "google_translate_free",
                    "model": "translate"
                }
            },
            "modelOrder": ["polish-open-ai", "translate-free"],
            "selections": {
                "polishModelId": "polish-open-ai",
                "translationModelId": "translate-free"
            }
        }
    })
}

#[test]
fn path_normalization_matches_the_frontend_fingerprint_contract() {
    assert_eq!(
        normalize_automation_path(" C:/Watch/Sub// "),
        "c:\\watch\\sub"
    );
}

#[test]
fn validation_uses_normalized_paths_for_same_directory_checks() {
    let rule = sample_rule(" C:/Watch ".to_string(), "c:\\watch\\".to_string(), |_| {});

    let result = validate_rule_activation_inner(&rule, &json!({}), &valid_tags()).unwrap();

    assert!(!result.valid);
    assert_eq!(result.code.as_deref(), Some("automation.same_directory"));
}

#[test]
fn validation_creates_the_output_directory_for_valid_rules() {
    let dir = tempdir().unwrap();
    let watch_dir = dir.path().join("watch");
    let export_dir = dir.path().join("exports");
    let batch_model_dir = dir.path().join("batch-model");
    fs::create_dir_all(&watch_dir).unwrap();
    fs::create_dir_all(&batch_model_dir).unwrap();
    let rule = sample_rule(
        watch_dir.to_string_lossy().into_owned(),
        export_dir.to_string_lossy().into_owned(),
        |_| {},
    );
    let global_config = valid_global_config(batch_model_dir.to_string_lossy().as_ref());

    let result = validate_rule_activation_inner(&rule, &global_config, &valid_tags()).unwrap();

    assert!(result.valid);
    assert!(export_dir.is_dir());
}

#[test]
fn validation_accepts_configured_volcengine_batch_asr_without_local_model_path() {
    let dir = tempdir().unwrap();
    let watch_dir = dir.path().join("watch");
    let export_dir = dir.path().join("exports");
    fs::create_dir_all(&watch_dir).unwrap();
    let rule = sample_rule(
        watch_dir.to_string_lossy().into_owned(),
        export_dir.to_string_lossy().into_owned(),
        |_| {},
    );
    let mut global_config = valid_global_config("");
    global_config["asr"] = json!({
        "selections": {
            "batch": {
                "engine": "online",
                "mode": "batch",
                "modelId": null,
                "modelPath": "",
                "providerId": "volcengine-doubao",
                "profileId": "volcengine-doubao-default"
            }
        },
        "providers": {
            "online": {
                "volcengine-doubao": {
                    "apiKey": "volc-test-key",
                    "batchEndpoint": "https://openspeech.bytedance.com/api/v3/auc/bigmodel/recognize/flash",
                    "batchResourceId": "volc.bigasr.auc_turbo",
                    "streamingEndpoint": "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async",
                    "streamingResourceId": "volc.seedasr.sauc.duration"
                }
            }
        }
    });

    let result = validate_rule_activation_inner(&rule, &global_config, &valid_tags()).unwrap();

    assert!(result.valid);
    assert!(export_dir.is_dir());
}

#[test]
fn validation_uses_volcengine_batch_defaults_when_only_api_key_is_saved() {
    let dir = tempdir().unwrap();
    let watch_dir = dir.path().join("watch");
    let export_dir = dir.path().join("exports");
    fs::create_dir_all(&watch_dir).unwrap();
    let rule = sample_rule(
        watch_dir.to_string_lossy().into_owned(),
        export_dir.to_string_lossy().into_owned(),
        |_| {},
    );
    let mut global_config = valid_global_config("");
    global_config["asr"] = json!({
        "selections": {
            "batch": {
                "engine": "online",
                "mode": "batch",
                "modelId": null,
                "modelPath": "",
                "providerId": "volcengine-doubao",
                "profileId": "volcengine-doubao-default"
            }
        },
        "providers": {
            "online": {
                "volcengine-doubao": {
                    "apiKey": "volc-test-key"
                }
            }
        }
    });

    let result = validate_rule_activation_inner(&rule, &global_config, &valid_tags()).unwrap();

    assert!(result.valid);
    assert!(export_dir.is_dir());
}

#[test]
fn validation_rejects_volcengine_local_batch_when_saved_endpoint_is_url_only_async() {
    let dir = tempdir().unwrap();
    let watch_dir = dir.path().join("watch");
    let export_dir = dir.path().join("exports");
    fs::create_dir_all(&watch_dir).unwrap();
    let rule = sample_rule(
        watch_dir.to_string_lossy().into_owned(),
        export_dir.to_string_lossy().into_owned(),
        |_| {},
    );
    let mut global_config = valid_global_config("");
    global_config["asr"] = json!({
        "selections": {
            "batch": {
                "engine": "online",
                "mode": "batch",
                "modelId": null,
                "modelPath": "",
                "providerId": "volcengine-doubao",
                "profileId": "volcengine-doubao-default"
            }
        },
        "providers": {
            "online": {
                "volcengine-doubao": {
                    "apiKey": "volc-test-key",
                    "batchEndpoint": "https://openspeech.bytedance.com/api/v3/auc/bigmodel/submit",
                    "batchResourceId": "volc.seedasr.auc",
                    "streamingEndpoint": "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async",
                    "streamingResourceId": "volc.seedasr.sauc.duration"
                }
            }
        }
    });

    let result = validate_rule_activation_inner(&rule, &global_config, &valid_tags()).unwrap();

    assert!(!result.valid);
    assert_eq!(
        result.code.as_deref(),
        Some("automation.batch_model_missing")
    );
}

#[test]
fn validation_accepts_rules_that_do_not_save_history_without_tags() {
    let dir = tempdir().unwrap();
    let watch_dir = dir.path().join("watch");
    let export_dir = dir.path().join("exports");
    let batch_model_dir = dir.path().join("batch-model");
    fs::create_dir_all(&watch_dir).unwrap();
    fs::create_dir_all(&batch_model_dir).unwrap();
    let rule = sample_rule(
        watch_dir.to_string_lossy().into_owned(),
        export_dir.to_string_lossy().into_owned(),
        |rule| {
            rule.save_history = false;
            rule.tag_ids.clear();
        },
    );
    let global_config = valid_global_config(batch_model_dir.to_string_lossy().as_ref());

    let result = validate_rule_activation_inner(&rule, &global_config, &[]).unwrap();

    assert!(result.valid);
}

#[test]
fn validation_rejects_missing_tag() {
    let dir = tempdir().unwrap();
    let watch_dir = dir.path().join("watch");
    let export_dir = dir.path().join("exports");
    let batch_model_dir = dir.path().join("batch-model");
    fs::create_dir_all(&watch_dir).unwrap();
    fs::create_dir_all(&batch_model_dir).unwrap();
    let rule = sample_rule(
        watch_dir.to_string_lossy().into_owned(),
        export_dir.to_string_lossy().into_owned(),
        |rule| {
            rule.tag_ids = vec!["missing-tag".to_string()];
        },
    );
    let global_config = valid_global_config(batch_model_dir.to_string_lossy().as_ref());

    let result = validate_rule_activation_inner(&rule, &global_config, &[]).unwrap();

    assert!(!result.valid);
    assert_eq!(result.code.as_deref(), Some("automation.tag_missing"));
}

#[test]
fn validation_requires_feature_models_when_auto_stages_are_enabled() {
    let dir = tempdir().unwrap();
    let watch_dir = dir.path().join("watch");
    let export_dir = dir.path().join("exports");
    let batch_model_dir = dir.path().join("batch-model");
    fs::create_dir_all(&watch_dir).unwrap();
    fs::create_dir_all(&batch_model_dir).unwrap();
    let rule = sample_rule(
        watch_dir.to_string_lossy().into_owned(),
        export_dir.to_string_lossy().into_owned(),
        |rule| {
            rule.stage_config.auto_polish = true;
        },
    );
    let mut global_config = valid_global_config(batch_model_dir.to_string_lossy().as_ref());
    global_config["llmSettings"] = json!({
        "activeProvider": "open_ai",
        "providers": {},
        "models": {},
        "modelOrder": [],
        "selections": {}
    });

    let result = validate_rule_activation_inner(&rule, &global_config, &valid_tags()).unwrap();

    assert!(!result.valid);
    assert_eq!(
        result.code.as_deref(),
        Some("automation.polish_model_missing")
    );
}

#[test]
fn validation_accepts_translation_with_google_translate_free_without_an_api_key() {
    let dir = tempdir().unwrap();
    let watch_dir = dir.path().join("watch");
    let export_dir = dir.path().join("exports");
    let batch_model_dir = dir.path().join("batch-model");
    fs::create_dir_all(&watch_dir).unwrap();
    fs::create_dir_all(&batch_model_dir).unwrap();
    let rule = sample_rule(
        watch_dir.to_string_lossy().into_owned(),
        export_dir.to_string_lossy().into_owned(),
        |rule| {
            rule.stage_config.auto_translate = true;
            rule.export_config.mode = "translation".to_string();
        },
    );
    let global_config = valid_global_config(batch_model_dir.to_string_lossy().as_ref());

    let result = validate_rule_activation_inner(&rule, &global_config, &valid_tags()).unwrap();

    assert!(result.valid);
}
