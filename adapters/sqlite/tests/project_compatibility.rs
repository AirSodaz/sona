use std::sync::Arc;

use sona_core::project::{ProjectDefaults, ProjectRecord, ProjectStore};
use sona_core::tag::{TagDefaults, TagRecord, TagStore};
use sona_sqlite::{Database, SqliteProjectRepository, SqliteTagRepository};

fn project(id: &str, name: &str) -> ProjectRecord {
    ProjectRecord {
        id: id.to_string(),
        name: name.to_string(),
        description: format!("{name} description"),
        icon: "Folder".to_string(),
        created_at: 10,
        updated_at: 20,
        defaults: ProjectDefaults {
            summary_template_id: "general".to_string(),
            translation_language: "zh".to_string(),
            polish_preset_id: "general".to_string(),
            polish_scenario: None,
            polish_context: None,
            export_file_name_prefix: format!("{name}-"),
            enabled_text_replacement_set_ids: Vec::new(),
            enabled_hotword_set_ids: Vec::new(),
            enabled_polish_keyword_set_ids: Vec::new(),
            enabled_speaker_profile_ids: Vec::new(),
        },
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
        defaults: TagDefaults {
            export_file_name_prefix: format!("{name}-"),
            ..TagDefaults::default()
        },
    }
}

#[test]
fn legacy_project_replacement_preserves_tag_colors_and_uses_input_order() {
    let db = Arc::new(Database::open_in_memory().unwrap());
    let tags = SqliteTagRepository::new(Arc::clone(&db));
    let projects = SqliteProjectRepository::new(db);

    TagStore::replace_tags(
        &tags,
        vec![
            tag("tag-a", "Alpha", "#DC2626", 0),
            tag("tag-b", "Beta", "#2563EB", 1),
        ],
    )
    .unwrap();

    ProjectStore::replace_projects(
        &projects,
        vec![
            project("tag-b", "Beta updated"),
            project("tag-a", "Alpha updated"),
        ],
    )
    .unwrap();

    let state = TagStore::load_state(&tags).unwrap();
    assert_eq!(
        state
            .tags
            .iter()
            .map(|tag| (tag.id.as_str(), tag.color.as_str(), tag.sort_order))
            .collect::<Vec<_>>(),
        [("tag-b", "#2563EB", 0), ("tag-a", "#DC2626", 1),]
    );
}

#[test]
fn legacy_project_insert_appends_after_existing_tags() {
    let db = Arc::new(Database::open_in_memory().unwrap());
    let tags = SqliteTagRepository::new(Arc::clone(&db));
    let projects = SqliteProjectRepository::new(db);

    TagStore::insert_tag(&tags, tag("tag-a", "Alpha", "#DC2626", 0)).unwrap();

    let inserted = ProjectStore::insert_project(&projects, project("tag-b", "Beta")).unwrap();
    assert_eq!(inserted.id, "tag-b");

    let state = TagStore::load_state(&tags).unwrap();
    let canonical = state
        .tags
        .iter()
        .find(|tag| tag.id == "tag-b")
        .expect("Project compatibility insert should create a canonical Tag");
    assert_eq!(canonical.sort_order, 1);
    assert_eq!(canonical.color, "");
    assert_eq!(canonical.name, "Beta");
    assert_eq!(canonical.defaults.export_file_name_prefix, "Beta-");
}
