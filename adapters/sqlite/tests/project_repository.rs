use std::sync::Arc;

use sona_core::automation::repository::{
    AutomationRuleRecord, AutomationRuleRecordExportConfig, AutomationRuleRecordStageConfig,
    AutomationStore,
};
use sona_core::ports::time::{ClockError, ClockPort};
use sona_core::project::ports::ProjectRepository;
use sona_core::project::{ProjectCreateInput, ProjectPipelineConfig};
use sona_core::tag::{TagError, TagIdGenerator, TagRecord, TagStore};
use sona_sqlite::{
    Database, SqliteAutomationRepository, SqliteProjectAdapter, SqliteProjectRepository,
};

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

fn automation_rule(project_id: &str) -> AutomationRuleRecord {
    AutomationRuleRecord {
        id: "rule-1".to_string(),
        name: "Rule".to_string(),
        save_history: true,
        project_id: project_id.to_string(),
        watch_directory: "C:/watch".to_string(),
        recursive: false,
        enabled: true,
        stage_config: AutomationRuleRecordStageConfig {
            auto_polish: false,
            polish_preset_id: "general".to_string(),
            auto_translate: false,
            translation_language: "en".to_string(),
        },
        export_config: AutomationRuleRecordExportConfig {
            directory: String::new(),
            format: "srt".to_string(),
            mode: "original".to_string(),
            prefix: String::new(),
        },
        created_at: 1_000,
        updated_at: 2_000,
    }
}

fn seed_rule(automation: &SqliteAutomationRepository, project_id: &str) -> String {
    AutomationStore::replace_rules(automation, &[automation_rule(project_id)]).unwrap();
    AutomationStore::load_state(automation).unwrap().rules[0]
        .project_id
        .clone()
}

#[test]
fn project_deletion_and_replacement_clear_rule_project_ids() {
    let db = Arc::new(Database::open_in_memory().unwrap());
    let repository = SqliteProjectRepository::new(Arc::clone(&db));
    let automation = SqliteAutomationRepository::new(Arc::clone(&db));

    let loaded_project_id = || {
        AutomationStore::load_state(&automation).unwrap().rules[0]
            .project_id
            .clone()
    };

    // Deleting every project clears every rule link.
    assert_eq!(seed_rule(&automation, "proj-1"), "proj-1");
    repository.replace_tags(Vec::new()).unwrap();
    assert_eq!(loaded_project_id(), "");

    // Replacing projects keeps links to surviving projects only.
    repository
        .replace_tags(vec![tag("proj-1", "Kept", "#2563EB", 0)])
        .unwrap();
    assert_eq!(seed_rule(&automation, "proj-1"), "proj-1");
    repository
        .replace_tags(vec![tag("proj-1", "Kept", "#2563EB", 0)])
        .unwrap();
    assert_eq!(loaded_project_id(), "proj-1");
    repository
        .replace_tags(vec![tag("proj-2", "Other", "#DC2626", 0)])
        .unwrap();
    assert_eq!(loaded_project_id(), "");

    // Deleting a single project clears only that project's links.
    repository
        .replace_tags(vec![
            tag("proj-1", "Kept", "#2563EB", 0),
            tag("proj-2", "Other", "#DC2626", 1),
        ])
        .unwrap();
    assert_eq!(seed_rule(&automation, "proj-1"), "proj-1");
    repository.delete_tag("proj-1").unwrap();
    assert_eq!(loaded_project_id(), "");
}

#[test]
fn project_cascade_disables_rules_that_lose_their_project() {
    let db = Arc::new(Database::open_in_memory().unwrap());
    let adapter =
        SqliteProjectAdapter::new(Arc::clone(&db), Arc::new(FixedIds), Arc::new(FixedClock));
    let automation = SqliteAutomationRepository::new(Arc::clone(&db));

    adapter
        .replace_tags(vec![
            tag("proj-1", "One", "#2563EB", 0),
            tag("proj-2", "Two", "#DC2626", 1),
        ])
        .unwrap();

    let mut affected = automation_rule("proj-1");
    affected.id = "rule-affected".to_string();
    let mut survivor = automation_rule("proj-2");
    survivor.id = "rule-survivor".to_string();
    AutomationStore::replace_rules(&automation, &[affected, survivor]).unwrap();

    adapter
        .delete_project_with_cascade("proj-1", "moveToInbox")
        .unwrap();

    let state = AutomationStore::load_state(&automation).unwrap();
    let affected = state
        .rules
        .iter()
        .find(|rule| rule.id == "rule-affected")
        .unwrap();
    assert_eq!(affected.project_id, "");
    assert!(!affected.enabled);

    let survivor = state
        .rules
        .iter()
        .find(|rule| rule.id == "rule-survivor")
        .unwrap();
    assert_eq!(survivor.project_id, "proj-2");
    assert!(survivor.enabled);
}
