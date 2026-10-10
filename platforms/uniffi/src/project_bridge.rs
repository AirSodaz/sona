use crate::application_context::ContextSource;
use crate::{
    FfiProjectCreateInputV1, FfiProjectRecordV1, FfiProjectRepositorySnapshotV1,
    FfiProjectUpdateInputV1, SonaCoreBindingError, SonaCoreBindingResult,
};
use serde_json::Value;
#[cfg(test)]
use sona_core::ports::time::ClockError;
use sona_core::ports::time::ClockPort;
use sona_core::tag::{TagCreateInput, TagError, TagIdGenerator};
use sona_runtime_fs::{SystemClock, UuidGenerator};
use sona_sqlite::SqliteTagAdapter;
use std::sync::Arc;

pub(crate) fn load_project_repository_state_json(
    context: impl Into<ContextSource>,
) -> SonaCoreBindingResult<String> {
    with_project_adapter(
        context,
        Arc::new(UuidGenerator),
        Arc::new(SystemClock),
        |adapter| adapter.load_state(),
    )
    .and_then(serialize_project)
}

pub(crate) fn load_project_repository_v1(
    context: impl Into<ContextSource>,
) -> SonaCoreBindingResult<FfiProjectRepositorySnapshotV1> {
    with_project_adapter(
        context,
        Arc::new(UuidGenerator),
        Arc::new(SystemClock),
        |adapter| adapter.load_state(),
    )
    .map(Into::into)
}

pub(crate) fn replace_projects_json(
    context: impl Into<ContextSource>,
    projects_json: String,
) -> SonaCoreBindingResult<()> {
    let projects = parse_json_array("projects", &projects_json)?;
    with_project_input_adapter(
        context,
        Arc::new(UuidGenerator),
        Arc::new(SystemClock),
        |adapter| adapter.replace_tags_json(projects),
    )
}

pub(crate) fn replace_projects_v1(
    context: impl Into<ContextSource>,
    projects: Vec<FfiProjectRecordV1>,
) -> SonaCoreBindingResult<()> {
    let projects = projects
        .into_iter()
        .enumerate()
        .map(|(index, project)| {
            project.try_into().map_err(|reason: String| {
                invalid_input(format!("Invalid project at index {index}: {reason}"))
            })
        })
        .collect::<SonaCoreBindingResult<Vec<_>>>()?;
    with_project_adapter(
        context,
        Arc::new(UuidGenerator),
        Arc::new(SystemClock),
        |adapter| adapter.replace_tags(projects),
    )
}

pub(crate) fn create_project_json(
    context: impl Into<ContextSource>,
    input_json: String,
) -> SonaCoreBindingResult<String> {
    create_project_json_with_runtime(
        context,
        input_json,
        Arc::new(UuidGenerator),
        Arc::new(SystemClock),
    )
}

pub(crate) fn create_project_v1(
    context: impl Into<ContextSource>,
    input: FfiProjectCreateInputV1,
) -> SonaCoreBindingResult<FfiProjectRecordV1> {
    create_project_v1_with_runtime(
        context,
        input,
        Arc::new(UuidGenerator),
        Arc::new(SystemClock),
    )
}

pub(crate) fn update_project_json(
    context: impl Into<ContextSource>,
    project_id: String,
    updates_json: String,
) -> SonaCoreBindingResult<String> {
    update_project_json_with_clock(context, project_id, updates_json, Arc::new(SystemClock))
}

pub(crate) fn update_project_v1(
    context: impl Into<ContextSource>,
    project_id: String,
    updates: FfiProjectUpdateInputV1,
) -> SonaCoreBindingResult<Option<FfiProjectRecordV1>> {
    update_project_v1_with_clock(context, project_id, updates, Arc::new(SystemClock))
}

pub(crate) fn delete_project(
    context: impl Into<ContextSource>,
    project_id: String,
) -> SonaCoreBindingResult<()> {
    let project_id = parse_project_id("project ID", &project_id)?;
    with_project_adapter(
        context,
        Arc::new(UuidGenerator),
        Arc::new(SystemClock),
        |adapter| adapter.delete_project_with_cascade(&project_id, "moveToInbox"),
    )
}

pub(crate) fn delete_project_v1(
    context: impl Into<ContextSource>,
    project_id: String,
) -> SonaCoreBindingResult<()> {
    delete_project(context, project_id)
}

pub(crate) fn reorder_projects_json(
    context: impl Into<ContextSource>,
    project_ids_json: String,
) -> SonaCoreBindingResult<String> {
    let project_ids = parse_project_ids(&project_ids_json)?;
    with_project_adapter(
        context,
        Arc::new(UuidGenerator),
        Arc::new(SystemClock),
        |adapter| adapter.reorder_tags(project_ids),
    )
    .and_then(serialize_project)
}

pub(crate) fn reorder_projects_v1(
    context: impl Into<ContextSource>,
    project_ids: Vec<String>,
) -> SonaCoreBindingResult<Vec<FfiProjectRecordV1>> {
    let project_ids = normalize_project_ids(project_ids)?;
    with_project_adapter(
        context,
        Arc::new(UuidGenerator),
        Arc::new(SystemClock),
        |adapter| adapter.reorder_tags(project_ids),
    )
    .map(|tags| tags.into_iter().map(Into::into).collect())
}

pub(crate) fn set_active_project_id(
    context: impl Into<ContextSource>,
    project_id: Option<String>,
) -> SonaCoreBindingResult<()> {
    let project_id = project_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    with_project_adapter(
        context,
        Arc::new(UuidGenerator),
        Arc::new(SystemClock),
        |adapter| adapter.set_active_tag_id(project_id),
    )
}

pub(crate) fn set_active_project_id_v1(
    context: impl Into<ContextSource>,
    project_id: Option<String>,
) -> SonaCoreBindingResult<()> {
    set_active_project_id(context, project_id)
}

fn create_project_json_with_runtime(
    context: impl Into<ContextSource>,
    input_json: String,
    ids: Arc<dyn TagIdGenerator>,
    clock: Arc<dyn ClockPort>,
) -> SonaCoreBindingResult<String> {
    let input = parse_json_object_as::<TagCreateInput>("project input", &input_json)?;
    with_project_adapter(context, ids, clock, |adapter| adapter.create_tag(input))
        .and_then(serialize_project)
}

fn create_project_v1_with_runtime(
    context: impl Into<ContextSource>,
    input: FfiProjectCreateInputV1,
    ids: Arc<dyn TagIdGenerator>,
    clock: Arc<dyn ClockPort>,
) -> SonaCoreBindingResult<FfiProjectRecordV1> {
    with_project_adapter(context, ids, clock, |adapter| {
        adapter.create_tag(input.into())
    })
    .map(Into::into)
}

fn update_project_json_with_clock(
    context: impl Into<ContextSource>,
    project_id: String,
    updates_json: String,
    clock: Arc<dyn ClockPort>,
) -> SonaCoreBindingResult<String> {
    let project_id = parse_project_id("project ID", &project_id)?;
    let updates = parse_json_object("project updates", &updates_json)?;
    with_project_input_adapter(context, Arc::new(UuidGenerator), clock, |adapter| {
        adapter.update_tag_json(&project_id, updates)
    })
    .and_then(serialize_project)
}

fn update_project_v1_with_clock(
    context: impl Into<ContextSource>,
    project_id: String,
    updates: FfiProjectUpdateInputV1,
    clock: Arc<dyn ClockPort>,
) -> SonaCoreBindingResult<Option<FfiProjectRecordV1>> {
    let project_id = parse_project_id("project ID", &project_id)?;
    with_project_adapter(context, Arc::new(UuidGenerator), clock, |adapter| {
        adapter.update_tag(&project_id, updates.into())
    })
    .map(|project| project.map(Into::into))
}

fn with_project_adapter<T, F>(
    context: impl Into<ContextSource>,
    ids: Arc<dyn TagIdGenerator>,
    clock: Arc<dyn ClockPort>,
    operation: F,
) -> SonaCoreBindingResult<T>
where
    F: FnOnce(&SqliteTagAdapter) -> Result<T, TagError>,
{
    let context = context.into().resolve().map_err(project_error)?;
    let adapter = context.sqlite().tag_adapter(ids, clock);
    operation(&adapter).map_err(project_error)
}

fn with_project_input_adapter<T, F>(
    context: impl Into<ContextSource>,
    ids: Arc<dyn TagIdGenerator>,
    clock: Arc<dyn ClockPort>,
    operation: F,
) -> SonaCoreBindingResult<T>
where
    F: FnOnce(&SqliteTagAdapter) -> Result<T, TagError>,
{
    let context = context.into().resolve().map_err(project_error)?;
    let adapter = context.sqlite().tag_adapter(ids, clock);
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
    let values = parse_json_array("project IDs", input)?;
    let ids = values
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            value.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                invalid_input(format!(
                    "Invalid project IDs JSON: item {index} must be a string"
                ))
            })
        })
        .collect::<SonaCoreBindingResult<Vec<_>>>()?;
    normalize_project_ids(ids)
}

fn normalize_project_ids(project_ids: Vec<String>) -> SonaCoreBindingResult<Vec<String>> {
    project_ids
        .into_iter()
        .enumerate()
        .map(|(index, value)| parse_project_id(&format!("project ID at index {index}"), &value))
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

fn project_input_error(error: TagError) -> SonaCoreBindingError {
    match error {
        TagError::Serialization(source) => invalid_input(format!("Invalid project JSON: {source}")),
        error => project_error(error),
    }
}

#[cfg(test)]
fn create_project_json_at(
    context: impl Into<ContextSource>,
    input_json: String,
    id: &'static str,
    now_ms: u64,
) -> SonaCoreBindingResult<String> {
    struct FixedId(&'static str);
    impl TagIdGenerator for FixedId {
        fn generate_id(&self) -> String {
            self.0.to_string()
        }
    }

    create_project_json_with_runtime(
        context,
        input_json,
        Arc::new(FixedId(id)),
        Arc::new(FixedClock(now_ms)),
    )
}

#[cfg(test)]
fn create_project_v1_at(
    context: impl Into<ContextSource>,
    input: FfiProjectCreateInputV1,
    id: &'static str,
    now_ms: u64,
) -> SonaCoreBindingResult<FfiProjectRecordV1> {
    struct FixedId(&'static str);
    impl TagIdGenerator for FixedId {
        fn generate_id(&self) -> String {
            self.0.to_string()
        }
    }

    create_project_v1_with_runtime(
        context,
        input,
        Arc::new(FixedId(id)),
        Arc::new(FixedClock(now_ms)),
    )
}

#[cfg(test)]
fn update_project_json_at(
    context: impl Into<ContextSource>,
    project_id: String,
    updates_json: String,
    now_ms: u64,
) -> SonaCoreBindingResult<String> {
    update_project_json_with_clock(
        context,
        project_id,
        updates_json,
        Arc::new(FixedClock(now_ms)),
    )
}

#[cfg(test)]
struct FixedClock(u64);

#[cfg(test)]
impl ClockPort for FixedClock {
    fn now_ms(&self) -> Result<u64, ClockError> {
        Ok(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    fn context(dir: &tempfile::TempDir) -> String {
        dir.path().to_string_lossy().into_owned()
    }

    #[test]
    fn load_state_returns_empty_canonical_state() {
        let dir = tempfile::tempdir().unwrap();
        let output = load_project_repository_state_json(context(&dir)).unwrap();
        assert_eq!(output, r#"{"tags":[],"activeTagId":null}"#);
    }

    #[test]
    fn replace_projects_stores_exact_canonical_records() {
        let dir = tempfile::tempdir().unwrap();
        let projects = json!([
            {"id":"second","name":"Second","createdAt":2,"updatedAt":3},
            {"id":"first","name":"First","defaults":{"translationLanguage":"en"}}
        ]);

        replace_projects_json(context(&dir), projects.to_string()).unwrap();

        let state: Value = parse_json(
            "state",
            &load_project_repository_state_json(context(&dir)).unwrap(),
        )
        .unwrap();
        assert_eq!(state["tags"][0]["id"], "second");
        assert_eq!(state["tags"][1]["id"], "first");
        assert!(state["tags"][0].get("defaults").is_none());
        assert!(state["tags"][1].get("defaults").is_none());
    }

    #[test]
    fn create_generates_id_and_sets_equal_timestamps() {
        let dir = tempfile::tempdir().unwrap();
        let output = create_project_json_at(
            context(&dir),
            json!({"name":"Created"}).to_string(),
            "generated-id",
            42,
        )
        .unwrap();

        let project = parse_json("project", &output).unwrap();
        assert_eq!(project["id"], "generated-id");
        assert_eq!(project["createdAt"], 42);
        assert_eq!(project["updatedAt"], 42);
        assert_eq!(
            parse_json(
                "state",
                &load_project_repository_state_json(context(&dir)).unwrap()
            )
            .unwrap()["tags"][0],
            project
        );
    }

    #[test]
    fn v1_methods_roundtrip_typed_records() {
        let dir = tempfile::tempdir().unwrap();
        let created = create_project_v1_at(
            context(&dir),
            FfiProjectCreateInputV1 {
                name: "Work".to_string(),
                description: Some("Meetings".to_string()),
                icon: Some("briefcase".to_string()),
                color: Some("#123456".to_string()),
            },
            "project-1",
            100,
        )
        .unwrap();
        assert_eq!(created.id, "project-1");
        assert_eq!(created.name, "Work");
        assert_eq!(created.description, "Meetings");
        assert_eq!(created.icon, "briefcase");
        assert_eq!(created.color, "#123456");
        assert_eq!(created.sort_order, 0);
        assert_eq!(created.created_at, 100);
        assert_eq!(created.updated_at, 100);

        let updated = update_project_v1_with_clock(
            context(&dir),
            "project-1".to_string(),
            FfiProjectUpdateInputV1 {
                name: Some("Work Updated".to_string()),
                icon: None,
                color: None,
                description: Some("Updated description".to_string()),
            },
            Arc::new(FixedClock(200)),
        )
        .unwrap()
        .unwrap();
        assert_eq!(updated.name, "Work Updated");
        assert_eq!(updated.description, "Updated description");
        assert_eq!(updated.icon, "briefcase");
        assert_eq!(updated.color, "#123456");
        assert_eq!(updated.created_at, 100);
        assert_eq!(updated.updated_at, 200);

        let replacement = FfiProjectRecordV1 {
            id: "project-2".to_string(),
            name: "Personal".to_string(),
            description: String::new(),
            icon: "project".to_string(),
            color: "#abcdef".to_string(),
            sort_order: 0,
            created_at: 300,
            updated_at: 300,
        };
        replace_projects_v1(context(&dir), vec![replacement.clone()]).unwrap();
        let snapshot = load_project_repository_v1(context(&dir)).unwrap();
        assert_eq!(snapshot.projects, vec![replacement]);
        assert_eq!(snapshot.active_project_id, None);
    }

    #[test]
    fn update_persists_only_provided_fields_and_advances_timestamp() {
        let dir = tempfile::tempdir().unwrap();
        let output = create_project_json_at(
            context(&dir),
            json!({"name":"Before"}).to_string(),
            "project-1",
            10,
        )
        .unwrap();
        let original = parse_json("project", &output).unwrap();

        let updated_output = update_project_json_at(
            context(&dir),
            " project-1 ".to_string(),
            json!({"color":"#ffffff"}).to_string(),
            20,
        )
        .unwrap();
        let updated = parse_json("project", &updated_output).unwrap();

        assert_eq!(updated["name"], original["name"]);
        assert_eq!(updated["color"], "#ffffff");
        assert_eq!(updated["createdAt"], original["createdAt"]);
        assert_eq!(updated["updatedAt"], 20);
    }

    #[test]
    fn delete_returns_unit_and_removes_project() {
        let dir = tempfile::tempdir().unwrap();
        replace_projects_json(
            context(&dir),
            json!([{"id":"project-1","name":"Project"}]).to_string(),
        )
        .unwrap();

        delete_project(context(&dir), " project-1 ".to_string()).unwrap();

        let output = load_project_repository_state_json(context(&dir)).unwrap();
        assert_eq!(output, r#"{"tags":[],"activeTagId":null}"#);
    }

    #[test]
    fn reorder_returns_canonical_ordered_array() {
        let dir = tempfile::tempdir().unwrap();
        replace_projects_json(
            context(&dir),
            json!([
                {"id":"first","name":"First"},
                {"id":"second","name":"Second"}
            ])
            .to_string(),
        )
        .unwrap();

        let output =
            reorder_projects_json(context(&dir), json!([" second ", "first"]).to_string()).unwrap();

        assert_eq!(parse_json("reordered", &output).unwrap()[0]["id"], "second");
        assert_eq!(parse_json("reordered", &output).unwrap()[1]["id"], "first");
    }

    #[test]
    fn set_active_trims_ids_and_persists_null() {
        let dir = tempfile::tempdir().unwrap();

        set_active_project_id(context(&dir), Some(" project-1 ".to_string())).unwrap();
        let active = parse_json(
            "active",
            &load_project_repository_state_json(context(&dir)).unwrap(),
        )
        .unwrap();
        set_active_project_id(context(&dir), Some("   ".to_string())).unwrap();
        let cleared = parse_json(
            "cleared",
            &load_project_repository_state_json(context(&dir)).unwrap(),
        )
        .unwrap();

        assert_eq!(active["activeTagId"], "project-1");
        assert_eq!(cleared["activeTagId"], Value::Null);
    }

    #[test]
    fn malformed_payloads_are_invalid_input() {
        let dir = tempfile::tempdir().unwrap();
        let calls = [
            replace_projects_json(context(&dir), "{".to_string()),
            replace_projects_json(context(&dir), "{}".to_string()),
            create_project_json_at(context(&dir), "[]".to_string(), "id", 1).map(drop),
            create_project_json_at(context(&dir), "{}".to_string(), "id", 1).map(drop),
            update_project_json_at(context(&dir), "id".to_string(), "[]".to_string(), 1).map(drop),
            update_project_json_at(context(&dir), "   ".to_string(), "{}".to_string(), 1).map(drop),
            delete_project(context(&dir), "".to_string()),
            reorder_projects_json(context(&dir), "{}".to_string()).map(drop),
            reorder_projects_json(context(&dir), json!(["ok", 2]).to_string()).map(drop),
            reorder_projects_json(context(&dir), json!([" "]).to_string()).map(drop),
        ];

        for result in calls {
            assert!(matches!(
                result.unwrap_err(),
                SonaCoreBindingError::InvalidInput { .. }
            ));
        }

        assert!(matches!(
            reorder_projects_v1(context(&dir), vec![" ".to_string()]).unwrap_err(),
            SonaCoreBindingError::InvalidInput { .. }
        ));
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

        assert!(matches!(
            load_project_repository_v1(path_as_file.to_string_lossy().into_owned()).unwrap_err(),
            SonaCoreBindingError::Project { .. }
        ));
    }

    #[test]
    fn clock_failures_are_project_errors() {
        struct FailingClock;

        impl ClockPort for FailingClock {
            fn now_ms(&self) -> Result<u64, ClockError> {
                Err(ClockError::Unavailable("test clock failure".to_string()))
            }
        }

        let dir = tempfile::tempdir().unwrap();
        let error = super::create_project_json_with_runtime(
            context(&dir),
            json!({"name":"New", "defaults":{}}).to_string(),
            Arc::new(UuidGenerator),
            Arc::new(FailingClock),
        )
        .unwrap_err();

        assert!(matches!(error, SonaCoreBindingError::Project { .. }));

        let typed_error = super::create_project_v1_with_runtime(
            context(&dir),
            FfiProjectCreateInputV1 {
                name: "New".to_string(),
                description: None,
                icon: None,
                color: None,
            },
            Arc::new(UuidGenerator),
            Arc::new(FailingClock),
        )
        .unwrap_err();
        assert!(matches!(typed_error, SonaCoreBindingError::Project { .. }));
    }
}
