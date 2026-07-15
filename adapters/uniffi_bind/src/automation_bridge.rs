use crate::{SonaCoreBindingError, SonaCoreBindingResult};
use serde_json::Value;
use sona_core::automation::AutomationRule;
use sona_core::automation::service::AutomationValidationService;
use sona_runtime_fs::{NativeAutomationFileSystem, UuidGenerator};
use sona_sqlite::{Database, SqliteAutomationAdapter};
use std::path::Path;
use std::sync::Arc;

pub(crate) fn load_automation_repository_state_json(
    app_data_dir: String,
) -> SonaCoreBindingResult<String> {
    with_automation_adapter(&app_data_dir, |adapter| adapter.load_state())
        .and_then(serialize_automation)
}

pub(crate) fn replace_automation_rules_json(
    app_data_dir: String,
    rules_json: String,
) -> SonaCoreBindingResult<String> {
    let rules = parse_json_array("automation rules", &rules_json)?;
    with_automation_adapter(&app_data_dir, |adapter| {
        adapter.replace_rules_json(rules)?;
        adapter.load_state()
    })
    .and_then(serialize_automation)
}

pub(crate) fn replace_automation_processed_entries_json(
    app_data_dir: String,
    entries_json: String,
) -> SonaCoreBindingResult<String> {
    let entries = parse_json_array("automation processed entries", &entries_json)?;
    with_automation_adapter(&app_data_dir, |adapter| {
        adapter.replace_processed_entries_json(entries)?;
        adapter.load_state()
    })
    .and_then(serialize_automation)
}

pub(crate) fn replace_automation_repository_state_json(
    app_data_dir: String,
    state_json: String,
) -> SonaCoreBindingResult<String> {
    let (rules, processed_entries) = parse_repository_state(&state_json)?;
    with_automation_adapter(&app_data_dir, |adapter| {
        adapter.replace_state_json(rules, processed_entries)?;
        adapter.load_state()
    })
    .and_then(serialize_automation)
}

pub(crate) fn validate_automation_rule_activation_json(
    rule_json: String,
    global_config_json: String,
    project_json: Option<String>,
) -> SonaCoreBindingResult<String> {
    let rule = parse_json_object_as::<AutomationRule>("automation rule", &rule_json)?;
    let global_config = parse_json_object("global config", &global_config_json)?;
    let project = project_json
        .as_deref()
        .map(|json| parse_json_object("project", json))
        .transpose()?;
    let result = AutomationValidationService::new(&NativeAutomationFileSystem)
        .validate_rule_activation(&rule, &global_config, project.as_ref());
    serialize_automation(result)
}

fn with_automation_adapter<T, F>(app_data_dir: &str, operation: F) -> SonaCoreBindingResult<T>
where
    F: FnOnce(&SqliteAutomationAdapter) -> Result<T, String>,
{
    let database = Database::open(Path::new(app_data_dir)).map_err(automation_error)?;
    let adapter = SqliteAutomationAdapter::new(Arc::new(database), Arc::new(UuidGenerator));
    operation(&adapter).map_err(automation_error)
}

fn parse_json_array(label: &str, input: &str) -> SonaCoreBindingResult<Vec<Value>> {
    let value = parse_json(label, input)?;
    value
        .as_array()
        .cloned()
        .ok_or_else(|| invalid_input(format!("Invalid {label} JSON: expected an array")))
}

fn parse_repository_state(input: &str) -> SonaCoreBindingResult<(Vec<Value>, Vec<Value>)> {
    let value = parse_json_object("automation repository state", input)?;
    let rules = parse_state_array(&value, "rules")?;
    let processed_entries = parse_state_array(&value, "processedEntries")?;
    Ok((rules, processed_entries))
}

fn parse_state_array(state: &Value, field: &str) -> SonaCoreBindingResult<Vec<Value>> {
    match state.get(field) {
        None => Ok(Vec::new()),
        Some(Value::Array(values)) => Ok(values.clone()),
        Some(_) => Err(invalid_input(format!(
            "Invalid automation repository state JSON: {field} must be an array"
        ))),
    }
}

fn parse_json_object(label: &str, input: &str) -> SonaCoreBindingResult<Value> {
    let value = parse_json(label, input)?;
    if value.is_object() {
        Ok(value)
    } else {
        Err(invalid_input(format!(
            "Invalid {label} JSON: expected an object"
        )))
    }
}

fn parse_json_object_as<T>(label: &str, input: &str) -> SonaCoreBindingResult<T>
where
    T: serde::de::DeserializeOwned,
{
    let value = parse_json_object(label, input)?;
    serde_json::from_value(value)
        .map_err(|error| invalid_input(format!("Invalid {label} JSON: {error}")))
}

fn parse_json(label: &str, input: &str) -> SonaCoreBindingResult<Value> {
    serde_json::from_str(input)
        .map_err(|error| invalid_input(format!("Invalid {label} JSON: {error}")))
}

fn serialize_automation<T: serde::Serialize>(value: T) -> SonaCoreBindingResult<String> {
    serde_json::to_string(&value).map_err(automation_error)
}

fn invalid_input(reason: impl ToString) -> SonaCoreBindingError {
    SonaCoreBindingError::InvalidInput {
        reason: reason.to_string(),
    }
}

fn automation_error(reason: impl ToString) -> SonaCoreBindingError {
    SonaCoreBindingError::Automation {
        reason: reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        load_automation_repository_state_json, replace_automation_processed_entries_json,
        replace_automation_repository_state_json, replace_automation_rules_json,
        validate_automation_rule_activation_json,
    };
    use crate::SonaCoreBindingError;
    use serde_json::{Value, json};
    use std::fs;
    use std::path::Path;

    struct TestDir(tempfile::TempDir);

    impl TestDir {
        fn new() -> Self {
            Self(tempfile::tempdir().unwrap())
        }

        fn path(&self) -> &Path {
            self.0.path()
        }

        fn app_data_dir(&self) -> String {
            self.path().to_string_lossy().into_owned()
        }
    }

    fn parse_state(output: &str) -> Value {
        serde_json::from_str(output).unwrap()
    }

    #[test]
    fn load_returns_empty_canonical_state_json() {
        let dir = TestDir::new();

        let output = load_automation_repository_state_json(dir.app_data_dir()).unwrap();

        assert_eq!(output, r#"{"rules":[],"processedEntries":[]}"#);
    }

    #[test]
    fn replace_state_generates_ids_persists_and_returns_canonical_json() {
        let dir = TestDir::new();
        let output = replace_automation_repository_state_json(
            dir.app_data_dir(),
            json!({
                "rules": [{"name":"Rule"}],
                "processedEntries": [{"filePath":"C:\\audio.wav"}]
            })
            .to_string(),
        )
        .unwrap();
        let state = parse_state(&output);

        assert!(
            state["rules"][0]["id"]
                .as_str()
                .is_some_and(|id| !id.is_empty())
        );
        assert_eq!(state["rules"][0]["presetId"], "custom");
        assert!(state["processedEntries"][0]["id"].as_str().is_some());
        assert_eq!(
            parse_state(&load_automation_repository_state_json(dir.app_data_dir()).unwrap()),
            state
        );
    }

    #[test]
    fn rules_only_replacement_preserves_processed_entries() {
        let dir = TestDir::new();
        replace_automation_processed_entries_json(
            dir.app_data_dir(),
            json!([{"id":"entry-1","filePath":"C:\\audio.wav"}]).to_string(),
        )
        .unwrap();

        let output = replace_automation_rules_json(
            dir.app_data_dir(),
            json!([{"id":"rule-1","name":"Rule"}]).to_string(),
        )
        .unwrap();
        let state = parse_state(&output);

        assert_eq!(state["rules"][0]["id"], "rule-1");
        assert_eq!(state["processedEntries"][0]["id"], "entry-1");
    }

    #[test]
    fn processed_only_replacement_preserves_rules() {
        let dir = TestDir::new();
        replace_automation_rules_json(
            dir.app_data_dir(),
            json!([{"id":"rule-1","name":"Rule"}]).to_string(),
        )
        .unwrap();

        let output = replace_automation_processed_entries_json(
            dir.app_data_dir(),
            json!([{"id":"entry-1","filePath":"C:\\audio.wav"}]).to_string(),
        )
        .unwrap();
        let state = parse_state(&output);

        assert_eq!(state["rules"][0]["id"], "rule-1");
        assert_eq!(state["processedEntries"][0]["id"], "entry-1");
    }

    #[test]
    fn validation_creates_output_directory() {
        let dir = TestDir::new();
        let watch_directory = dir.path().join("watch");
        let output_directory = dir.path().join("output");
        let model_path = dir.path().join("model.onnx");
        fs::create_dir(&watch_directory).unwrap();
        fs::write(&model_path, b"model").unwrap();

        let output = validate_automation_rule_activation_json(
            rule_json(&watch_directory, &output_directory),
            json!({"offlineModelPath": model_path}).to_string(),
            None,
        )
        .unwrap();

        assert_eq!(parse_state(&output)["valid"], true);
        assert!(output_directory.is_dir());
    }

    #[test]
    fn malformed_json_and_incorrect_top_levels_are_invalid_input() {
        let dir = TestDir::new();
        let invalid_calls = [
            replace_automation_rules_json(dir.app_data_dir(), "{".to_string()),
            replace_automation_rules_json(dir.app_data_dir(), "{}".to_string()),
            replace_automation_processed_entries_json(dir.app_data_dir(), "[".to_string()),
            replace_automation_processed_entries_json(dir.app_data_dir(), "{}".to_string()),
            replace_automation_repository_state_json(dir.app_data_dir(), "{".to_string()),
            replace_automation_repository_state_json(dir.app_data_dir(), "[]".to_string()),
            validate_automation_rule_activation_json("{".to_string(), "{}".to_string(), None),
            validate_automation_rule_activation_json("[]".to_string(), "{}".to_string(), None),
            validate_automation_rule_activation_json("{}".to_string(), "{".to_string(), None),
            validate_automation_rule_activation_json("{}".to_string(), "[]".to_string(), None),
            validate_automation_rule_activation_json(
                "{}".to_string(),
                "{}".to_string(),
                Some("{".to_string()),
            ),
            validate_automation_rule_activation_json(
                "{}".to_string(),
                "{}".to_string(),
                Some("[]".to_string()),
            ),
        ];

        for result in invalid_calls {
            assert!(matches!(
                result.unwrap_err(),
                SonaCoreBindingError::InvalidInput { .. }
            ));
        }
    }

    #[test]
    fn app_data_path_that_is_a_file_maps_to_automation_error() {
        let dir = TestDir::new();
        let blocked = dir.path().join("blocked");
        fs::write(&blocked, b"not a directory").unwrap();

        let error = load_automation_repository_state_json(blocked.to_string_lossy().into_owned())
            .unwrap_err();

        assert!(matches!(error, SonaCoreBindingError::Automation { .. }));
    }

    fn rule_json(watch_directory: &Path, output_directory: &Path) -> String {
        json!({
            "name": "Rule",
            "projectId": "inbox",
            "watchDirectory": watch_directory,
            "stageConfig": {},
            "exportConfig": {
                "directory": output_directory,
                "mode": "original"
            }
        })
        .to_string()
    }
}
