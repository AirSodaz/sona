use crate::{SonaCoreBindingError, SonaCoreBindingResult};
use serde_json::Value;
#[cfg(test)]
use sona_core::ports::time::ClockError;
use sona_core::ports::time::UnixMillisClock;
use sona_core::tag::{TagCreateInput, TagError, TagIdGenerator};
use sona_runtime_fs::{SystemClock, UuidGenerator};
use sona_sqlite::{Database, SqliteTagAdapter};
use std::path::Path;
use std::sync::Arc;

pub(crate) fn load_tag_repository_state_json(
    app_data_dir: String,
) -> SonaCoreBindingResult<String> {
    with_tag_adapter(
        &app_data_dir,
        Arc::new(UuidGenerator),
        Arc::new(SystemClock),
        |adapter| adapter.load_state(),
    )
    .and_then(serialize_tag)
}

pub(crate) fn replace_tags_json(
    app_data_dir: String,
    tags_json: String,
) -> SonaCoreBindingResult<()> {
    let tags = parse_json_array("tags", &tags_json)?;
    with_tag_input_adapter(
        &app_data_dir,
        Arc::new(UuidGenerator),
        Arc::new(SystemClock),
        |adapter| adapter.replace_tags_json(tags),
    )
}

pub(crate) fn create_tag_json(
    app_data_dir: String,
    input_json: String,
) -> SonaCoreBindingResult<String> {
    create_tag_json_with_runtime(
        app_data_dir,
        input_json,
        Arc::new(UuidGenerator),
        Arc::new(SystemClock),
    )
}

pub(crate) fn update_tag_json(
    app_data_dir: String,
    tag_id: String,
    updates_json: String,
) -> SonaCoreBindingResult<String> {
    update_tag_json_with_clock(app_data_dir, tag_id, updates_json, Arc::new(SystemClock))
}

pub(crate) fn delete_tag(app_data_dir: String, tag_id: String) -> SonaCoreBindingResult<()> {
    let tag_id = parse_tag_id("tag ID", &tag_id)?;
    with_tag_adapter(
        &app_data_dir,
        Arc::new(UuidGenerator),
        Arc::new(SystemClock),
        |adapter| adapter.delete_tag(&tag_id),
    )
}

pub(crate) fn reorder_tags_json(
    app_data_dir: String,
    tag_ids_json: String,
) -> SonaCoreBindingResult<String> {
    let tag_ids = parse_tag_ids(&tag_ids_json)?;
    with_tag_adapter(
        &app_data_dir,
        Arc::new(UuidGenerator),
        Arc::new(SystemClock),
        |adapter| adapter.reorder_tags(tag_ids),
    )
    .and_then(serialize_tag)
}

pub(crate) fn set_active_tag_id(
    app_data_dir: String,
    tag_id: Option<String>,
) -> SonaCoreBindingResult<()> {
    let tag_id = tag_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    with_tag_adapter(
        &app_data_dir,
        Arc::new(UuidGenerator),
        Arc::new(SystemClock),
        |adapter| adapter.set_active_tag_id(tag_id),
    )
}

fn create_tag_json_with_runtime(
    app_data_dir: String,
    input_json: String,
    ids: Arc<dyn TagIdGenerator>,
    clock: Arc<dyn UnixMillisClock>,
) -> SonaCoreBindingResult<String> {
    let input = parse_json_object_as::<TagCreateInput>("tag input", &input_json)?;
    with_tag_adapter(&app_data_dir, ids, clock, |adapter| {
        adapter.create_tag(input)
    })
    .and_then(serialize_tag)
}

fn update_tag_json_with_clock(
    app_data_dir: String,
    tag_id: String,
    updates_json: String,
    clock: Arc<dyn UnixMillisClock>,
) -> SonaCoreBindingResult<String> {
    let tag_id = parse_tag_id("tag ID", &tag_id)?;
    let updates = parse_json_object("tag updates", &updates_json)?;
    with_tag_input_adapter(&app_data_dir, Arc::new(UuidGenerator), clock, |adapter| {
        adapter.update_tag_json(&tag_id, updates)
    })
    .and_then(serialize_tag)
}

fn with_tag_adapter<T, F>(
    app_data_dir: &str,
    ids: Arc<dyn TagIdGenerator>,
    clock: Arc<dyn UnixMillisClock>,
    operation: F,
) -> SonaCoreBindingResult<T>
where
    F: FnOnce(&SqliteTagAdapter) -> Result<T, TagError>,
{
    let database = Database::open(Path::new(app_data_dir)).map_err(tag_error)?;
    let adapter = SqliteTagAdapter::new(Arc::new(database), ids, clock);
    operation(&adapter).map_err(tag_error)
}

fn with_tag_input_adapter<T, F>(
    app_data_dir: &str,
    ids: Arc<dyn TagIdGenerator>,
    clock: Arc<dyn UnixMillisClock>,
    operation: F,
) -> SonaCoreBindingResult<T>
where
    F: FnOnce(&SqliteTagAdapter) -> Result<T, TagError>,
{
    let database = Database::open(Path::new(app_data_dir)).map_err(tag_error)?;
    let adapter = SqliteTagAdapter::new(Arc::new(database), ids, clock);
    operation(&adapter).map_err(tag_input_error)
}

fn parse_json_array(label: &str, input: &str) -> SonaCoreBindingResult<Vec<Value>> {
    let value = parse_json(label, input)?;
    value
        .as_array()
        .cloned()
        .ok_or_else(|| invalid_input(format!("Invalid {label} JSON: expected an array")))
}

fn parse_tag_ids(input: &str) -> SonaCoreBindingResult<Vec<String>> {
    parse_json_array("tag IDs", input)?
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            let value = value.as_str().ok_or_else(|| {
                invalid_input(format!(
                    "Invalid tag IDs JSON: item {index} must be a string"
                ))
            })?;
            parse_tag_id(&format!("tag ID at index {index}"), value)
        })
        .collect()
}

fn parse_tag_id(label: &str, input: &str) -> SonaCoreBindingResult<String> {
    let tag_id = input.trim();
    if tag_id.is_empty() {
        Err(invalid_input(format!(
            "Invalid {label}: expected a non-empty string"
        )))
    } else {
        Ok(tag_id.to_string())
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

fn serialize_tag<T: serde::Serialize>(value: T) -> SonaCoreBindingResult<String> {
    serde_json::to_string(&value).map_err(tag_error)
}

fn invalid_input(reason: impl ToString) -> SonaCoreBindingError {
    SonaCoreBindingError::InvalidInput {
        reason: reason.to_string(),
    }
}

fn tag_error(reason: impl ToString) -> SonaCoreBindingError {
    SonaCoreBindingError::Tag {
        reason: reason.to_string(),
    }
}

fn tag_input_error(error: TagError) -> SonaCoreBindingError {
    match error {
        TagError::Serialization(source) => invalid_input(format!("Invalid tag JSON: {source}")),
        error => tag_error(error),
    }
}

#[cfg(test)]
fn create_tag_json_at(
    app_data_dir: String,
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

    create_tag_json_with_runtime(
        app_data_dir,
        input_json,
        Arc::new(FixedId(id)),
        Arc::new(FixedClock(now_ms)),
    )
}

#[cfg(test)]
fn update_tag_json_at(
    app_data_dir: String,
    tag_id: String,
    updates_json: String,
    now_ms: u64,
) -> SonaCoreBindingResult<String> {
    update_tag_json_with_clock(
        app_data_dir,
        tag_id,
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
        create_tag_json_at, delete_tag, load_tag_repository_state_json, reorder_tags_json,
        replace_tags_json, set_active_tag_id, update_tag_json_at,
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

        let output = load_tag_repository_state_json(app_data_dir(&dir)).unwrap();

        assert_eq!(output, r#"{"tags":[],"activeTagId":null}"#);
    }

    #[test]
    fn replace_persists_canonical_defaults_and_order_and_returns_unit() {
        let dir = tempfile::tempdir().unwrap();
        let tags = json!([
            {"id":"second","name":"Second","createdAt":2,"updatedAt":3},
            {"id":"first","name":"First","defaults":{"translationLanguage":"en"}}
        ]);

        replace_tags_json(app_data_dir(&dir), tags.to_string()).unwrap();
        let state = parse_json(&load_tag_repository_state_json(app_data_dir(&dir)).unwrap());

        assert_eq!(state["tags"][0]["id"], "second");
        assert_eq!(state["tags"][1]["id"], "first");
        assert_eq!(state["tags"][0]["defaults"]["summaryTemplateId"], "general");
        assert_eq!(state["tags"][0]["defaults"]["translationLanguage"], "zh");
        assert_eq!(state["tags"][1]["defaults"]["translationLanguage"], "en");
    }

    #[test]
    fn create_generates_id_and_timestamps_and_returns_record() {
        let dir = tempfile::tempdir().unwrap();

        let output = create_tag_json_at(
            app_data_dir(&dir),
            json!({"name":"New","defaults":{}}).to_string(),
            "generated-id",
            42,
        )
        .unwrap();
        let tag = parse_json(&output);

        assert_eq!(tag["id"], "generated-id");
        assert_eq!(tag["createdAt"], 42);
        assert_eq!(tag["updatedAt"], 42);
        assert_eq!(
            parse_json(&load_tag_repository_state_json(app_data_dir(&dir)).unwrap())["tags"][0],
            tag
        );
    }

    #[test]
    fn update_returns_canonical_nullable_record_json() {
        let dir = tempfile::tempdir().unwrap();
        replace_tags_json(
            app_data_dir(&dir),
            json!([{"id":"tag-1","name":"Before"}]).to_string(),
        )
        .unwrap();

        let updated = update_tag_json_at(
            app_data_dir(&dir),
            " tag-1 ".to_string(),
            json!({"name":"After"}).to_string(),
            99,
        )
        .unwrap();
        let missing = update_tag_json_at(
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
    fn delete_returns_unit_and_removes_tag() {
        let dir = tempfile::tempdir().unwrap();
        replace_tags_json(
            app_data_dir(&dir),
            json!([{"id":"tag-1","name":"Tag"}]).to_string(),
        )
        .unwrap();

        delete_tag(app_data_dir(&dir), " tag-1 ".to_string()).unwrap();

        assert_eq!(
            load_tag_repository_state_json(app_data_dir(&dir)).unwrap(),
            r#"{"tags":[],"activeTagId":null}"#
        );
    }

    #[test]
    fn reorder_returns_canonical_ordered_array() {
        let dir = tempfile::tempdir().unwrap();
        replace_tags_json(
            app_data_dir(&dir),
            json!([
                {"id":"first","name":"First"},
                {"id":"second","name":"Second"}
            ])
            .to_string(),
        )
        .unwrap();

        let output =
            reorder_tags_json(app_data_dir(&dir), json!([" second ", "first"]).to_string())
                .unwrap();

        assert_eq!(parse_json(&output)[0]["id"], "second");
        assert_eq!(parse_json(&output)[1]["id"], "first");
    }

    #[test]
    fn set_active_trims_ids_and_persists_null() {
        let dir = tempfile::tempdir().unwrap();

        set_active_tag_id(app_data_dir(&dir), Some(" tag-1 ".to_string())).unwrap();
        let active = parse_json(&load_tag_repository_state_json(app_data_dir(&dir)).unwrap());
        set_active_tag_id(app_data_dir(&dir), Some("   ".to_string())).unwrap();
        let cleared = parse_json(&load_tag_repository_state_json(app_data_dir(&dir)).unwrap());

        assert_eq!(active["activeTagId"], "tag-1");
        assert_eq!(cleared["activeTagId"], Value::Null);
    }

    #[test]
    fn malformed_payloads_are_invalid_input() {
        let dir = tempfile::tempdir().unwrap();
        let calls = [
            replace_tags_json(app_data_dir(&dir), "{".to_string()),
            replace_tags_json(app_data_dir(&dir), "{}".to_string()),
            create_tag_json_at(app_data_dir(&dir), "[]".to_string(), "id", 1).map(drop),
            create_tag_json_at(app_data_dir(&dir), "{}".to_string(), "id", 1).map(drop),
            update_tag_json_at(app_data_dir(&dir), "id".to_string(), "[]".to_string(), 1).map(drop),
            update_tag_json_at(app_data_dir(&dir), "   ".to_string(), "{}".to_string(), 1)
                .map(drop),
            delete_tag(app_data_dir(&dir), "".to_string()),
            reorder_tags_json(app_data_dir(&dir), "{}".to_string()).map(drop),
            reorder_tags_json(app_data_dir(&dir), json!(["ok", 2]).to_string()).map(drop),
            reorder_tags_json(app_data_dir(&dir), json!([" "]).to_string()).map(drop),
        ];

        for result in calls {
            assert!(matches!(
                result.unwrap_err(),
                SonaCoreBindingError::InvalidInput { .. }
            ));
        }
    }

    #[test]
    fn filesystem_and_database_failures_are_tag_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path_as_file = dir.path().join("blocked");
        fs::write(&path_as_file, b"not a directory").unwrap();
        let corrupt_dir = dir.path().join("corrupt");
        fs::create_dir(&corrupt_dir).unwrap();
        fs::write(corrupt_dir.join("sona.db"), b"not sqlite").unwrap();

        for error in [
            load_tag_repository_state_json(path_as_file.to_string_lossy().into_owned())
                .unwrap_err(),
            load_tag_repository_state_json(corrupt_dir.to_string_lossy().into_owned()).unwrap_err(),
        ] {
            assert!(matches!(error, SonaCoreBindingError::Tag { .. }));
        }
    }

    #[test]
    fn clock_failures_are_tag_errors() {
        struct FailingClock;

        impl UnixMillisClock for FailingClock {
            fn now_ms(&self) -> Result<u64, ClockError> {
                Err(ClockError::Unavailable("test clock failure".to_string()))
            }
        }

        let dir = tempfile::tempdir().unwrap();
        let error = super::create_tag_json_with_runtime(
            app_data_dir(&dir),
            json!({"name":"New", "defaults":{}}).to_string(),
            Arc::new(UuidGenerator),
            Arc::new(FailingClock),
        )
        .unwrap_err();

        assert!(matches!(error, SonaCoreBindingError::Tag { .. }));
    }
}
