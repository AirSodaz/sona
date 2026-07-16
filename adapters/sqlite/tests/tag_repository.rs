use std::sync::Arc;

use sona_core::tag::{TagDefaults, TagRecord, TagStore};
use sona_sqlite::{Database, SqliteTagRepository};

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
fn tag_repository_round_trips_color_sort_order_defaults_and_reordering() {
    let db = Arc::new(Database::open_in_memory().unwrap());
    let repository = SqliteTagRepository::new(db);

    repository
        .replace_tags(vec![
            tag("tag-b", "Beta", "#2563EB", 1),
            tag("tag-a", "Alpha", "#DC2626", 0),
        ])
        .unwrap();

    let state = repository.load_state().unwrap();
    assert_eq!(
        state
            .tags
            .iter()
            .map(|tag| tag.id.as_str())
            .collect::<Vec<_>>(),
        ["tag-a", "tag-b"]
    );
    assert_eq!(state.tags[0].color, "#DC2626");
    assert_eq!(state.tags[0].sort_order, 0);
    assert_eq!(state.tags[0].defaults.export_file_name_prefix, "Alpha-");

    let reordered = repository
        .reorder_tags(vec!["tag-b".to_string(), "tag-a".to_string()])
        .unwrap();
    assert_eq!(
        reordered
            .iter()
            .map(|tag| tag.id.as_str())
            .collect::<Vec<_>>(),
        ["tag-b", "tag-a"]
    );
    assert_eq!(reordered[0].sort_order, 0);
    assert_eq!(reordered[1].sort_order, 1);
}
