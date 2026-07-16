use sona_core::tag::{TagDefaults, TagRecord, highest_priority_tag};

fn tag(id: &str, sort_order: usize) -> TagRecord {
    TagRecord {
        id: id.to_string(),
        name: id.to_string(),
        description: String::new(),
        icon: String::new(),
        color: String::new(),
        sort_order,
        created_at: 1,
        updated_at: 1,
        defaults: TagDefaults::default(),
    }
}

#[test]
fn tag_record_serializes_workspace_metadata() {
    let value = serde_json::to_value(tag("work", 2)).unwrap();

    assert_eq!(value["id"], "work");
    assert_eq!(value["color"], "");
    assert_eq!(value["sortOrder"], 2);
    assert!(value.get("projectId").is_none());
}

#[test]
fn highest_priority_tag_uses_global_sort_order() {
    let tags = vec![tag("later", 5), tag("first", 1), tag("middle", 3)];

    assert_eq!(
        highest_priority_tag(&tags, &["later".into(), "first".into()]).map(|tag| tag.id.as_str()),
        Some("first")
    );
    assert!(highest_priority_tag(&tags, &["missing".into()]).is_none());
}
