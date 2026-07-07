use serde_json::json;
use sona_core::automation::{
    AutomationRule, AutomationRuleActivationEnvironment, AutomationRuleExportConfig,
    AutomationRuleStageConfig, AutomationRuntimePathCollectionOutcome,
    AutomationRuntimePathMetadata, AutomationRuntimeRuleConfig, collect_runtime_rule_path_result,
    resolve_batch_model_path, should_consider_runtime_candidate_path, validate_rule_activation,
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
            batch_model_path_exists: false,
        },
    );

    assert!(result.valid, "{result:?}");
    assert_eq!(result.code, None);
}

#[test]
fn resolves_trimmed_batch_model_path() {
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
        resolve_batch_model_path(&config).as_deref(),
        Some("C:\\models\\sherpa")
    );
}

#[test]
fn runtime_path_collection_maps_file_metadata_to_candidate_payload() {
    let rule = sample_runtime_rule(|_| {});

    let result = collect_runtime_rule_path_result(
        &rule,
        "C:\\watch\\Meeting.WAV",
        Ok(Some(AutomationRuntimePathMetadata {
            is_file: true,
            size: 42,
            mtime_ms: 1_700_000_000_000,
        })),
    );

    assert_eq!(
        result.outcome,
        AutomationRuntimePathCollectionOutcome::Candidate
    );
    let candidate = result.candidate.expect("candidate payload");
    assert_eq!(candidate.rule_id, "rule-1");
    assert_eq!(candidate.file_path, "C:\\watch\\Meeting.WAV");
    assert_eq!(candidate.size, 42);
    assert_eq!(candidate.mtime_ms, 1_700_000_000_000);
    assert_eq!(
        candidate.source_fingerprint,
        "c:\\watch\\meeting.wav::42::1700000000000"
    );
}

#[test]
fn runtime_path_collection_classifies_adapter_metadata_states() {
    let rule = sample_runtime_rule(|_| {});

    assert_eq!(
        collect_runtime_rule_path_result(&rule, "C:\\watch\\notes.txt", Ok(None)).outcome,
        AutomationRuntimePathCollectionOutcome::Unsupported
    );
    assert_eq!(
        collect_runtime_rule_path_result(&rule, "C:\\watch\\exports\\meeting.wav", Ok(None))
            .outcome,
        AutomationRuntimePathCollectionOutcome::Excluded
    );
    assert_eq!(
        collect_runtime_rule_path_result(&rule, "C:\\watch\\missing.wav", Ok(None)).outcome,
        AutomationRuntimePathCollectionOutcome::Missing
    );
    assert_eq!(
        collect_runtime_rule_path_result(
            &rule,
            "C:\\watch\\folder.wav",
            Ok(Some(AutomationRuntimePathMetadata {
                is_file: false,
                size: 0,
                mtime_ms: 0,
            })),
        )
        .outcome,
        AutomationRuntimePathCollectionOutcome::NotFile
    );

    let error_result = collect_runtime_rule_path_result(
        &rule,
        "C:\\watch\\meeting.wav",
        Err("denied".to_string()),
    );
    assert_eq!(
        error_result.outcome,
        AutomationRuntimePathCollectionOutcome::Error
    );
    assert_eq!(error_result.error.as_deref(), Some("denied"));
}

#[test]
fn runtime_candidate_path_filter_respects_non_recursive_watch_scope() {
    let rule = sample_runtime_rule(|rule| {
        rule.recursive = false;
    });

    assert!(should_consider_runtime_candidate_path(
        &rule,
        "C:\\watch\\meeting.wav"
    ));
    assert!(!should_consider_runtime_candidate_path(
        &rule,
        "C:\\watch\\nested\\meeting.wav"
    ));
}

fn sample_runtime_rule(
    overrides: impl FnOnce(&mut AutomationRuntimeRuleConfig),
) -> AutomationRuntimeRuleConfig {
    let mut rule = AutomationRuntimeRuleConfig {
        rule_id: "rule-1".to_string(),
        watch_directory: "C:\\watch".to_string(),
        recursive: true,
        exclude_directory: "C:\\watch\\exports".to_string(),
        debounce_ms: 5,
        stable_window_ms: 10,
    };
    overrides(&mut rule);
    rule
}
