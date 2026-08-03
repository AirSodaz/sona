use sona_uniffi_bind::{
    FfiTagCreateInputV1, FfiTagUpdateInputV1, create_tag_v1, load_tag_repository_v1,
    reorder_tags_v1, update_tag_v1,
};

#[test]
fn tag_v1_roundtrips_records_without_json_payloads() {
    let dir = tempfile::tempdir().unwrap();
    let app_data_dir = dir.path().to_string_lossy().into_owned();

    let empty = load_tag_repository_v1(app_data_dir.clone()).unwrap();
    assert!(empty.tags.is_empty());
    assert_eq!(empty.active_tag_id, None);

    let first = create_tag_v1(
        app_data_dir.clone(),
        FfiTagCreateInputV1 {
            name: "First".to_string(),
            description: Some("Description".to_string()),
            icon: Some("tag".to_string()),
            color: Some("#112233".to_string()),
        },
    )
    .unwrap();
    let second = create_tag_v1(
        app_data_dir.clone(),
        FfiTagCreateInputV1 {
            name: "Second".to_string(),
            description: None,
            icon: None,
            color: None,
        },
    )
    .unwrap();

    let updated = update_tag_v1(
        app_data_dir.clone(),
        first.id.clone(),
        FfiTagUpdateInputV1 {
            name: Some("Updated".to_string()),
            icon: None,
            color: None,
            description: None,
        },
    )
    .unwrap()
    .unwrap();
    assert_eq!(updated.name, "Updated");

    let reordered = reorder_tags_v1(
        app_data_dir.clone(),
        vec![second.id.clone(), first.id.clone()],
    )
    .unwrap();
    assert_eq!(reordered[0].id, second.id);
    assert_eq!(reordered[0].sort_order, 0);
    assert_eq!(reordered[1].id, first.id);
    assert_eq!(reordered[1].sort_order, 1);

    let snapshot = load_tag_repository_v1(app_data_dir).unwrap();
    assert_eq!(snapshot.tags, reordered);
}
