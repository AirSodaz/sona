use serde_json::{Value, json};
use sona_core::automation::repository::{
    AutomationProcessedInput, AutomationProcessedRecord, AutomationRepositoryInput,
    AutomationRepositoryState, AutomationRuleInput, AutomationRuleRecord, AutomationStore,
};
use sona_core::automation::service::{
    AutomationFileSystem, AutomationIdGenerator, AutomationRepositoryService,
    AutomationValidationService,
};
use sona_core::automation::{
    AutomationError, AutomationRule, AutomationRuleExportConfig, AutomationRuleStageConfig,
};
use sona_core::ports::fs::{FileSystemError, FileSystemOperation};
use std::sync::Mutex;

#[derive(Default)]
struct MemoryStore {
    state: Mutex<AutomationRepositoryState>,
    calls: Mutex<Vec<&'static str>>,
    fail_with: Mutex<Option<String>>,
}

impl AutomationStore for MemoryStore {
    fn load_state(&self) -> Result<AutomationRepositoryState, AutomationError> {
        if let Some(error) = self.fail_with.lock().unwrap().clone() {
            return Err(AutomationError::Repository(error));
        }
        self.calls.lock().unwrap().push("load_state");
        Ok(self.state.lock().unwrap().clone())
    }

    fn replace_rules(&self, rules: &[AutomationRuleRecord]) -> Result<(), AutomationError> {
        self.calls.lock().unwrap().push("replace_rules");
        self.state.lock().unwrap().rules = rules.to_vec();
        Ok(())
    }

    fn replace_profiles(
        &self,
        profiles: &[sona_core::automation::repository::AutomationProfileRecord],
    ) -> Result<(), AutomationError> {
        self.calls.lock().unwrap().push("replace_profiles");
        self.state.lock().unwrap().profiles = profiles.to_vec();
        Ok(())
    }

    fn replace_processed_entries(
        &self,
        entries: &[AutomationProcessedRecord],
    ) -> Result<(), AutomationError> {
        self.calls.lock().unwrap().push("replace_processed_entries");
        self.state.lock().unwrap().processed_entries = entries.to_vec();
        Ok(())
    }

    fn replace_state(&self, state: &AutomationRepositoryState) -> Result<(), AutomationError> {
        self.calls.lock().unwrap().push("replace_state");
        *self.state.lock().unwrap() = state.clone();
        Ok(())
    }
}

struct SequenceIds(Mutex<Vec<String>>);

impl AutomationIdGenerator for SequenceIds {
    fn generate_id(&self) -> String {
        self.0.lock().unwrap().remove(0)
    }
}

#[derive(Default)]
struct FakeFileSystem {
    existing: Mutex<Vec<String>>,
    queried: Mutex<Vec<String>>,
    created: Mutex<Vec<String>>,
    create_succeeds: bool,
    metadata_failure: Option<String>,
}

impl AutomationFileSystem for FakeFileSystem {
    fn path_exists(&self, path: &str) -> Result<bool, FileSystemError> {
        self.queried.lock().unwrap().push(path.to_string());
        if let Some(reason) = &self.metadata_failure {
            return Err(FileSystemError::new(
                FileSystemOperation::Metadata,
                path,
                reason,
            ));
        }
        Ok(self
            .existing
            .lock()
            .unwrap()
            .iter()
            .any(|item| item == path))
    }

    fn create_dir_all(&self, path: &str) -> Result<(), FileSystemError> {
        self.created.lock().unwrap().push(path.to_string());
        if self.create_succeeds {
            Ok(())
        } else {
            Err(FileSystemError::new(
                FileSystemOperation::CreateDirectory,
                path,
                "permission denied",
            ))
        }
    }
}

#[test]
fn replace_state_normalizes_defaults_and_generates_only_missing_ids() {
    let store = MemoryStore::default();
    let ids = SequenceIds(Mutex::new(vec![
        "rule-generated".into(),
        "entry-generated".into(),
    ]));
    let service = AutomationRepositoryService::new(&store, &ids);

    service
        .replace_state(AutomationRepositoryInput {
            profiles: vec![],
            rules: vec![rule_input(
                json!({"id":"kept", "name":"Rule", "createdAt":-5}),
            )],
            processed_entries: vec![processed_input(
                json!({"filePath":"C:\\audio.wav", "size":-1}),
            )],
        })
        .unwrap();

    let state = store.state.lock().unwrap().clone();
    assert_eq!(state.rules[0].id, "kept");
    assert_eq!(state.rules[0].preset_id, "custom");
    assert_eq!(state.rules[0].stage_config.polish_preset_id, "general");
    assert_eq!(state.rules[0].export_config.format, "txt");
    assert_eq!(state.rules[0].created_at, -5);
    assert_eq!(state.processed_entries[0].id, "rule-generated");
    assert_eq!(state.processed_entries[0].size, -1);
    assert_eq!(store.calls.lock().unwrap().as_slice(), ["replace_state"]);
}

#[test]
fn rule_ids_generate_only_for_missing_values() {
    let store = MemoryStore::default();
    let ids = SequenceIds(Mutex::new(vec!["missing".into()]));
    let service = AutomationRepositoryService::new(&store, &ids);

    service
        .replace_rules(vec![rule_input(json!({})), rule_input(json!({"id": ""}))])
        .unwrap();

    let state = store.state.lock().unwrap();
    assert_eq!(state.rules[0].id, "missing");
    assert_eq!(state.rules[1].id, "");
}

#[test]
fn processed_ids_generate_when_missing() {
    let store = MemoryStore::default();
    let ids = SequenceIds(Mutex::new(vec!["entry-generated".into()]));

    AutomationRepositoryService::new(&store, &ids)
        .replace_processed_entries(vec![processed_input(json!({"filePath": "C:\\audio.wav"}))])
        .unwrap();

    assert_eq!(
        store.state.lock().unwrap().processed_entries[0].id,
        "entry-generated"
    );
}

#[test]
fn normalization_uses_all_designed_defaults() {
    let store = MemoryStore::default();
    let ids = SequenceIds(Mutex::new(vec!["rule-1".into(), "entry-1".into()]));

    AutomationRepositoryService::new(&store, &ids)
        .replace_state(AutomationRepositoryInput {
            profiles: vec![],
            rules: vec![rule_input(json!({}))],
            processed_entries: vec![processed_input(json!({}))],
        })
        .unwrap();

    let state = store.state.lock().unwrap();
    assert_eq!(
        serde_json::to_value(&state.rules[0]).unwrap(),
        json!({
            "id": "rule-1", "name": "", "kind": "file", "priority": 0,
            "profileSource": "tag_match", "saveHistory": true, "tagIds": [], "presetId": "custom",
            "watchDirectory": "", "recursive": false, "enabled": false,
            "stageConfig": {
                "autoPolish": false, "polishPresetId": "general", "autoTranslate": false,
                "translationLanguage": "en", "exportEnabled": false
            },
            "actions": {"autoPolish": false, "autoTranslate": false, "autoSummary": false},
            "exportConfig": {"directory": "", "format": "txt", "mode": "original", "prefix": ""},
            "createdAt": 0, "updatedAt": 0
        })
    );
    assert_eq!(
        serde_json::to_value(&state.processed_entries[0]).unwrap(),
        json!({
            "id": "entry-1", "ruleId": "", "kind": "file", "inputVersion": "",
            "attempt": 1, "filePath": "", "sourceFingerprint": "",
            "size": 0, "mtimeMs": 0, "status": "complete", "processedAt": 0
        })
    );
}

#[test]
fn optional_processed_fields_are_omitted_when_none() {
    let store = MemoryStore::default();
    let ids = SequenceIds(Mutex::new(vec![]));
    AutomationRepositoryService::new(&store, &ids)
        .replace_processed_entries(vec![processed_input(json!({"id": "entry-1"}))])
        .unwrap();

    let value = serde_json::to_value(&store.state.lock().unwrap().processed_entries[0]).unwrap();
    assert!(value.get("historyId").is_none());
    assert!(value.get("exportPath").is_none());
    assert!(value.get("errorMessage").is_none());
}

#[test]
fn replace_rules_calls_only_replace_rules() {
    let store = MemoryStore::default();
    let ids = SequenceIds(Mutex::new(vec![]));

    AutomationRepositoryService::new(&store, &ids)
        .replace_rules(vec![rule_input(json!({"id": "rule-1"}))])
        .unwrap();

    assert_eq!(store.calls.lock().unwrap().as_slice(), ["replace_rules"]);
}

#[test]
fn replace_processed_entries_calls_only_replace_processed_entries() {
    let store = MemoryStore::default();
    let ids = SequenceIds(Mutex::new(vec![]));

    AutomationRepositoryService::new(&store, &ids)
        .replace_processed_entries(vec![processed_input(json!({"id": "entry-1"}))])
        .unwrap();

    assert_eq!(
        store.calls.lock().unwrap().as_slice(),
        ["replace_processed_entries"]
    );
}

#[test]
fn load_state_returns_store_records_without_renormalizing() {
    let store = MemoryStore::default();
    store
        .state
        .lock()
        .unwrap()
        .rules
        .push(rule_record_with(|rule| {
            rule.preset_id.clear();
            rule.created_at = -9;
        }));
    let ids = SequenceIds(Mutex::new(vec![]));

    let state = AutomationRepositoryService::new(&store, &ids)
        .load_state()
        .unwrap();

    assert_eq!(state.rules[0].preset_id, "");
    assert_eq!(state.rules[0].created_at, -9);
    assert_eq!(store.calls.lock().unwrap().as_slice(), ["load_state"]);
}

#[test]
fn store_errors_preserve_repository_category() {
    let store = MemoryStore::default();
    *store.fail_with.lock().unwrap() = Some("storage unavailable".into());
    let ids = SequenceIds(Mutex::new(vec![]));

    assert_eq!(
        AutomationRepositoryService::new(&store, &ids).load_state(),
        Err(AutomationError::Repository(
            "storage unavailable".to_string()
        ))
    );
}

#[test]
fn full_state_normalizes_both_collections_before_calling_store() {
    let store = MemoryStore::default();
    let ids = SequenceIds(Mutex::new(vec!["rule-1".into()]));

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = AutomationRepositoryService::new(&store, &ids).replace_state(
            AutomationRepositoryInput {
                profiles: vec![],
                rules: vec![rule_input(json!({}))],
                processed_entries: vec![processed_input(json!({}))],
            },
        );
    }));

    assert!(result.is_err());
    assert!(store.calls.lock().unwrap().is_empty());
}

#[test]
fn validation_prepares_export_directory_only_after_safe_preconditions() {
    let fs = FakeFileSystem {
        existing: Mutex::new(vec!["C:\\watch".into(), "C:\\models\\batch".into()]),
        queried: Mutex::new(Vec::new()),
        created: Mutex::new(Vec::new()),
        create_succeeds: true,
        metadata_failure: None,
    };
    let service = AutomationValidationService::new(&fs);
    let rule = sample_rule("C:\\watch", "C:\\exports");
    let result = service
        .validate_rule_activation(
            &rule,
            &json!({"offlineModelPath":"C:\\models\\batch"}),
            &[json!({"id":"tag-1"})],
        )
        .unwrap();
    assert!(result.valid);
    assert_eq!(fs.created.lock().unwrap().as_slice(), ["C:\\exports"]);
}

#[test]
fn invalid_name_does_not_create_export_directory() {
    assert_precondition_invalid(
        |rule| rule.name.clear(),
        None,
        "automation.name_required",
        "Rule name is required.",
    );
}

#[test]
fn missing_tag_does_not_create_export_directory() {
    assert_precondition_invalid(
        |_| {},
        None,
        "automation.tag_missing",
        "One or more selected tags are missing.",
    );
}

#[test]
fn empty_watch_path_does_not_create_export_directory() {
    assert_precondition_invalid(
        |rule| rule.watch_directory.clear(),
        Some(json!({"id": "tag-1"})),
        "automation.watch_directory_required",
        "Choose a watch directory.",
    );
}

#[test]
fn empty_export_path_does_not_create_export_directory() {
    assert_precondition_invalid(
        |rule| rule.export_config.directory.clear(),
        Some(json!({"id": "tag-1"})),
        "automation.output_directory_required",
        "Choose an output directory.",
    );
}

#[test]
fn equal_normalized_paths_do_not_create_export_directory() {
    assert_precondition_invalid(
        |rule| {
            rule.watch_directory = " C:/WATCH/ ".into();
            rule.export_config.directory = "c:\\watch".into();
        },
        Some(json!({"id": "tag-1"})),
        "automation.same_directory",
        "Watch and output directories must be different.",
    );
}

#[test]
fn missing_watch_directory_does_not_create_export_directory() {
    let fs = FakeFileSystem {
        create_succeeds: true,
        ..FakeFileSystem::default()
    };
    let result = AutomationValidationService::new(&fs)
        .validate_rule_activation(
            &sample_rule("C:\\missing", "C:\\exports"),
            &online_config(),
            &[json!({"id":"tag-1"})],
        )
        .unwrap();

    assert_invalid(
        &result,
        "automation.watch_directory_missing",
        "The watch directory does not exist.",
    );
    assert!(fs.created.lock().unwrap().is_empty());
}

#[test]
fn export_directory_creation_failure_preserves_filesystem_error() {
    let fs = FakeFileSystem {
        existing: Mutex::new(vec!["C:\\watch".into()]),
        create_succeeds: false,
        ..FakeFileSystem::default()
    };
    let result = AutomationValidationService::new(&fs).validate_rule_activation(
        &sample_rule("C:\\watch", "C:\\exports"),
        &online_config(),
        &[json!({"id":"tag-1"})],
    );

    assert!(matches!(result, Err(AutomationError::FileSystem(_))));
    assert_eq!(fs.created.lock().unwrap().as_slice(), ["C:\\exports"]);
}

#[test]
fn existing_export_path_still_propagates_directory_creation_failure() {
    let fs = FakeFileSystem {
        existing: Mutex::new(vec!["C:\\watch".into(), "C:\\exports".into()]),
        create_succeeds: false,
        ..FakeFileSystem::default()
    };
    let result = AutomationValidationService::new(&fs).validate_rule_activation(
        &sample_rule("C:\\watch", "C:\\exports"),
        &online_config(),
        &[json!({"id":"tag-1"})],
    );

    assert!(matches!(result, Err(AutomationError::FileSystem(_))));
    assert_eq!(fs.created.lock().unwrap().as_slice(), ["C:\\exports"]);
}

#[test]
fn local_batch_model_path_existence_is_queried() {
    let fs = FakeFileSystem {
        existing: Mutex::new(vec!["C:\\watch".into(), "C:\\exports".into()]),
        create_succeeds: true,
        ..FakeFileSystem::default()
    };
    let result = AutomationValidationService::new(&fs)
        .validate_rule_activation(
            &sample_rule("C:\\watch", "C:\\exports"),
            &json!({"offlineModelPath": " C:\\models\\missing "}),
            &[json!({"id":"tag-1"})],
        )
        .unwrap();

    assert_invalid(
        &result,
        "automation.batch_model_missing",
        "A batch ASR model or online ASR credential is required before automation can run.",
    );
    assert!(
        fs.queried
            .lock()
            .unwrap()
            .contains(&"C:\\models\\missing".to_string())
    );
}

#[test]
fn filesystem_metadata_failure_is_not_reported_as_a_missing_path() {
    let fs = FakeFileSystem {
        metadata_failure: Some("access denied".into()),
        create_succeeds: true,
        ..FakeFileSystem::default()
    };

    let error = AutomationValidationService::new(&fs)
        .validate_rule_activation(
            &sample_rule("C:\\watch", "C:\\exports"),
            &online_config(),
            &[json!({"id":"tag-1"})],
        )
        .unwrap_err();

    assert!(matches!(
        error,
        AutomationError::FileSystem(FileSystemError {
            operation: FileSystemOperation::Metadata,
            ..
        })
    ));
}

#[test]
fn online_asr_does_not_require_a_local_model() {
    let fs = FakeFileSystem {
        existing: Mutex::new(vec!["C:\\watch".into(), "C:\\exports".into()]),
        create_succeeds: true,
        ..FakeFileSystem::default()
    };
    let result = AutomationValidationService::new(&fs)
        .validate_rule_activation(
            &sample_rule("C:\\watch", "C:\\exports"),
            &online_config(),
            &[json!({"id":"tag-1"})],
        )
        .unwrap();

    assert!(result.valid, "{result:?}");
    assert!(
        fs.queried
            .lock()
            .unwrap()
            .iter()
            .all(|path| !path.contains("models"))
    );
}

fn sample_rule(watch_directory: &str, export_directory: &str) -> AutomationRule {
    AutomationRule {
        name: "Rule".into(),
        save_history: true,
        tag_ids: vec!["tag-1".into()],
        watch_directory: watch_directory.into(),
        stage_config: AutomationRuleStageConfig {
            auto_polish: false,
            auto_translate: false,
        },
        export_config: AutomationRuleExportConfig {
            directory: export_directory.into(),
            mode: "original".into(),
        },
    }
}

fn online_config() -> Value {
    json!({
        "asr": {
            "selections": {"batch": {"engine": "online", "providerId": "groq-whisper"}},
            "providers": {"online": {"groq-whisper": {"apiKey": "key"}}}
        }
    })
}

fn assert_invalid(
    result: &sona_core::automation::AutomationRuleValidationResult,
    code: &str,
    message: &str,
) {
    assert!(!result.valid);
    assert_eq!(result.code.as_deref(), Some(code));
    assert_eq!(result.message.as_deref(), Some(message));
}

fn assert_precondition_invalid(
    mutate: impl FnOnce(&mut AutomationRule),
    tag: Option<Value>,
    code: &str,
    message: &str,
) {
    let fs = FakeFileSystem::default();
    let mut rule = sample_rule("C:\\watch", "C:\\exports");
    mutate(&mut rule);

    let tags = tag.into_iter().collect::<Vec<_>>();
    let result = AutomationValidationService::new(&fs)
        .validate_rule_activation(&rule, &online_config(), &tags)
        .unwrap();

    assert_invalid(&result, code, message);
    assert!(fs.created.lock().unwrap().is_empty());
}

fn rule_input(value: Value) -> AutomationRuleInput {
    serde_json::from_value(value).unwrap()
}

fn processed_input(value: Value) -> AutomationProcessedInput {
    serde_json::from_value(value).unwrap()
}

fn rule_record_with(overrides: impl FnOnce(&mut AutomationRuleRecord)) -> AutomationRuleRecord {
    let mut rule: AutomationRuleRecord = serde_json::from_value(json!({
        "id": "rule-1", "name": "Rule", "saveHistory": true, "tagIds": ["tag-1"], "presetId": "custom",
        "kind": "file", "priority": 0, "profileSource": "tag_match",
        "watchDirectory": "C:\\watch", "recursive": false, "enabled": false,
        "stageConfig": {
            "autoPolish": false, "polishPresetId": "general", "autoTranslate": false,
            "translationLanguage": "en", "exportEnabled": false
        },
        "exportConfig": {"directory": "C:\\exports", "format": "txt", "mode": "original", "prefix": ""},
        "actions": {"autoPolish": false, "autoTranslate": false, "autoSummary": false},
        "createdAt": 0, "updatedAt": 0
    }))
    .unwrap();
    overrides(&mut rule);
    rule
}
