#![allow(dead_code)]

pub mod integrations {
    #[path = "../../src/integrations/asr_providers.rs"]
    pub mod asr_providers;
}
#[path = "../src/repositories/automation.rs"]
mod automation_repository;

use automation_repository::{
    AutomationRepository, AutomationRule, AutomationRuleExportConfig, AutomationRuleStageConfig,
    create_automation_fingerprint, normalize_automation_path, validate_rule_activation_inner,
};
use serde_json::{Value, json};
use std::fs;
use tempfile::tempdir;

fn sample_rule(
    watch_directory: String,
    export_directory: String,
    overrides: impl FnOnce(&mut AutomationRule),
) -> AutomationRule {
    let mut rule = AutomationRule {
        name: "Meeting Inbox".to_string(),
        project_id: "project-1".to_string(),
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

fn valid_global_config(offline_model_path: &str) -> Value {
    json!({
        "offlineModelPath": offline_model_path,
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
fn load_state_creates_compatible_manifest_files() {
    let dir = tempdir().unwrap();
    let repository = AutomationRepository::new(dir.path().to_path_buf());

    let state = repository.load_state().unwrap();

    assert_eq!(state.rules, Vec::<Value>::new());
    assert_eq!(state.processed_entries, Vec::<Value>::new());
    assert_eq!(
        fs::read_to_string(dir.path().join("automation").join("rules.json")).unwrap(),
        "[]"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("automation").join("processed.json")).unwrap(),
        "[]"
    );
}

#[test]
fn persist_state_round_trips_existing_json_shapes() {
    let dir = tempdir().unwrap();
    let repository = AutomationRepository::new(dir.path().to_path_buf());
    let rules = vec![json!({
        "id": "rule-1",
        "name": "Meeting Inbox",
        "watchDirectory": "C:/watch",
        "stageConfig": {
            "autoPolish": true,
            "autoTranslate": false
        }
    })];
    let processed_entries = vec![json!({
        "ruleId": "rule-1",
        "filePath": "C:/watch/meeting.wav",
        "sourceFingerprint": "c:\\watch\\meeting.wav::10::20",
        "size": 10,
        "mtimeMs": 20,
        "status": "complete",
        "processedAt": 30
    })];

    repository
        .persist_state(rules.clone(), processed_entries.clone())
        .unwrap();
    let state = repository.load_state().unwrap();

    assert_eq!(state.rules, rules);
    assert_eq!(state.processed_entries, processed_entries);
}

#[test]
fn path_normalization_matches_the_frontend_fingerprint_contract() {
    assert_eq!(
        normalize_automation_path(" C:/Watch/Sub// "),
        "c:\\watch\\sub"
    );
    assert_eq!(
        create_automation_fingerprint(" C:/Watch/Meeting.WAV ", 42, 1000),
        "c:\\watch\\meeting.wav::42::1000"
    );
}

#[test]
fn validation_uses_normalized_paths_for_same_directory_checks() {
    let rule = sample_rule(" C:/Watch ".to_string(), "c:\\watch\\".to_string(), |_| {});

    let result = validate_rule_activation_inner(&rule, &json!({}), Some(&json!({})));

    assert!(!result.valid);
    assert_eq!(result.code.as_deref(), Some("automation.same_directory"));
}

#[test]
fn validation_creates_the_output_directory_for_valid_rules() {
    let dir = tempdir().unwrap();
    let watch_dir = dir.path().join("watch");
    let export_dir = dir.path().join("exports");
    let offline_model_dir = dir.path().join("offline-model");
    fs::create_dir_all(&watch_dir).unwrap();
    fs::create_dir_all(&offline_model_dir).unwrap();
    let rule = sample_rule(
        watch_dir.to_string_lossy().into_owned(),
        export_dir.to_string_lossy().into_owned(),
        |_| {},
    );
    let global_config = valid_global_config(offline_model_dir.to_string_lossy().as_ref());

    let result = validate_rule_activation_inner(&rule, &global_config, Some(&json!({})));

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
                "mode": "offline",
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

    let result = validate_rule_activation_inner(&rule, &global_config, Some(&json!({})));

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
                "mode": "offline",
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

    let result = validate_rule_activation_inner(&rule, &global_config, Some(&json!({})));

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
                "mode": "offline",
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

    let result = validate_rule_activation_inner(&rule, &global_config, Some(&json!({})));

    assert!(!result.valid);
    assert_eq!(
        result.code.as_deref(),
        Some("automation.offline_model_missing")
    );
}

#[test]
fn validation_accepts_inbox_without_project_record() {
    let dir = tempdir().unwrap();
    let watch_dir = dir.path().join("watch");
    let export_dir = dir.path().join("exports");
    let offline_model_dir = dir.path().join("offline-model");
    fs::create_dir_all(&watch_dir).unwrap();
    fs::create_dir_all(&offline_model_dir).unwrap();
    let rule = sample_rule(
        watch_dir.to_string_lossy().into_owned(),
        export_dir.to_string_lossy().into_owned(),
        |rule| {
            rule.project_id = "inbox".to_string();
        },
    );
    let global_config = valid_global_config(offline_model_dir.to_string_lossy().as_ref());

    let result = validate_rule_activation_inner(&rule, &global_config, None);

    assert!(result.valid);
}

#[test]
fn validation_rejects_missing_real_project_record() {
    let dir = tempdir().unwrap();
    let watch_dir = dir.path().join("watch");
    let export_dir = dir.path().join("exports");
    let offline_model_dir = dir.path().join("offline-model");
    fs::create_dir_all(&watch_dir).unwrap();
    fs::create_dir_all(&offline_model_dir).unwrap();
    let rule = sample_rule(
        watch_dir.to_string_lossy().into_owned(),
        export_dir.to_string_lossy().into_owned(),
        |rule| {
            rule.project_id = "missing-project".to_string();
        },
    );
    let global_config = valid_global_config(offline_model_dir.to_string_lossy().as_ref());

    let result = validate_rule_activation_inner(&rule, &global_config, None);

    assert!(!result.valid);
    assert_eq!(result.code.as_deref(), Some("automation.project_missing"));
}

#[test]
fn validation_requires_feature_models_when_auto_stages_are_enabled() {
    let dir = tempdir().unwrap();
    let watch_dir = dir.path().join("watch");
    let export_dir = dir.path().join("exports");
    let offline_model_dir = dir.path().join("offline-model");
    fs::create_dir_all(&watch_dir).unwrap();
    fs::create_dir_all(&offline_model_dir).unwrap();
    let rule = sample_rule(
        watch_dir.to_string_lossy().into_owned(),
        export_dir.to_string_lossy().into_owned(),
        |rule| {
            rule.stage_config.auto_polish = true;
        },
    );
    let global_config = json!({
        "offlineModelPath": offline_model_dir,
        "llmSettings": {
            "activeProvider": "open_ai",
            "providers": {},
            "models": {},
            "modelOrder": [],
            "selections": {}
        }
    });

    let result = validate_rule_activation_inner(&rule, &global_config, Some(&json!({})));

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
    let offline_model_dir = dir.path().join("offline-model");
    fs::create_dir_all(&watch_dir).unwrap();
    fs::create_dir_all(&offline_model_dir).unwrap();
    let rule = sample_rule(
        watch_dir.to_string_lossy().into_owned(),
        export_dir.to_string_lossy().into_owned(),
        |rule| {
            rule.stage_config.auto_translate = true;
            rule.export_config.mode = "translation".to_string();
        },
    );
    let global_config = valid_global_config(offline_model_dir.to_string_lossy().as_ref());

    let result = validate_rule_activation_inner(&rule, &global_config, Some(&json!({})));

    assert!(result.valid);
}
