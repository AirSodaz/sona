use std::sync::Arc;

use sona_core::ports::time::{ClockError, UnixMillisClock};
use sona_core::tag::{TagCreateInput, TagDefaultsInput, TagError, TagIdGenerator, TagListOptions};
use sona_sqlite::{Database, SqliteApplicationContext};

#[derive(Debug)]
struct FixedRuntime;

impl TagIdGenerator for FixedRuntime {
    fn generate_id(&self) -> String {
        "tag-from-context".to_string()
    }
}

impl UnixMillisClock for FixedRuntime {
    fn now_ms(&self) -> Result<u64, ClockError> {
        Ok(42)
    }
}

#[test]
fn context_normalizes_its_path_and_owns_the_injected_database() {
    let root = tempfile::tempdir().unwrap();
    let app_data_dir = root.path().join("app-data");
    std::fs::create_dir_all(app_data_dir.join("child")).unwrap();
    let database = Arc::new(Database::open(&app_data_dir).unwrap());
    let alias = app_data_dir.join("child").join("..");

    let context = SqliteApplicationContext::from_database(alias, Arc::clone(&database)).unwrap();

    assert_eq!(context.app_data_dir(), app_data_dir.canonicalize().unwrap());
    assert!(Arc::ptr_eq(&context.database(), &database));
}

#[test]
fn adapters_created_by_one_context_share_the_same_database_state() {
    let root = tempfile::tempdir().unwrap();
    let context = SqliteApplicationContext::open(root.path()).unwrap();

    context
        .tag_adapter(Arc::new(FixedRuntime), Arc::new(FixedRuntime))
        .create_tag(TagCreateInput {
            name: "Shared".to_string(),
            description: None,
            icon: None,
            color: Some("#2563EB".to_string()),
            defaults: TagDefaultsInput::default(),
        })
        .unwrap();

    let tags = context
        .tag_adapter(Arc::new(FixedRuntime), Arc::new(FixedRuntime))
        .list_tags(TagListOptions::default())
        .unwrap();
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].id, "tag-from-context");
    assert_eq!(tags[0].created_at, 42);
}

#[test]
fn read_only_context_reads_the_snapshot_and_rejects_writes() {
    let root = tempfile::tempdir().unwrap();
    let writer = SqliteApplicationContext::open(root.path()).unwrap();
    writer
        .tag_adapter(Arc::new(FixedRuntime), Arc::new(FixedRuntime))
        .create_tag(TagCreateInput {
            name: "Existing".to_string(),
            description: None,
            icon: None,
            color: None,
            defaults: TagDefaultsInput::default(),
        })
        .unwrap();

    let reader = SqliteApplicationContext::open_read_only(root.path()).unwrap();
    assert_eq!(
        reader
            .tag_adapter(Arc::new(FixedRuntime), Arc::new(FixedRuntime))
            .list_tags(TagListOptions::default())
            .unwrap()
            .len(),
        1
    );

    let error = reader
        .tag_adapter(Arc::new(FixedRuntime), Arc::new(FixedRuntime))
        .create_tag(TagCreateInput {
            name: "Rejected".to_string(),
            description: None,
            icon: None,
            color: None,
            defaults: TagDefaultsInput::default(),
        })
        .unwrap_err();
    assert!(matches!(error, TagError::Repository(_)));
}

#[test]
fn injected_database_must_belong_to_the_context_directory() {
    let database_root = tempfile::tempdir().unwrap();
    let requested_root = tempfile::tempdir().unwrap();
    let database = Arc::new(Database::open(database_root.path()).unwrap());

    let error =
        SqliteApplicationContext::from_database(requested_root.path().to_path_buf(), database)
            .unwrap_err();

    assert!(error.to_string().contains("does not belong"));
}
