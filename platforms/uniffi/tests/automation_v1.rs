use sona_uniffi_bind::{
    FfiAutomationExportConfigV1, FfiAutomationProcessedInputV1, FfiAutomationProjectReferenceV1,
    FfiAutomationRepositoryInputV1, FfiAutomationRuleInputV1, FfiAutomationStageConfigV1,
    FfiAutomationValidationExportConfigV1, FfiAutomationValidationRuleV1,
    FfiAutomationValidationStageConfigV1, SonaCoreBindingError,
    load_automation_repository_state_v1, replace_automation_processed_entries_v1,
    replace_automation_repository_state_v1, replace_automation_rules_v1,
    validate_automation_rule_activation_v1,
};
use std::fs;

fn rule(id: Option<&str>, name: &str) -> FfiAutomationRuleInputV1 {
    FfiAutomationRuleInputV1 {
        id: id.map(str::to_string),
        name: name.to_string(),
        save_history: false,
        project_id: String::new(),
        watch_directory: "C:\\watch".to_string(),
        recursive: true,
        enabled: true,
        stage_config: FfiAutomationStageConfigV1 {
            auto_polish: true,
            polish_preset_id: "general".to_string(),
            auto_translate: true,
            translation_language: "zh".to_string(),
        },
        export_config: FfiAutomationExportConfigV1 {
            directory: "C:\\export".to_string(),
            format: "srt".to_string(),
            mode: "bilingual".to_string(),
            prefix: "done-".to_string(),
        },
        created_at: 100,
        updated_at: 200,
    }
}

fn processed(id: Option<&str>, rule_id: &str) -> FfiAutomationProcessedInputV1 {
    FfiAutomationProcessedInputV1 {
        id: id.map(str::to_string),
        rule_id: rule_id.to_string(),
        attempt: 1,
        file_path: "C:\\watch\\audio.wav".to_string(),
        source_fingerprint: "fingerprint".to_string(),
        size: 42,
        mtime_ms: 300,
        status: "complete".to_string(),
        processed_at: 400,
        history_id: Some("history-1".to_string()),
        export_path: Some("C:\\export\\audio.srt".to_string()),
        error_message: None,
    }
}

#[test]
fn automation_v1_round_trips_typed_repository_state_without_json() {
    let dir = tempfile::tempdir().unwrap();
    let app_data_dir = dir.path().to_string_lossy().into_owned();

    let state = replace_automation_repository_state_v1(
        app_data_dir.clone(),
        FfiAutomationRepositoryInputV1 {
            rules: vec![rule(Some("rule-1"), "Rule")],
            processed_entries: vec![processed(Some("entry-1"), "rule-1")],
        },
    )
    .unwrap();

    assert_eq!(state.rules[0].id, "rule-1");
    assert_eq!(state.rules[0].stage_config.translation_language, "zh");
    assert_eq!(state.rules[0].export_config.format, "srt");
    assert_eq!(state.processed_entries[0].id, "entry-1");
    assert_eq!(
        state.processed_entries[0].history_id.as_deref(),
        Some("history-1")
    );
    assert_eq!(
        load_automation_repository_state_v1(app_data_dir).unwrap(),
        state
    );
}

#[test]
fn automation_v1_collection_replacements_preserve_the_other_collection() {
    let dir = tempfile::tempdir().unwrap();
    let app_data_dir = dir.path().to_string_lossy().into_owned();

    let with_processed = replace_automation_processed_entries_v1(
        app_data_dir.clone(),
        vec![processed(Some("entry-1"), "rule-1")],
    )
    .unwrap();
    assert!(with_processed.rules.is_empty());

    let with_rules =
        replace_automation_rules_v1(app_data_dir, vec![rule(Some("rule-1"), "Rule")]).unwrap();
    assert_eq!(with_rules.rules[0].id, "rule-1");
    assert_eq!(with_rules.processed_entries[0].id, "entry-1");
}

#[test]
fn automation_v1_validation_uses_typed_rule_and_tags() {
    let dir = tempfile::tempdir().unwrap();
    let watch_directory = dir.path().join("watch");
    let output_directory = dir.path().join("output");
    let model_path = dir.path().join("model.onnx");
    fs::create_dir(&watch_directory).unwrap();
    fs::write(&model_path, b"model").unwrap();
    let project_rule = FfiAutomationValidationRuleV1 {
        name: "Project Rule".to_string(),
        save_history: true,
        project_id: "tag-1".to_string(),
        watch_directory: watch_directory.to_string_lossy().into_owned(),
        recursive: false,
        stage_config: FfiAutomationValidationStageConfigV1 {
            auto_polish: false,
            auto_translate: false,
        },
        export_config: FfiAutomationValidationExportConfigV1 {
            directory: String::new(),
            mode: "original".to_string(),
        },
    };
    let projects = vec![FfiAutomationProjectReferenceV1 {
        id: "tag-1".to_string(),
    }];

    let project_result = validate_automation_rule_activation_v1(
        project_rule,
        serde_json::json!({"offlineModelPath": model_path}).to_string(),
        projects.clone(),
    )
    .unwrap();
    assert!(project_result.valid);
    assert_eq!(project_result.code, None);
    assert!(!output_directory.exists());

    let rule = FfiAutomationValidationRuleV1 {
        name: "Rule".to_string(),
        save_history: false,
        project_id: String::new(),
        watch_directory: watch_directory.to_string_lossy().into_owned(),
        recursive: false,
        stage_config: FfiAutomationValidationStageConfigV1 {
            auto_polish: false,
            auto_translate: false,
        },
        export_config: FfiAutomationValidationExportConfigV1 {
            directory: output_directory.to_string_lossy().into_owned(),
            mode: "original".to_string(),
        },
    };

    let result = validate_automation_rule_activation_v1(
        rule.clone(),
        serde_json::json!({"offlineModelPath": model_path}).to_string(),
        projects.clone(),
    )
    .unwrap();
    assert!(result.valid);
    assert_eq!(result.code, None);
    assert!(output_directory.is_dir());

    fs::remove_dir(&output_directory).unwrap();
    let error =
        validate_automation_rule_activation_v1(rule, "{".to_string(), projects).unwrap_err();
    assert!(matches!(error, SonaCoreBindingError::InvalidInput { .. }));
    assert!(!output_directory.exists());
}
