#![allow(dead_code)]

#[path = "../src/automation_repository.rs"]
mod automation_repository;

use automation_repository::{
    create_automation_fingerprint, normalize_automation_path, validate_rule_activation_inner,
    AutomationRepository, AutomationRule, AutomationRuleExportConfig, AutomationRuleStageConfig,
};
use serde_json::{json, Value};
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
