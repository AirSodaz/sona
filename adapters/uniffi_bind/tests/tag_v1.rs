use sona_uniffi_bind::{
    FfiTagCreateInputV1, FfiTagDefaultsInputV1, FfiTagDefaultsPatchV1, FfiTagUpdateInputV1,
    create_tag_v1, load_tag_repository_v1, reorder_tags_v1, update_tag_v1,
};

fn empty_defaults() -> FfiTagDefaultsInputV1 {
    FfiTagDefaultsInputV1 {
        summary_template_id: None,
        summary_template: None,
        translation_language: None,
        polish_preset_id: None,
        polish_scenario: None,
        polish_context: None,
        export_file_name_prefix: None,
        enabled_text_replacement_set_ids: None,
        enabled_hotword_set_ids: None,
        enabled_polish_keyword_set_ids: None,
        enabled_speaker_profile_ids: None,
    }
}

fn empty_defaults_patch() -> FfiTagDefaultsPatchV1 {
    FfiTagDefaultsPatchV1 {
        summary_template_id: None,
        translation_language: None,
        polish_preset_id: None,
        polish_scenario: None,
        polish_context: None,
        export_file_name_prefix: None,
        enabled_text_replacement_set_ids: None,
        enabled_hotword_set_ids: None,
        enabled_polish_keyword_set_ids: None,
        enabled_speaker_profile_ids: None,
    }
}

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
            defaults: empty_defaults(),
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
            defaults: empty_defaults(),
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
            defaults: Some(FfiTagDefaultsPatchV1 {
                translation_language: Some("en".to_string()),
                ..empty_defaults_patch()
            }),
        },
    )
    .unwrap()
    .unwrap();
    assert_eq!(updated.name, "Updated");
    assert_eq!(updated.defaults.translation_language, "en");

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
