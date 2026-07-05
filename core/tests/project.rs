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
