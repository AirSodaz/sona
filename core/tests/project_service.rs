use std::sync::Mutex;

use serde_json::{Value, json};
use sona_core::ports::time::{ClockError, UnixMillisClock};
use sona_core::project::{
    ActiveProjectSelection, ProjectCreateInput, ProjectDefaults, ProjectDefaultsInput,
    ProjectDefaultsPatch, ProjectError, ProjectIdGenerator, ProjectListOptions, ProjectPatch,
    ProjectRecord, ProjectRepositoryService, ProjectStore, ProjectStoredState, ProjectUpdateInput,
};

#[derive(Default)]
struct MemoryProjectStore {
    state: Mutex<ProjectStoredState>,
    calls: Mutex<Vec<&'static str>>,
    fail_with: Mutex<Option<String>>,
    last_reorder: Mutex<Option<Vec<String>>>,
}

impl MemoryProjectStore {
    fn with_state(state: ProjectStoredState) -> Self {
        Self {
            state: Mutex::new(state),
            ..Self::default()
        }
    }

    fn fail(&self, message: &str) {
        *self.fail_with.lock().unwrap() = Some(message.to_string());
    }

    fn begin_call(&self, call: &'static str) -> Result<(), ProjectError> {
        self.calls.lock().unwrap().push(call);
        if let Some(error) = self.fail_with.lock().unwrap().take() {
            return Err(ProjectError::Repository(error));
        }
        Ok(())
    }
}

impl ProjectStore for MemoryProjectStore {
    fn load_state(&self) -> Result<ProjectStoredState, ProjectError> {
        self.begin_call("load_state")?;
        Ok(self.state.lock().unwrap().clone())
    }

    fn insert_project(&self, project: ProjectRecord) -> Result<ProjectRecord, ProjectError> {
        self.begin_call("insert_project")?;
        self.state.lock().unwrap().projects.push(project.clone());
        Ok(project)
    }

    fn update_project(
        &self,
        project_id: &str,
        patch: ProjectPatch,
        updated_at: u64,
    ) -> Result<Option<ProjectRecord>, ProjectError> {
        self.begin_call("update_project")?;
        let mut state = self.state.lock().unwrap();
        let Some(project) = state
            .projects
            .iter_mut()
            .find(|project| project.id == project_id)
        else {
            return Ok(None);
        };

        if let Some(value) = patch.name {
            project.name = value;
        }
        if let Some(value) = patch.icon {
            project.icon = value;
        }
        if let Some(value) = patch.description {
            project.description = value;
        }
        apply_defaults_patch(&mut project.defaults, patch.defaults);
        project.updated_at = updated_at;
        Ok(Some(project.clone()))
    }

    fn delete_project(&self, project_id: &str) -> Result<(), ProjectError> {
        self.begin_call("delete_project")?;
        let mut state = self.state.lock().unwrap();
        state.projects.retain(|project| project.id != project_id);
        Ok(())
    }

    fn replace_projects(&self, projects: Vec<ProjectRecord>) -> Result<(), ProjectError> {
        self.begin_call("replace_projects")?;
        let mut state = self.state.lock().unwrap();
        state.projects = projects;
        Ok(())
    }

    fn reorder_projects(
        &self,
        project_ids: Vec<String>,
    ) -> Result<Vec<ProjectRecord>, ProjectError> {
        self.begin_call("reorder_projects")?;
        *self.last_reorder.lock().unwrap() = Some(project_ids.clone());
        let state = self.state.lock().unwrap();
        let projects = project_ids
            .iter()
            .filter_map(|id| state.projects.iter().find(|project| &project.id == id))
            .cloned()
            .collect();
        Ok(projects)
    }

    fn set_active_project_setting_json(&self, setting_json: String) -> Result<(), ProjectError> {
        self.begin_call("set_active_project_setting_json")?;
        let mut state = self.state.lock().unwrap();
        state.active_project_setting_json = Some(setting_json);
        Ok(())
    }
}

struct SequenceIds(Mutex<Vec<String>>);

impl ProjectIdGenerator for SequenceIds {
    fn generate_id(&self) -> String {
        self.0.lock().unwrap().remove(0)
    }
}

struct FixedClock(u64);

impl UnixMillisClock for FixedClock {
    fn now_ms(&self) -> Result<u64, ClockError> {
        Ok(self.0)
    }
}

struct RecordingClock {
    result: Result<u64, ClockError>,
    calls: Mutex<usize>,
}

impl RecordingClock {
    fn fixed(value: u64) -> Self {
        Self {
            result: Ok(value),
            calls: Mutex::new(0),
        }
    }

    fn failing(message: &str) -> Self {
        Self {
            result: Err(ClockError::Unavailable(message.to_string())),
            calls: Mutex::new(0),
        }
    }
}

impl UnixMillisClock for RecordingClock {
    fn now_ms(&self) -> Result<u64, ClockError> {
        *self.calls.lock().unwrap() += 1;
        self.result.clone()
    }
}

fn service<'a>(
    store: &'a MemoryProjectStore,
    ids: &'a SequenceIds,
    clock: &'a dyn UnixMillisClock,
) -> ProjectRepositoryService<'a> {
    ProjectRepositoryService::new(store, ids, clock)
}

fn defaults() -> ProjectDefaults {
    ProjectDefaults {
        summary_template_id: "general".to_string(),
        translation_language: "zh".to_string(),
        polish_preset_id: "general".to_string(),
        polish_scenario: Some("scenario-old".to_string()),
        polish_context: Some("context-old".to_string()),
        export_file_name_prefix: "old-prefix".to_string(),
        enabled_text_replacement_set_ids: vec!["text-old".to_string()],
        enabled_hotword_set_ids: vec!["hotword-old".to_string()],
        enabled_polish_keyword_set_ids: vec!["keyword-old".to_string()],
        enabled_speaker_profile_ids: vec!["speaker-old".to_string()],
    }
}

fn project(id: &str, name: &str) -> ProjectRecord {
    ProjectRecord {
        id: id.to_string(),
        name: name.to_string(),
        description: "description-old".to_string(),
        icon: "icon-old".to_string(),
        created_at: 10,
        updated_at: 20,
        defaults: defaults(),
    }
}

fn apply_defaults_patch(defaults: &mut ProjectDefaults, patch: ProjectDefaultsPatch) {
    if let Some(value) = patch.summary_template_id {
        defaults.summary_template_id = value;
    }
    if let Some(value) = patch.translation_language {
        defaults.translation_language = value;
    }
    if let Some(value) = patch.polish_preset_id {
        defaults.polish_preset_id = value;
    }
    if let Some(value) = patch.polish_scenario {
        defaults.polish_scenario = Some(value);
    }
    if let Some(value) = patch.polish_context {
        defaults.polish_context = Some(value);
    }
    if let Some(value) = patch.export_file_name_prefix {
        defaults.export_file_name_prefix = value;
    }
    if let Some(value) = patch.enabled_text_replacement_set_ids {
        defaults.enabled_text_replacement_set_ids = value;
    }
    if let Some(value) = patch.enabled_hotword_set_ids {
        defaults.enabled_hotword_set_ids = value;
    }
    if let Some(value) = patch.enabled_polish_keyword_set_ids {
        defaults.enabled_polish_keyword_set_ids = value;
    }
    if let Some(value) = patch.enabled_speaker_profile_ids {
        defaults.enabled_speaker_profile_ids = value;
    }
}

#[test]
fn load_returns_projects_and_trims_json_string_active_setting() {
    let store = MemoryProjectStore::with_state(ProjectStoredState {
        projects: vec![project("p1", "First")],
        active_project_setting_json: Some(json!("  p1  ").to_string()),
    });
    let ids = SequenceIds(Mutex::new(vec![]));

    let snapshot = service(&store, &ids, &FixedClock(0)).load_state().unwrap();

    assert_eq!(snapshot.projects, vec![project("p1", "First")]);
    assert_eq!(snapshot.active_project_id.as_deref(), Some("p1"));
    assert_eq!(*store.calls.lock().unwrap(), vec!["load_state"]);
}

#[test]
fn absent_or_non_string_active_values_return_none() {
    let cases = [
        None,
        Some("null"),
        Some("123"),
        Some("\"\""),
        Some("\"   \""),
    ];

    for setting in cases {
        let store = MemoryProjectStore::with_state(ProjectStoredState {
            projects: vec![],
            active_project_setting_json: setting.map(str::to_string),
        });
        let ids = SequenceIds(Mutex::new(vec![]));
        assert_eq!(
            service(&store, &ids, &FixedClock(0))
                .get_active_project_id()
                .unwrap(),
            None,
            "setting {setting:?}"
        );
    }
}

#[test]
fn active_selection_distinguishes_missing_setting_from_present_empty_values() {
    let cases = [
        (None, false, None),
        (Some("null"), true, None),
        (Some("123"), true, None),
        (Some("\"\""), true, None),
        (Some("\"   \""), true, None),
        (Some("\" project-a \""), true, Some("project-a")),
    ];

    for (setting, setting_exists, project_id) in cases {
        let store = MemoryProjectStore::with_state(ProjectStoredState {
            projects: vec![],
            active_project_setting_json: setting.map(str::to_string),
        });
        let ids = SequenceIds(Mutex::new(vec![]));

        assert_eq!(
            service(&store, &ids, &FixedClock(0))
                .get_active_project_selection()
                .unwrap(),
            ActiveProjectSelection {
                setting_exists,
                project_id: project_id.map(str::to_string),
            },
            "setting {setting:?}"
        );
    }
}

#[test]
fn malformed_and_raw_empty_active_settings_return_exact_parse_errors_without_mutation() {
    let cases = [
        ("{", "EOF while parsing an object at line 1 column 1"),
        ("", "EOF while parsing a value at line 1 column 0"),
    ];

    for (setting, expected_error) in cases {
        let original = ProjectStoredState {
            projects: vec![project("p1", "First")],
            active_project_setting_json: Some(setting.to_string()),
        };
        let store = MemoryProjectStore::with_state(original.clone());
        let ids = SequenceIds(Mutex::new(vec![]));

        let error = service(&store, &ids, &FixedClock(0))
            .load_state()
            .unwrap_err();

        assert!(
            matches!(error, ProjectError::Serialization(ref source) if source.to_string() == expected_error)
        );
        assert_eq!(*store.state.lock().unwrap(), original);
        assert_eq!(*store.calls.lock().unwrap(), vec!["load_state"]);
    }
}

#[test]
fn set_active_overwrites_malformed_raw_value_and_serializes_string_and_null() {
    let store = MemoryProjectStore::with_state(ProjectStoredState {
        projects: vec![],
        active_project_setting_json: Some("{".to_string()),
    });
    let ids = SequenceIds(Mutex::new(vec![]));
    let service = service(&store, &ids, &FixedClock(0));

    service
        .set_active_project_id(Some("project-\"quoted\"".to_string()))
        .unwrap();
    assert_eq!(
        store.state.lock().unwrap().active_project_setting_json,
        Some("\"project-\\\"quoted\\\"\"".to_string())
    );
    assert_eq!(
        *store.calls.lock().unwrap(),
        vec!["set_active_project_setting_json"]
    );

    service.set_active_project_id(None).unwrap();
    assert_eq!(
        store.state.lock().unwrap().active_project_setting_json,
        Some("null".to_string())
    );
    assert_eq!(
        *store.calls.lock().unwrap(),
        vec![
            "set_active_project_setting_json",
            "set_active_project_setting_json"
        ]
    );
}

#[test]
fn create_uses_one_id_and_clock_and_preserves_adapter_defaults() {
    let store = MemoryProjectStore::default();
    let ids = SequenceIds(Mutex::new(vec!["id-1".to_string(), "id-2".to_string()]));
    let clock = RecordingClock::fixed(1234);
    let input = ProjectCreateInput {
        name: "  exact name  ".to_string(),
        description: Some("  exact description  ".to_string()),
        icon: Some("  exact icon  ".to_string()),
        defaults: ProjectDefaultsInput {
            summary_template_id: None,
            summary_template: Some("legacy-must-be-ignored".to_string()),
            translation_language: Some("en".to_string()),
            polish_preset_id: Some("formal".to_string()),
            polish_scenario: Some("meeting".to_string()),
            polish_context: Some("board".to_string()),
            export_file_name_prefix: Some("notes".to_string()),
            enabled_text_replacement_set_ids: Some(vec!["text".to_string()]),
            enabled_hotword_set_ids: Some(vec!["hotword".to_string()]),
            enabled_polish_keyword_set_ids: Some(vec!["keyword".to_string()]),
            enabled_speaker_profile_ids: Some(vec!["speaker".to_string()]),
        },
    };

    let created = service(&store, &ids, &clock).create_project(input).unwrap();

    assert_eq!(created.id, "id-1");
    assert_eq!(created.name, "  exact name  ");
    assert_eq!(created.description, "  exact description  ");
    assert_eq!(created.icon, "  exact icon  ");
    assert_eq!(created.created_at, 1234);
    assert_eq!(created.updated_at, 1234);
    assert_eq!(created.defaults.summary_template_id, "general");
    assert_eq!(created.defaults.translation_language, "en");
    assert_eq!(created.defaults.polish_preset_id, "formal");
    assert_eq!(created.defaults.polish_scenario.as_deref(), Some("meeting"));
    assert_eq!(created.defaults.polish_context.as_deref(), Some("board"));
    assert_eq!(created.defaults.export_file_name_prefix, "notes");
    assert_eq!(created.defaults.enabled_text_replacement_set_ids, ["text"]);
    assert_eq!(created.defaults.enabled_hotword_set_ids, ["hotword"]);
    assert_eq!(created.defaults.enabled_polish_keyword_set_ids, ["keyword"]);
    assert_eq!(created.defaults.enabled_speaker_profile_ids, ["speaker"]);
    assert_eq!(*ids.0.lock().unwrap(), vec!["id-2"]);
    assert_eq!(*clock.calls.lock().unwrap(), 1);
    assert_eq!(*store.calls.lock().unwrap(), vec!["insert_project"]);
}

#[test]
fn create_propagates_clock_and_store_errors_without_partial_writes() {
    let clock_store = MemoryProjectStore::default();
    let clock_ids = SequenceIds(Mutex::new(vec!["id-clock".to_string()]));
    let clock = RecordingClock::failing("clock failed exactly");
    let input = ProjectCreateInput {
        name: "name".to_string(),
        description: None,
        icon: None,
        defaults: ProjectDefaultsInput::default(),
    };

    assert!(matches!(
        service(&clock_store, &clock_ids, &clock)
            .create_project(input.clone())
            .unwrap_err(),
        ProjectError::Clock(ClockError::Unavailable(reason)) if reason == "clock failed exactly"
    ));
    assert!(clock_store.state.lock().unwrap().projects.is_empty());
    assert!(clock_store.calls.lock().unwrap().is_empty());
    assert_eq!(*clock_ids.0.lock().unwrap(), vec!["id-clock"]);

    let failing_store = MemoryProjectStore::default();
    failing_store.fail("store failed exactly");
    let store_ids = SequenceIds(Mutex::new(vec!["id-store".to_string()]));
    assert!(matches!(
        service(&failing_store, &store_ids, &FixedClock(88))
            .create_project(input)
            .unwrap_err(),
        ProjectError::Repository(reason) if reason == "store failed exactly"
    ));
    assert!(failing_store.state.lock().unwrap().projects.is_empty());
    assert!(store_ids.0.lock().unwrap().is_empty());
    assert_eq!(*failing_store.calls.lock().unwrap(), vec!["insert_project"]);
}

#[test]
fn typed_update_overlays_only_supplied_fields_and_uses_clock_timestamp() {
    let store = MemoryProjectStore::with_state(ProjectStoredState {
        projects: vec![project("p1", "Original")],
        active_project_setting_json: None,
    });
    let ids = SequenceIds(Mutex::new(vec![]));
    let clock = RecordingClock::fixed(777);
    let updates: ProjectUpdateInput = serde_json::from_value(json!({
        "description": "new description",
        "defaults": {
            "summaryTemplateId": "custom",
            "enabledHotwordSetIds": []
        }
    }))
    .unwrap();

    let updated = service(&store, &ids, &clock)
        .update_project("p1", updates)
        .unwrap()
        .unwrap();

    assert_eq!(updated.name, "Original");
    assert_eq!(updated.description, "new description");
    assert_eq!(updated.icon, "icon-old");
    assert_eq!(updated.updated_at, 777);
    assert_eq!(updated.defaults.summary_template_id, "custom");
    assert!(updated.defaults.enabled_hotword_set_ids.is_empty());
    assert_eq!(updated.defaults.translation_language, "zh");
    assert_eq!(*clock.calls.lock().unwrap(), 1);
    assert_eq!(*store.calls.lock().unwrap(), vec!["update_project"]);
}

#[test]
fn typed_replace_forwards_canonical_records_without_json_normalization() {
    let store = MemoryProjectStore::default();
    let ids = SequenceIds(Mutex::new(vec![]));
    let mut replacement = project("canonical", "");
    replacement.description = "typed description".to_string();
    replacement.created_at = 123;
    replacement.updated_at = 456;

    service(&store, &ids, &FixedClock(0))
        .replace_projects(vec![replacement.clone()])
        .unwrap();

    assert_eq!(store.state.lock().unwrap().projects, vec![replacement]);
    assert_eq!(*store.calls.lock().unwrap(), vec!["replace_projects"]);
}

#[test]
fn non_object_update_loads_current_project_without_clock_or_update_call() {
    let store = MemoryProjectStore::with_state(ProjectStoredState {
        projects: vec![project("p1", "Original")],
        active_project_setting_json: None,
    });
    let ids = SequenceIds(Mutex::new(vec![]));
    let clock = RecordingClock::failing("must not read clock");

    let unchanged = service(&store, &ids, &clock)
        .update_project_json("p1", json!(null))
        .unwrap();

    assert_eq!(unchanged, Some(project("p1", "Original")));
    assert_eq!(*clock.calls.lock().unwrap(), 0);
    assert_eq!(*store.calls.lock().unwrap(), vec!["load_state"]);
}

#[test]
fn object_update_parses_compatible_patch_and_uses_clock_timestamp() {
    let store = MemoryProjectStore::with_state(ProjectStoredState {
        projects: vec![project("p1", "Original")],
        active_project_setting_json: None,
    });
    let ids = SequenceIds(Mutex::new(vec![]));
    let clock = RecordingClock::fixed(777);

    let updated = service(&store, &ids, &clock)
        .update_project_json(
            "p1",
            json!({
                "name": "",
                "icon": 99,
                "description": "new description",
                "defaults": {
                    "summaryTemplateId": "custom",
                    "translationLanguage": false,
                    "polishPresetId": "",
                    "polishScenario": null,
                    "polishContext": 42,
                    "exportFileNamePrefix": "new-prefix",
                    "enabledTextReplacementSetIds": ["a", 1, "b"],
                    "enabledHotwordSetIds": "not-an-array",
                    "enabledPolishKeywordSetIds": [false, "keyword"],
                    "enabledSpeakerProfileIds": []
                }
            }),
        )
        .unwrap()
        .unwrap();

    assert_eq!(updated.name, "");
    assert_eq!(updated.icon, "icon-old");
    assert_eq!(updated.description, "new description");
    assert_eq!(updated.updated_at, 777);
    assert_eq!(updated.defaults.summary_template_id, "custom");
    assert_eq!(updated.defaults.translation_language, "zh");
    assert_eq!(updated.defaults.polish_preset_id, "");
    assert_eq!(
        updated.defaults.polish_scenario.as_deref(),
        Some("scenario-old")
    );
    assert_eq!(
        updated.defaults.polish_context.as_deref(),
        Some("context-old")
    );
    assert_eq!(updated.defaults.export_file_name_prefix, "new-prefix");
    assert_eq!(
        updated.defaults.enabled_text_replacement_set_ids,
        ["a", "b"]
    );
    assert_eq!(updated.defaults.enabled_hotword_set_ids, ["hotword-old"]);
    assert_eq!(updated.defaults.enabled_polish_keyword_set_ids, ["keyword"]);
    assert!(updated.defaults.enabled_speaker_profile_ids.is_empty());
    assert_eq!(*clock.calls.lock().unwrap(), 1);
    assert_eq!(*store.calls.lock().unwrap(), vec!["update_project"]);
}

#[test]
fn missing_object_update_returns_none() {
    let store = MemoryProjectStore::default();
    let ids = SequenceIds(Mutex::new(vec![]));
    let clock = RecordingClock::fixed(42);

    assert_eq!(
        service(&store, &ids, &clock)
            .update_project_json("missing", json!({"name": "new"}))
            .unwrap(),
        None
    );
    assert_eq!(*clock.calls.lock().unwrap(), 1);
    assert_eq!(*store.calls.lock().unwrap(), vec!["update_project"]);
}

#[test]
fn replace_projects_preserves_order_and_legacy_scalar_semantics() {
    let store = MemoryProjectStore::with_state(ProjectStoredState {
        projects: vec![project("old", "Old")],
        active_project_setting_json: Some(json!("old").to_string()),
    });
    let ids = SequenceIds(Mutex::new(vec![]));

    service(&store, &ids, &FixedClock(0))
        .replace_projects_json(vec![
            json!({
                "id": "first",
                "name": "",
                "description": 1,
                "icon": "first-icon",
                "createdAt": 12,
                "updatedAt": -1,
                "defaults": {
                    "summaryTemplate": "legacy",
                    "enabledPolishKeywordSetIds": ["one", false],
                    "enabledSpeakerProfileIds": ["speaker"]
                }
            }),
            json!({
                "id": 2,
                "name": "  exact whitespace  ",
                "createdAt": 3.5,
                "updatedAt": 9,
                "defaults": null
            }),
            Value::Null,
        ])
        .unwrap();

    let state = store.state.lock().unwrap();
    assert_eq!(state.projects.len(), 3);
    assert_eq!(state.projects[0].id, "first");
    assert_eq!(state.projects[0].name, "");
    assert_eq!(state.projects[0].description, "");
    assert_eq!(state.projects[0].created_at, 12);
    assert_eq!(state.projects[0].updated_at, 0);
    assert_eq!(state.projects[0].defaults.summary_template_id, "legacy");
    assert_eq!(
        state.projects[0].defaults.enabled_polish_keyword_set_ids,
        ["one"]
    );
    assert_eq!(
        state.projects[0].defaults.enabled_speaker_profile_ids,
        ["speaker"]
    );
    assert_eq!(state.projects[1].id, "");
    assert_eq!(state.projects[1].name, "  exact whitespace  ");
    assert_eq!(state.projects[1].created_at, 0);
    assert_eq!(state.projects[1].updated_at, 9);
    assert_eq!(state.projects[2].name, "");
    assert_eq!(
        state.active_project_setting_json,
        Some(json!("old").to_string())
    );
    assert_eq!(*store.calls.lock().unwrap(), vec!["replace_projects"]);
}

#[test]
fn replace_succeeds_once_despite_malformed_active_setting() {
    let store = MemoryProjectStore::with_state(ProjectStoredState {
        projects: vec![project("old", "Old")],
        active_project_setting_json: Some("{".to_string()),
    });
    let ids = SequenceIds(Mutex::new(vec![]));

    service(&store, &ids, &FixedClock(0))
        .replace_projects_json(vec![json!({"id": "new", "name": "New"})])
        .unwrap();

    let state = store.state.lock().unwrap();
    assert_eq!(state.projects[0].id, "new");
    assert_eq!(state.active_project_setting_json.as_deref(), Some("{"));
    assert_eq!(*store.calls.lock().unwrap(), vec!["replace_projects"]);
}

#[test]
fn replace_projects_propagates_store_error_without_partial_write() {
    let original = ProjectStoredState {
        projects: vec![project("old", "Old")],
        active_project_setting_json: None,
    };
    let store = MemoryProjectStore::with_state(original.clone());
    store.fail("replace failed exactly");
    let ids = SequenceIds(Mutex::new(vec![]));

    assert!(matches!(
        service(&store, &ids, &FixedClock(0))
            .replace_projects_json(vec![json!({"id": "new", "name": "New"})])
            .unwrap_err(),
        ProjectError::Repository(reason) if reason == "replace failed exactly"
    ));
    assert_eq!(*store.state.lock().unwrap(), original);
    assert_eq!(*store.calls.lock().unwrap(), vec!["replace_projects"]);
}

#[test]
fn delete_succeeds_once_with_raw_empty_active_setting_and_does_not_clear_it() {
    let store = MemoryProjectStore::with_state(ProjectStoredState {
        projects: vec![project("p1", "First"), project("p2", "Second")],
        active_project_setting_json: Some(String::new()),
    });
    let ids = SequenceIds(Mutex::new(vec![]));

    service(&store, &ids, &FixedClock(0))
        .delete_project("p1")
        .unwrap();

    let state = store.state.lock().unwrap();
    assert_eq!(state.projects, vec![project("p2", "Second")]);
    assert_eq!(state.active_project_setting_json.as_deref(), Some(""));
    assert_eq!(*store.calls.lock().unwrap(), vec!["delete_project"]);
}

#[test]
fn reorder_delegates_supplied_ids_and_returns_store_result() {
    let store = MemoryProjectStore::with_state(ProjectStoredState {
        projects: vec![project("p1", "First"), project("p2", "Second")],
        active_project_setting_json: None,
    });
    let ids = SequenceIds(Mutex::new(vec![]));
    let supplied = vec!["p2".to_string(), "missing".to_string(), "p1".to_string()];

    let reordered = service(&store, &ids, &FixedClock(0))
        .reorder_projects(supplied.clone())
        .unwrap();

    assert_eq!(
        reordered,
        vec![project("p2", "Second"), project("p1", "First")]
    );
    assert_eq!(*store.last_reorder.lock().unwrap(), Some(supplied));
    assert_eq!(*store.calls.lock().unwrap(), vec!["reorder_projects"]);
}

#[test]
fn list_accepts_compatibility_options_without_changing_typed_records() {
    let projects = vec![project("p1", "First")];
    let store = MemoryProjectStore::with_state(ProjectStoredState {
        projects: projects.clone(),
        active_project_setting_json: None,
    });
    let ids = SequenceIds(Mutex::new(vec![]));

    let listed = service(&store, &ids, &FixedClock(0))
        .list_projects(ProjectListOptions {
            fallback_enabled_polish_keyword_set_ids: vec!["fallback-keyword".to_string()],
            fallback_enabled_speaker_profile_ids: vec!["fallback-speaker".to_string()],
        })
        .unwrap();

    assert_eq!(listed, projects);
    assert_eq!(*store.calls.lock().unwrap(), vec!["load_state"]);
}
