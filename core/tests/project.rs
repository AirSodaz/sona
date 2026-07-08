use sona_core::project::{
    ProjectCreateInput, ProjectDefaults, ProjectDefaultsInput, ProjectRecord,
};

#[test]
fn project_record_transport_shape_lives_in_core() {
    let project = ProjectRecord {
        id: "project-1".to_string(),
        name: "Work".to_string(),
        description: "Default workspace".to_string(),
        icon: "Briefcase".to_string(),
        created_at: 10,
        updated_at: 20,
        defaults: ProjectDefaults {
            summary_template_id: "general".to_string(),
            translation_language: "zh".to_string(),
            polish_preset_id: "general".to_string(),
            polish_scenario: None,
            polish_context: Some("weekly notes".to_string()),
            export_file_name_prefix: "sona".to_string(),
            enabled_text_replacement_set_ids: vec!["typos".to_string()],
            enabled_hotword_set_ids: vec!["team".to_string()],
            enabled_polish_keyword_set_ids: vec!["default".to_string()],
            enabled_speaker_profile_ids: vec!["speaker-1".to_string()],
        },
    };

    let value = serde_json::to_value(project).unwrap();

    assert_eq!(value["createdAt"], 10);
    assert_eq!(value["defaults"]["summaryTemplateId"], "general");
    assert_eq!(value["defaults"]["polishContext"], "weekly notes");
    assert!(value["defaults"].get("polishScenario").is_none());
    assert!(value.get("created_at").is_none());
}

#[test]
fn project_create_input_accepts_partial_defaults() {
    let input: ProjectCreateInput = serde_json::from_value(serde_json::json!({
        "name": "Work",
        "defaults": {
            "summaryTemplate": "meeting",
            "translationLanguage": "en"
        }
    }))
    .unwrap();

    assert_eq!(input.name, "Work");
    assert_eq!(input.description, None);
    assert_eq!(
        input.defaults,
        ProjectDefaultsInput {
            summary_template_id: None,
            summary_template: Some("meeting".to_string()),
            translation_language: Some("en".to_string()),
            polish_preset_id: None,
            polish_scenario: None,
            polish_context: None,
            export_file_name_prefix: None,
            enabled_text_replacement_set_ids: None,
            enabled_hotword_set_ids: None,
            enabled_polish_keyword_set_ids: None,
            enabled_speaker_profile_ids: None,
        }
    );
}

#[test]
fn test_project_normalization_logic() {
    use serde_json::json;
    use sona_core::project::{
        ProjectListOptions, non_empty_trimmed_string, normalize_project_value, positive_millis,
        string_array, string_value,
    };

    // Test helper functions
    assert_eq!(
        string_value(Some(&json!("hello"))),
        Some("hello".to_string())
    );
    assert_eq!(string_value(None), None);

    assert_eq!(
        non_empty_trimmed_string(Some(&json!("  hello  "))),
        Some("hello".to_string())
    );
    assert_eq!(non_empty_trimmed_string(Some(&json!("   "))), None);

    assert_eq!(positive_millis(Some(&json!(123.4))), Some(123));
    assert_eq!(positive_millis(Some(&json!(-10.0))), None);

    assert_eq!(
        string_array(Some(&json!(["a", "b"]))),
        Some(vec!["a".to_string(), "b".to_string()])
    );

    // Test project normalization
    let input = json!({
        "id": "p1",
        "name": "  My Project  ",
        "description": "desc",
        "createdAt": 1000,
        "updatedAt": 2000,
        "defaults": {
            "summaryTemplate": "general_template",
            "translationLanguage": "ja",
            "polishScenario": "writing",
            "enabledTextReplacementSetIds": ["r1"],
        }
    });

    let options = ProjectListOptions {
        fallback_enabled_polish_keyword_set_ids: vec!["fallback_keyword".to_string()],
        fallback_enabled_speaker_profile_ids: vec!["fallback_speaker".to_string()],
    };

    let record = normalize_project_value(&input, &options);

    assert_eq!(record.id, "p1");
    assert_eq!(record.name, "My Project");
    assert_eq!(record.description, "desc");
    assert_eq!(record.created_at, 1000);
    assert_eq!(record.updated_at, 2000);
    assert_eq!(record.defaults.summary_template_id, "general_template");
    assert_eq!(record.defaults.translation_language, "ja");
    assert_eq!(record.defaults.polish_preset_id, ""); // empty because polishScenario is present
    assert_eq!(record.defaults.polish_scenario, Some("writing".to_string()));
    assert_eq!(
        record.defaults.enabled_text_replacement_set_ids,
        vec!["r1".to_string()]
    );
    assert_eq!(
        record.defaults.enabled_polish_keyword_set_ids,
        vec!["fallback_keyword".to_string()]
    );
    assert_eq!(
        record.defaults.enabled_speaker_profile_ids,
        vec!["fallback_speaker".to_string()]
    );
}

#[test]
fn project_normalization_uses_supplied_fallback_timestamp() {
    use serde_json::json;
    use sona_core::project::{
        ProjectListOptions, normalize_project_record_for_import_with_timestamp,
        normalize_project_value_with_timestamp,
    };

    let record = normalize_project_value_with_timestamp(
        &json!({
            "id": "p1",
            "name": "Imported",
            "defaults": {}
        }),
        &ProjectListOptions::default(),
        1234,
    );

    assert_eq!(record.created_at, 1234);
    assert_eq!(record.updated_at, 1234);

    let value = normalize_project_record_for_import_with_timestamp(
        &json!({
            "id": "p2",
            "name": "Imported again",
            "defaults": {}
        }),
        5678,
    )
    .unwrap();

    assert_eq!(value["createdAt"], 5678);
    assert_eq!(value["updatedAt"], 5678);
}
