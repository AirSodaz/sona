use crate::json_bridge::parse_core_json;
use crate::{SonaCoreBindingError, SonaCoreBindingResult};
use serde_json::Value;
use sona_runtime_fs::SystemClock;
use sona_sqlite::{Database, SqliteAppConfigAdapter};
use std::path::Path;
use std::sync::Arc;

pub(crate) fn load_app_config_json(app_data_dir: String) -> SonaCoreBindingResult<Option<String>> {
    with_app_config_adapter(&app_data_dir, |adapter| adapter.load_config())?
        .map(|config| serde_json::to_string(&config).map_err(config_repository_error))
        .transpose()
}

pub(crate) fn save_app_config_json(
    app_data_dir: String,
    config_json: String,
) -> SonaCoreBindingResult<()> {
    let config: Value = parse_core_json(&config_json, "app config")?;
    with_app_config_adapter(&app_data_dir, |adapter| adapter.save_config(&config))
}

pub(crate) fn get_app_setting_json(
    app_data_dir: String,
    key: String,
) -> SonaCoreBindingResult<Option<String>> {
    with_app_config_adapter(&app_data_dir, |adapter| adapter.get_setting(&key))?
        .map(|value| serde_json::to_string(&value).map_err(config_repository_error))
        .transpose()
}

pub(crate) fn set_app_setting_json(
    app_data_dir: String,
    key: String,
    value_json: String,
) -> SonaCoreBindingResult<()> {
    let value: Value = parse_core_json(&value_json, "app setting")?;
    with_app_config_adapter(&app_data_dir, |adapter| adapter.set_setting(&key, &value))
}

fn with_app_config_adapter<T, F>(app_data_dir: &str, operation: F) -> SonaCoreBindingResult<T>
where
    F: FnOnce(&SqliteAppConfigAdapter) -> Result<T, String>,
{
    let database = Database::open(Path::new(app_data_dir)).map_err(config_repository_error)?;
    let adapter = SqliteAppConfigAdapter::new(Arc::new(database), Arc::new(SystemClock));
    operation(&adapter).map_err(config_repository_error)
}

fn config_repository_error(reason: impl ToString) -> SonaCoreBindingError {
    SonaCoreBindingError::ConfigRepository {
        reason: reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        get_app_setting_json, load_app_config_json, save_app_config_json, set_app_setting_json,
    };
    use crate::{SonaCoreBindingError, SonaCoreFacade};
    use serde_json::{Value, json};
    use std::collections::HashSet;
    use std::fs;

    struct TestDir(tempfile::TempDir);

    impl TestDir {
        fn new() -> Self {
            Self(tempfile::tempdir().unwrap())
        }

        fn app_data_dir(&self) -> String {
            self.0.path().to_string_lossy().into_owned()
        }

        fn file_path(&self) -> String {
            let path = self.0.path().join("not-a-directory");
            fs::write(&path, "file").unwrap();
            path.to_string_lossy().into_owned()
        }
    }

    fn full_config() -> Value {
        json!({
            "sona-config": {
                "theme": "dark",
                "configVersion": 7,
                "summaryCustomTemplates": [
                    {"id":"duplicate","name":"Summary A","instructions":"A"},
                    {"id":"duplicate","name":"Summary B","instructions":"B"}
                ],
                "polishCustomPresets": [
                    {"id":"","name":"Polish","context":"Formal"}
                ],
                "textReplacementSets": [{
                    "id":"","name":"Replacement","enabled":true,"ignoreCase":true,
                    "rules":[
                        {"id":"same-rule","from":"a","to":"A"},
                        {"id":"same-rule","from":"b","to":"B"}
                    ]
                }],
                "hotwordSets": [{
                    "id":"","name":"Hotwords","enabled":true,
                    "rules":[{"id":"","text":"Sona"}]
                }],
                "polishKeywordSets": [{
                    "id":"","name":"Keywords","enabled":false,"keywords":"clear, concise"
                }],
                "speakerProfiles": [{
                    "id":"","name":"Alice","enabled":true,
                    "samples":[{
                        "id":"","filePath":"alice.wav","sourceName":"Alice sample",
                        "durationSeconds":12.5
                    }]
                }]
            }
        })
    }

    fn assert_non_empty_unique_ids(values: &[Value]) {
        let ids = values
            .iter()
            .map(|value| value["id"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert!(ids.iter().all(|id| !id.trim().is_empty()));
        assert_eq!(ids.iter().copied().collect::<HashSet<_>>().len(), ids.len());
    }

    #[test]
    fn empty_repository_loads_none() {
        let dir = TestDir::new();

        assert_eq!(load_app_config_json(dir.app_data_dir()).unwrap(), None);
    }

    #[test]
    fn save_and_load_round_trip_complete_config_and_repair_ids() {
        let dir = TestDir::new();

        save_app_config_json(dir.app_data_dir(), full_config().to_string()).unwrap();
        let loaded: Value =
            serde_json::from_str(&load_app_config_json(dir.app_data_dir()).unwrap().unwrap())
                .unwrap();
        let config = &loaded["sona-config"];

        assert_eq!(config["theme"], json!("dark"));
        assert_non_empty_unique_ids(config["summaryCustomTemplates"].as_array().unwrap());
        assert_eq!(
            config["summaryCustomTemplates"][1]["instructions"],
            json!("B")
        );
        assert_non_empty_unique_ids(config["polishCustomPresets"].as_array().unwrap());
        assert_eq!(config["polishCustomPresets"][0]["context"], json!("Formal"));
        assert_non_empty_unique_ids(config["textReplacementSets"].as_array().unwrap());
        assert_non_empty_unique_ids(
            config["textReplacementSets"][0]["rules"]
                .as_array()
                .unwrap(),
        );
        assert_eq!(config["textReplacementSets"][0]["ignoreCase"], json!(true));
        assert_eq!(
            config["textReplacementSets"][0]["rules"][1]["to"],
            json!("B")
        );
        assert_non_empty_unique_ids(config["hotwordSets"].as_array().unwrap());
        assert_non_empty_unique_ids(config["hotwordSets"][0]["rules"].as_array().unwrap());
        assert_eq!(config["hotwordSets"][0]["rules"][0]["text"], json!("Sona"));
        assert_non_empty_unique_ids(config["polishKeywordSets"].as_array().unwrap());
        assert_eq!(
            config["polishKeywordSets"][0]["keywords"],
            json!("clear, concise")
        );
        assert_non_empty_unique_ids(config["speakerProfiles"].as_array().unwrap());
        assert_non_empty_unique_ids(config["speakerProfiles"][0]["samples"].as_array().unwrap());
        assert_eq!(
            config["speakerProfiles"][0]["samples"][0]["filePath"],
            json!("alice.wav")
        );
        assert_eq!(
            config["speakerProfiles"][0]["samples"][0]["durationSeconds"],
            json!(12.5)
        );
    }

    #[test]
    fn settings_round_trip_null_objects_and_arrays() {
        let dir = TestDir::new();

        for (index, value) in [json!(null), json!({"nested": true}), json!([1, "two"])]
            .into_iter()
            .enumerate()
        {
            let key = format!("setting-{index}");
            assert_eq!(
                get_app_setting_json(dir.app_data_dir(), key.clone()).unwrap(),
                None
            );
            set_app_setting_json(dir.app_data_dir(), key.clone(), value.to_string()).unwrap();
            let loaded = get_app_setting_json(dir.app_data_dir(), key)
                .unwrap()
                .unwrap();
            assert_eq!(serde_json::from_str::<Value>(&loaded).unwrap(), value);
        }
    }

    #[test]
    fn malformed_json_is_rejected_before_database_open() {
        let dir = TestDir::new();
        let file_path = dir.file_path();

        for error in [
            save_app_config_json(file_path.clone(), "{".into()).unwrap_err(),
            set_app_setting_json(file_path, "key".into(), "{".into()).unwrap_err(),
        ] {
            assert!(matches!(error, SonaCoreBindingError::InvalidInput { .. }));
        }
    }

    #[test]
    fn database_open_errors_use_config_repository_variant() {
        let dir = TestDir::new();
        let file_path = dir.file_path();

        for error in [
            load_app_config_json(file_path.clone()).unwrap_err(),
            save_app_config_json(file_path.clone(), full_config().to_string()).unwrap_err(),
            get_app_setting_json(file_path.clone(), "key".into()).unwrap_err(),
            set_app_setting_json(file_path, "key".into(), json!(true).to_string()).unwrap_err(),
        ] {
            assert!(matches!(
                error,
                SonaCoreBindingError::ConfigRepository { .. }
            ));
        }
    }

    #[test]
    fn facade_delegates_all_repository_operations() {
        let dir = TestDir::new();
        let app_data_dir = dir.app_data_dir();

        SonaCoreFacade::save_app_config_json(app_data_dir.clone(), full_config().to_string())
            .unwrap();
        assert!(
            SonaCoreFacade::load_app_config_json(app_data_dir.clone())
                .unwrap()
                .is_some()
        );
        SonaCoreFacade::set_app_setting_json(
            app_data_dir.clone(),
            "facade".into(),
            json!({"ok": true}).to_string(),
        )
        .unwrap();
        assert_eq!(
            SonaCoreFacade::get_app_setting_json(app_data_dir, "facade".into()).unwrap(),
            Some(json!({"ok": true}).to_string())
        );
    }
}
