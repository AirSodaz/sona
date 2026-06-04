#![allow(dead_code)]

use tauri_appsona_lib::repositories::project as project_repository;

use project_repository::{
    ProjectCreateInput, ProjectDefaultsInput, ProjectListOptions, ProjectRepository,
    get_active_project_id_from_dir, set_active_project_id_in_dir,
};
use serde_json::json;
use std::fs;

fn defaults() -> ProjectDefaultsInput {
    ProjectDefaultsInput {
        summary_template: None,
        summary_template_id: Some("general".to_string()),
        translation_language: Some("zh".to_string()),
        polish_preset_id: Some("general".to_string()),
        polish_scenario: None,
        polish_context: None,
        export_file_name_prefix: Some(String::new()),
        enabled_text_replacement_set_ids: Some(Vec::new()),
        enabled_hotword_set_ids: Some(Vec::new()),
        enabled_polish_keyword_set_ids: Some(Vec::new()),
        enabled_speaker_profile_ids: Some(Vec::new()),
    }
}

#[test]
fn list_normalizes_backfills_and_writes_migrated_projects() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let projects_dir = temp_dir.path().join("projects");
    fs::create_dir_all(&projects_dir).expect("projects dir");
    fs::write(
        projects_dir.join("index.json"),
        serde_json::to_string_pretty(&json!([
            {
                "id": "project-1",
                "name": "  Alpha  ",
                "description": "Legacy",
                "createdAt": 10,
                "updatedAt": 20,
                "defaults": {
                    "translationLanguage": "en",
                    "polishPresetId": "lecture",
                    "enabledTextReplacementSetIds": ["replace"],
                    "enabledHotwordSetIds": ["hot"]
                }
            }
        ]))
        .expect("json"),
    )
    .expect("write index");

    let repository = ProjectRepository::new(temp_dir.path().to_path_buf());
    let projects = repository
        .list(ProjectListOptions {
            fallback_enabled_polish_keyword_set_ids: vec!["keywords".to_string()],
            fallback_enabled_speaker_profile_ids: vec!["speaker".to_string()],
        })
        .expect("list projects");

    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].name, "Alpha");
    assert_eq!(projects[0].icon, "");
    assert_eq!(projects[0].defaults.summary_template_id, "general");
    assert_eq!(
        projects[0].defaults.enabled_polish_keyword_set_ids,
        vec!["keywords".to_string()]
    );
    assert_eq!(
        projects[0].defaults.enabled_speaker_profile_ids,
        vec!["speaker".to_string()]
    );

    let persisted: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(projects_dir.join("index.json")).expect("read"))
            .expect("persisted json");
    assert_eq!(persisted[0]["name"], "Alpha");
    assert_eq!(persisted[0]["icon"], "");
    assert_eq!(
        persisted[0]["defaults"]["enabledPolishKeywordSetIds"],
        json!(["keywords"])
    );
    assert_eq!(
        persisted[0]["defaults"]["enabledSpeakerProfileIds"],
        json!(["speaker"])
    );
}

#[test]
fn create_update_reorder_and_delete_preserve_project_shape() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let repository = ProjectRepository::new(temp_dir.path().to_path_buf());

    let alpha = repository
        .create(ProjectCreateInput {
            name: "Alpha".to_string(),
            description: Some("First".to_string()),
            icon: Some("a".to_string()),
            defaults: defaults(),
        })
        .expect("create alpha");
    let beta = repository
        .create(ProjectCreateInput {
            name: "Beta".to_string(),
            description: None,
            icon: None,
            defaults: defaults(),
        })
        .expect("create beta");

    assert_eq!(
        repository
            .list(ProjectListOptions::default())
            .expect("list")
            .iter()
            .map(|project| project.id.as_str())
            .collect::<Vec<_>>(),
        vec![beta.id.as_str(), alpha.id.as_str()]
    );

    let updated = repository
        .update(
            &alpha.id,
            json!({
                "name": "Alpha Updated",
                "icon": "folder",
                "defaults": {
                    "translationLanguage": "en",
                    "enabledPolishKeywordSetIds": ["keywords"]
                },
                "createdAt": 999
            }),
        )
        .expect("update")
        .expect("updated project");
    assert_eq!(updated.name, "Alpha Updated");
    assert_eq!(updated.icon, "folder");
    assert_eq!(updated.created_at, alpha.created_at);
    assert_eq!(updated.defaults.translation_language, "en");
    assert_eq!(
        updated.defaults.enabled_polish_keyword_set_ids,
        vec!["keywords".to_string()]
    );

    let reordered = repository
        .reorder(vec![beta.id.clone()])
        .expect("reorder projects");
    assert_eq!(
        reordered
            .iter()
            .map(|project| project.id.as_str())
            .collect::<Vec<_>>(),
        vec![beta.id.as_str(), alpha.id.as_str()]
    );

    repository.delete(&beta.id).expect("delete beta");
    let remaining = repository
        .list(ProjectListOptions::default())
        .expect("list remaining");
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].id, alpha.id);
}

#[test]
fn active_project_id_round_trips_through_settings_json() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    fs::write(
        temp_dir.path().join("settings.json"),
        serde_json::to_string_pretty(&json!({ "other": true })).expect("json"),
    )
    .expect("write settings");

    assert_eq!(
        get_active_project_id_from_dir(temp_dir.path()).expect("get active"),
        None
    );

    set_active_project_id_in_dir(temp_dir.path(), Some("project-1".to_string()))
        .expect("set active");
    assert_eq!(
        get_active_project_id_from_dir(temp_dir.path()).expect("get active"),
        Some("project-1".to_string())
    );

    set_active_project_id_in_dir(temp_dir.path(), None).expect("clear active");
    assert_eq!(
        get_active_project_id_from_dir(temp_dir.path()).expect("get active"),
        None
    );

    let persisted: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(temp_dir.path().join("settings.json")).expect("read"),
    )
    .expect("persisted settings");
    assert_eq!(persisted["other"], true);
    assert!(persisted["sona-active-project-id"].is_null());
}
