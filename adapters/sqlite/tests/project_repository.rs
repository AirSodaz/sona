use std::sync::Arc;

use sona_core::ports::time::{ClockError, ClockPort};
use sona_core::project::ports::ProjectRepository;
use sona_core::project::{ProjectCreateInput, ProjectPipelineConfig};
use sona_core::tag::{TagError, TagIdGenerator, TagRecord, TagStore};
use sona_sqlite::{Database, SqliteProjectAdapter, SqliteProjectRepository};

struct FixedIds;
impl TagIdGenerator for FixedIds {
    fn generate_id(&self) -> String {
        "fixed-id-1".to_string()
    }
}

struct FixedClock;
impl ClockPort for FixedClock {
    fn now_ms(&self) -> Result<u64, ClockError> {
        Ok(1000)
    }
}

fn tag(id: &str, name: &str, color: &str, sort_order: usize) -> TagRecord {
    TagRecord {
        id: id.to_string(),
        name: name.to_string(),
        description: format!("{name} description"),
        icon: "Tag".to_string(),
        color: color.to_string(),
        sort_order,
        created_at: 10,
        updated_at: 20,
    }
}

#[test]
fn project_repository_round_trips_metadata_and_reordering() {
    let db = Arc::new(Database::open_in_memory().unwrap());
    let repository = SqliteProjectRepository::new(db);

    repository
        .replace_tags(vec![
            tag("project-b", "Beta", "#2563EB", 1),
            tag("project-a", "Alpha", "#DC2626", 0),
        ])
        .unwrap();

    let state = repository.load_state().unwrap();
    assert_eq!(
        state
            .tags
            .iter()
            .map(|tag| tag.id.as_str())
            .collect::<Vec<_>>(),
        ["project-a", "project-b"]
    );
    assert_eq!(state.tags[0].color, "#DC2626");
    assert_eq!(state.tags[0].sort_order, 0);

    let reordered = repository
        .reorder_tags(vec!["project-b".to_string(), "project-a".to_string()])
        .unwrap();
    assert_eq!(
        reordered
            .iter()
            .map(|tag| tag.id.as_str())
            .collect::<Vec<_>>(),
        ["project-b", "project-a"]
    );
    assert_eq!(reordered[0].sort_order, 0);
    assert_eq!(reordered[1].sort_order, 1);
}

#[test]
fn project_repository_maps_database_errors_to_repository_variant() {
    let db = Arc::new(Database::open_in_memory().unwrap());
    let repository = SqliteProjectRepository::new(db);

    let error = repository
        .replace_tags(vec![
            tag("duplicate", "First", "#000000", 0),
            tag("duplicate", "Last", "#111111", 1),
        ])
        .unwrap_err();

    assert!(matches!(
        error,
        TagError::Repository(message) if message.contains("UNIQUE constraint failed")
    ));
}

#[test]
fn project_adapter_full_lifecycle_and_cascade_delete() {
    let db = Arc::new(Database::open_in_memory().unwrap());
    let adapter =
        SqliteProjectAdapter::new(Arc::clone(&db), Arc::new(FixedIds), Arc::new(FixedClock));

    // 1. Create project with pipeline
    let created = adapter
        .create_project(ProjectCreateInput {
            name: "Work".to_string(),
            description: Some("Work project".to_string()),
            icon: Some("briefcase".to_string()),
            color: Some("#2563EB".to_string()),
            pipeline: Some(ProjectPipelineConfig {
                enabled: true,
                auto_polish: true,
                polish_preset_id: Some("preset-1".to_string()),
                ..Default::default()
            }),
        })
        .unwrap();

    assert_eq!(created.id, "fixed-id-1");
    assert_eq!(created.name, "Work");
    assert!(created.pipeline.is_some());
    assert!(created.pipeline.as_ref().unwrap().auto_polish);

    // 2. Active project selection
    adapter
        .set_active_project_id(Some("fixed-id-1".to_string()))
        .unwrap();
    assert_eq!(
        adapter.get_active_project_id().unwrap(),
        Some("fixed-id-1".to_string())
    );

    // 3. List projects verifies single-query join with pipeline
    let list = adapter.list_projects().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, "fixed-id-1");
    assert!(list[0].pipeline.is_some());

    // 4. Insert history item linked to this project
    db.with_connection(|conn| {
        conn.execute(
            "INSERT INTO history_items (id, timestamp, project_id) VALUES ('hist-1', 100, 'fixed-id-1')",
            [],
        )?;
        Ok(())
    })
    .unwrap();

    // 5. Cascade delete with "deleteItems"
    adapter
        .delete_project_with_cascade("fixed-id-1", "deleteItems")
        .unwrap();

    // Verify project deleted
    let remaining = adapter.list_projects().unwrap();
    assert_eq!(remaining.len(), 0);

    // Verify active project cleared
    assert_eq!(adapter.get_active_project_id().unwrap(), None);

    // Verify history item project_id set to NULL and deleted_at set (trash)
    db.with_connection(|conn| {
        let (project_id, deleted_at): (Option<String>, Option<i64>) = conn.query_row(
            "SELECT project_id, deleted_at FROM history_items WHERE id = 'hist-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        assert_eq!(project_id, None);
        assert!(deleted_at.is_some());

        // Verify pipeline table deleted
        let pipeline_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM project_pipelines WHERE project_id = 'fixed-id-1'",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(pipeline_count, 0);

        Ok(())
    })
    .unwrap();
}
