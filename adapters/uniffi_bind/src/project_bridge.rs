use crate::{SonaCoreBindingError, SonaCoreBindingResult};
use serde_json::Value;
#[cfg(test)]
use sona_core::ports::time::ClockError;
use sona_core::ports::time::UnixMillisClock;
use sona_core::project::{ProjectCreateInput, ProjectError, ProjectIdGenerator};
use sona_runtime_fs::{SystemClock, UuidGenerator};
use sona_sqlite::{Database, SqliteProjectAdapter};
use std::path::Path;
use std::sync::Arc;

pub(crate) fn load_project_repository_state_json(
    app_data_dir: String,
) -> SonaCoreBindingResult<String> {
    with_project_adapter(
        &app_data_dir,
        Arc::new(UuidGenerator),
        Arc::new(SystemClock),
        |adapter| adapter.load_state(),
    )
    .and_then(serialize_project)
}

pub(crate) fn replace_projects_json(
    app_data_dir: String,
    projects_json: String,
) -> SonaCoreBindingResult<()> {
    let projects = parse_json_array("projects", &projects_json)?;
    with_project_input_adapter(
        &app_data_dir,
        Arc::new(UuidGenerator),
        Arc::new(SystemClock),
        |adapter| adapter.replace_projects_json(projects),
    )
}

pub(crate) fn create_project_json(
    app_data_dir: String,
    input_json: String,
) -> SonaCoreBindingResult<String> {
    create_project_json_with_runtime(
        app_data_dir,
        input_json,
        Arc::new(UuidGenerator),
        Arc::new(SystemClock),
    )
}

pub(crate) fn update_project_json(
    app_data_dir: String,
    project_id: String,
    updates_json: String,
) -> SonaCoreBindingResult<String> {
    update_project_json_with_clock(
        app_data_dir,
        project_id,
        updates_json,
        Arc::new(SystemClock),
    )
}

pub(crate) fn delete_project(
    app_data_dir: String,
    project_id: String,
) -> SonaCoreBindingResult<()> {
    let project_id = parse_project_id("project ID", &project_id)?;
    with_project_adapter(
        &app_data_dir,
        Arc::new(UuidGenerator),
        Arc::new(SystemClock),
        |adapter| adapter.delete_project(&project_id),
    )
}

pub(crate) fn reorder_projects_json(
    app_data_dir: String,
    project_ids_json: String,
) -> SonaCoreBindingResult<String> {
    let project_ids = parse_project_ids(&project_ids_json)?;
    with_project_adapter(
        &app_data_dir,
        Arc::new(UuidGenerator),
        Arc::new(SystemClock),
        |adapter| adapter.reorder_projects(project_ids),
    )
    .and_then(serialize_project)
}

pub(crate) fn set_active_project_id(
    app_data_dir: String,
    project_id: Option<String>,
) -> SonaCoreBindingResult<()> {
    let project_id = project_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    with_project_adapter(
        &app_data_dir,
        Arc::new(UuidGenerator),
        Arc::new(SystemClock),
        |adapter| adapter.set_active_project_id(project_id),
    )
}

fn create_project_json_with_runtime(
    app_data_dir: String,
    input_json: String,
    ids: Arc<dyn ProjectIdGenerator>,
    clock: Arc<dyn UnixMillisClock>,
) -> SonaCoreBindingResult<String> {
    let input = parse_json_object_as::<ProjectCreateInput>("project input", &input_json)?;
    with_project_adapter(&app_data_dir, ids, clock, |adapter| {
        adapter.create_project(input)
    })
    .and_then(serialize_project)
}

fn update_project_json_with_clock(
    app_data_dir: String,
    project_id: String,
    updates_json: String,
    clock: Arc<dyn UnixMillisClock>,
) -> SonaCoreBindingResult<String> {
    let project_id = parse_project_id("project ID", &project_id)?;
    let updates = parse_json_object("project updates", &updates_json)?;
    with_project_input_adapter(&app_data_dir, Arc::new(UuidGenerator), clock, |adapter| {
        adapter.update_project_json(&project_id, updates)
    })
    .and_then(serialize_project)
}

fn with_project_adapter<T, F>(
    app_data_dir: &str,
    ids: Arc<dyn ProjectIdGenerator>,
    clock: Arc<dyn UnixMillisClock>,
    operation: F,
) -> SonaCoreBindingResult<T>
where
    F: FnOnce(&SqliteProjectAdapter) -> Result<T, ProjectError>,
{
    let database = Database::open(Path::new(app_data_dir)).map_err(project_error)?;
    let adapter = SqliteProjectAdapter::new(Arc::new(database), ids, clock);
    operation(&adapter).map_err(project_error)
}

fn with_project_input_adapter<T, F>(
    app_data_dir: &str,
    ids: Arc<dyn ProjectIdGenerator>,
    clock: Arc<dyn UnixMillisClock>,
    operation: F,
) -> SonaCoreBindingResult<T>
where
    F: FnOnce(&SqliteProjectAdapter) -> Result<T, ProjectError>,
{
    let database = Database::open(Path::new(app_data_dir)).map_err(project_error)?;
    let adapter = SqliteProjectAdapter::new(Arc::new(database), ids, clock);
    operation(&adapter).map_err(project_input_error)
}

fn parse_json_array(label: &str, input: &str) -> SonaCoreBindingResult<Vec<Value>> {
    let value = parse_json(label, input)?;
    value
        .as_array()
        .cloned()
        .ok_or_else(|| invalid_input(format!("Invalid {label} JSON: expected an array")))
}

fn parse_project_ids(input: &str) -> SonaCoreBindingResult<Vec<String>> {
    parse_json_array("project IDs", input)?
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            let value = value.as_str().ok_or_else(|| {
                invalid_input(format!(
                    "Invalid project IDs JSON: item {index} must be a string"
                ))
            })?;
            parse_project_id(&format!("project ID at index {index}"), value)
        })
        .collect()
}

fn parse_project_id(label: &str, input: &str) -> SonaCoreBindingResult<String> {
    let project_id = input.trim();
    if project_id.is_empty() {
        Err(invalid_input(format!(
            "Invalid {label}: expected a non-empty string"
        )))
    } else {
        Ok(project_id.to_string())
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

fn serialize_project<T: serde::Serialize>(value: T) -> SonaCoreBindingResult<String> {
    serde_json::to_string(&value).map_err(project_error)
}

fn invalid_input(reason: impl ToString) -> SonaCoreBindingError {
    SonaCoreBindingError::InvalidInput {
        reason: reason.to_string(),
    }
}

fn project_error(reason: impl ToString) -> SonaCoreBindingError {
    SonaCoreBindingError::Project {
        reason: reason.to_string(),
    }
}

fn project_input_error(error: ProjectError) -> SonaCoreBindingError {
    match error {
        ProjectError::Serialization(source) => {
            invalid_input(format!("Invalid project JSON: {source}"))
        }
        error => project_error(error),
    }
}

#[cfg(test)]
fn create_project_json_at(
    app_data_dir: String,
    input_json: String,
    id: &'static str,
    now_ms: u64,
) -> SonaCoreBindingResult<String> {
    struct FixedId(&'static str);
    impl ProjectIdGenerator for FixedId {
        fn generate_id(&self) -> String {
            self.0.to_string()
        }
    }

    create_project_json_with_runtime(
        app_data_dir,
        input_json,
        Arc::new(FixedId(id)),
        Arc::new(FixedClock(now_ms)),
    )
}

#[cfg(test)]
fn update_project_json_at(
    app_data_dir: String,
    project_id: String,
    updates_json: String,
    now_ms: u64,
) -> SonaCoreBindingResult<String> {
    update_project_json_with_clock(
        app_data_dir,
        project_id,
        updates_json,
        Arc::new(FixedClock(now_ms)),
    )
}

#[cfg(test)]
struct FixedClock(u64);

#[cfg(test)]
impl UnixMillisClock for FixedClock {
    fn now_ms(&self) -> Result<u64, ClockError> {
        Ok(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        create_project_json_at, delete_project, load_project_repository_state_json,
        reorder_projects_json, replace_projects_json, set_active_project_id,
        update_project_json_at,
    };
    use crate::SonaCoreBindingError;
    use serde_json::{Value, json};
    use sona_core::ports::time::{ClockError, UnixMillisClock};
    use sona_runtime_fs::UuidGenerator;
    use std::fs;
    use std::sync::Arc;

    fn app_data_dir(dir: &tempfile::TempDir) -> String {
        dir.path().to_string_lossy().into_owned()
    }

    fn parse_json(output: &str) -> Value {
        serde_json::from_str(output).unwrap()
    }

    #[test]
    fn load_returns_empty_canonical_state() {
        let dir = tempfile::tempdir().unwrap();

        let output = load_project_repository_state_json(app_data_dir(&dir)).unwrap();

        assert_eq!(output, r#"{"projects":[],"activeProjectId":null}"#);
    }

    #[test]
    fn replace_persists_canonical_defaults_and_order_and_returns_unit() {
        let dir = tempfile::tempdir().unwrap();
        let projects = json!([
            {"id":"second","name":"Second","createdAt":2,"updatedAt":3},
            {"id":"first","name":"First","defaults":{"translationLanguage":"en"}}
        ]);

        replace_projects_json(app_data_dir(&dir), projects.to_string()).unwrap();
        let state = parse_json(&load_project_repository_state_json(app_data_dir(&dir)).unwrap());

        assert_eq!(state["projects"][0]["id"], "second");
        assert_eq!(state["projects"][1]["id"], "first");
        assert_eq!(
            state["projects"][0]["defaults"]["summaryTemplateId"],
            "general"
        );
        assert_eq!(
            state["projects"][0]["defaults"]["translationLanguage"],
            "zh"
        );
        assert_eq!(
            state["projects"][1]["defaults"]["translationLanguage"],
            "en"
        );
    }

    #[test]
    fn create_generates_id_and_timestamps_and_returns_record() {
        let dir = tempfile::tempdir().unwrap();

        let output = create_project_json_at(
            app_data_dir(&dir),
            json!({"name":"New","defaults":{}}).to_string(),
            "generated-id",
            42,
        )
        .unwrap();
        let project = parse_json(&output);

        assert_eq!(project["id"], "generated-id");
        assert_eq!(project["createdAt"], 42);
        assert_eq!(project["updatedAt"], 42);
        assert_eq!(
            parse_json(&load_project_repository_state_json(app_data_dir(&dir)).unwrap())["projects"]
                [0],
            project
        );
    }

    #[test]
    fn update_returns_canonical_nullable_record_json() {
        let dir = tempfile::tempdir().unwrap();
        replace_projects_json(
            app_data_dir(&dir),
            json!([{"id":"project-1","name":"Before"}]).to_string(),
        )
        .unwrap();

        let updated = update_project_json_at(
            app_data_dir(&dir),
            " project-1 ".to_string(),
            json!({"name":"After"}).to_string(),
            99,
        )
        .unwrap();
        let missing = update_project_json_at(
            app_data_dir(&dir),
            "missing".to_string(),
            json!({"name":"Ignored"}).to_string(),
            100,
        )
        .unwrap();

        assert_eq!(parse_json(&updated)["name"], "After");
        assert_eq!(parse_json(&updated)["updatedAt"], 99);
        assert_eq!(missing, "null");
    }

    #[test]
    fn delete_returns_unit_and_removes_project() {
        let dir = tempfile::tempdir().unwrap();
        replace_projects_json(
            app_data_dir(&dir),
            json!([{"id":"project-1","name":"Project"}]).to_string(),
        )
        .unwrap();

        delete_project(app_data_dir(&dir), " project-1 ".to_string()).unwrap();

        assert_eq!(
            load_project_repository_state_json(app_data_dir(&dir)).unwrap(),
            r#"{"projects":[],"activeProjectId":null}"#
        );
    }

    #[test]
    fn reorder_returns_canonical_ordered_array() {
        let dir = tempfile::tempdir().unwrap();
        replace_projects_json(
            app_data_dir(&dir),
            json!([
                {"id":"first","name":"First"},
                {"id":"second","name":"Second"}
            ])
            .to_string(),
        )
        .unwrap();

        let output =
            reorder_projects_json(app_data_dir(&dir), json!([" second ", "first"]).to_string())
                .unwrap();

        assert_eq!(parse_json(&output)[0]["id"], "second");
        assert_eq!(parse_json(&output)[1]["id"], "first");
    }

    #[test]
    fn set_active_trims_ids_and_persists_null() {
        let dir = tempfile::tempdir().unwrap();

        set_active_project_id(app_data_dir(&dir), Some(" project-1 ".to_string())).unwrap();
        let active = parse_json(&load_project_repository_state_json(app_data_dir(&dir)).unwrap());
        set_active_project_id(app_data_dir(&dir), Some("   ".to_string())).unwrap();
        let cleared = parse_json(&load_project_repository_state_json(app_data_dir(&dir)).unwrap());

        assert_eq!(active["activeProjectId"], "project-1");
        assert_eq!(cleared["activeProjectId"], Value::Null);
    }

    #[test]
    fn malformed_payloads_are_invalid_input() {
        let dir = tempfile::tempdir().unwrap();
        let calls = [
            replace_projects_json(app_data_dir(&dir), "{".to_string()),
            replace_projects_json(app_data_dir(&dir), "{}".to_string()),
            create_project_json_at(app_data_dir(&dir), "[]".to_string(), "id", 1).map(drop),
            create_project_json_at(app_data_dir(&dir), "{}".to_string(), "id", 1).map(drop),
            update_project_json_at(app_data_dir(&dir), "id".to_string(), "[]".to_string(), 1)
                .map(drop),
            update_project_json_at(app_data_dir(&dir), "   ".to_string(), "{}".to_string(), 1)
                .map(drop),
            delete_project(app_data_dir(&dir), "".to_string()),
            reorder_projects_json(app_data_dir(&dir), "{}".to_string()).map(drop),
            reorder_projects_json(app_data_dir(&dir), json!(["ok", 2]).to_string()).map(drop),
            reorder_projects_json(app_data_dir(&dir), json!([" "]).to_string()).map(drop),
        ];

        for result in calls {
            assert!(matches!(
                result.unwrap_err(),
                SonaCoreBindingError::InvalidInput { .. }
            ));
        }
    }

    #[test]
    fn filesystem_and_database_failures_are_project_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path_as_file = dir.path().join("blocked");
        fs::write(&path_as_file, b"not a directory").unwrap();
        let corrupt_dir = dir.path().join("corrupt");
        fs::create_dir(&corrupt_dir).unwrap();
        fs::write(corrupt_dir.join("sona.db"), b"not sqlite").unwrap();

        for error in [
            load_project_repository_state_json(path_as_file.to_string_lossy().into_owned())
                .unwrap_err(),
            load_project_repository_state_json(corrupt_dir.to_string_lossy().into_owned())
                .unwrap_err(),
        ] {
            assert!(matches!(error, SonaCoreBindingError::Project { .. }));
        }
    }

    #[test]
    fn clock_failures_are_project_errors() {
        struct FailingClock;

        impl UnixMillisClock for FailingClock {
            fn now_ms(&self) -> Result<u64, ClockError> {
                Err(ClockError::Unavailable("test clock failure".to_string()))
            }
        }

        let dir = tempfile::tempdir().unwrap();
        let error = super::create_project_json_with_runtime(
            app_data_dir(&dir),
            json!({"name":"New", "defaults":{}}).to_string(),
            Arc::new(UuidGenerator),
            Arc::new(FailingClock),
        )
        .unwrap_err();

        assert!(matches!(error, SonaCoreBindingError::Project { .. }));
    }
}
