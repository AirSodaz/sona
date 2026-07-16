use serde_json::{Map, Value};

use crate::ports::time::UnixMillisClock;

use super::{
    ActiveProjectSelection, DEFAULT_POLISH_PRESET_ID, DEFAULT_SUMMARY_TEMPLATE_ID,
    DEFAULT_TRANSLATION_LANGUAGE, ProjectCreateInput, ProjectDefaults, ProjectDefaultsInput,
    ProjectDefaultsPatch, ProjectListOptions, ProjectPatch, ProjectRecord,
    ProjectRepositorySnapshot, ProjectStore, ProjectStoredState, ProjectUpdateInput,
    active_project_id_from_value, normalize_defaults,
};

pub trait ProjectIdGenerator: Send + Sync {
    fn generate_id(&self) -> String;
}

pub struct ProjectRepositoryService<'a> {
    store: &'a dyn ProjectStore,
    ids: &'a dyn ProjectIdGenerator,
    clock: &'a dyn UnixMillisClock,
}

impl<'a> ProjectRepositoryService<'a> {
    pub fn new(
        store: &'a dyn ProjectStore,
        ids: &'a dyn ProjectIdGenerator,
        clock: &'a dyn UnixMillisClock,
    ) -> Self {
        Self { store, ids, clock }
    }

    pub fn load_state(&self) -> Result<ProjectRepositorySnapshot, String> {
        snapshot_from_state(self.store.load_state()?)
    }

    pub fn list_projects(
        &self,
        _options: ProjectListOptions,
    ) -> Result<Vec<ProjectRecord>, String> {
        Ok(self.store.load_state()?.projects)
    }

    pub fn replace_projects_json(&self, projects: Vec<Value>) -> Result<(), String> {
        let projects = projects.iter().map(normalize_replacement_project).collect();
        self.store.replace_projects(projects)
    }

    pub fn replace_projects(&self, projects: Vec<ProjectRecord>) -> Result<(), String> {
        self.store.replace_projects(projects)
    }

    pub fn create_project(&self, input: ProjectCreateInput) -> Result<ProjectRecord, String> {
        let now = self.clock.now_ms()?;
        let project = ProjectRecord {
            id: self.ids.generate_id(),
            name: input.name,
            description: input.description.unwrap_or_default(),
            icon: input.icon.unwrap_or_default(),
            created_at: now,
            updated_at: now,
            defaults: create_defaults(input.defaults),
        };
        self.store.insert_project(project)
    }

    pub fn update_project_json(
        &self,
        project_id: &str,
        updates: Value,
    ) -> Result<Option<ProjectRecord>, String> {
        let Some(updates) = updates.as_object() else {
            return Ok(self
                .store
                .load_state()?
                .projects
                .into_iter()
                .find(|project| project.id == project_id));
        };

        let patch = parse_project_patch(updates);
        let updated_at = self.clock.now_ms()?;
        self.store.update_project(project_id, patch, updated_at)
    }

    pub fn update_project(
        &self,
        project_id: &str,
        updates: ProjectUpdateInput,
    ) -> Result<Option<ProjectRecord>, String> {
        let patch = ProjectPatch {
            name: updates.name,
            icon: updates.icon,
            description: updates.description,
            defaults: updates.defaults.unwrap_or_default(),
        };
        let updated_at = self.clock.now_ms()?;
        self.store.update_project(project_id, patch, updated_at)
    }

    pub fn delete_project(&self, project_id: &str) -> Result<(), String> {
        self.store.delete_project(project_id)
    }

    pub fn reorder_projects(&self, project_ids: Vec<String>) -> Result<Vec<ProjectRecord>, String> {
        self.store.reorder_projects(project_ids)
    }

    pub fn get_active_project_id(&self) -> Result<Option<String>, String> {
        Ok(self.get_active_project_selection()?.project_id)
    }

    pub fn get_active_project_selection(&self) -> Result<ActiveProjectSelection, String> {
        active_selection_from_setting_json(
            self.store
                .load_state()?
                .active_project_setting_json
                .as_deref(),
        )
    }

    pub fn set_active_project_id(&self, project_id: Option<String>) -> Result<(), String> {
        let value = project_id.map(Value::String).unwrap_or(Value::Null);
        let setting_json = serde_json::to_string(&value).map_err(|error| error.to_string())?;
        self.store.set_active_project_setting_json(setting_json)
    }
}

fn snapshot_from_state(state: ProjectStoredState) -> Result<ProjectRepositorySnapshot, String> {
    let active_project_id =
        active_selection_from_setting_json(state.active_project_setting_json.as_deref())?
            .project_id;

    Ok(ProjectRepositorySnapshot {
        projects: state.projects,
        active_project_id,
    })
}

fn active_selection_from_setting_json(
    setting_json: Option<&str>,
) -> Result<ActiveProjectSelection, String> {
    let project_id = setting_json
        .map(serde_json::from_str::<Value>)
        .transpose()
        .map_err(|error| error.to_string())?
        .as_ref()
        .and_then(active_project_id_from_value);

    Ok(ActiveProjectSelection {
        setting_exists: setting_json.is_some(),
        project_id,
    })
}

fn create_defaults(input: ProjectDefaultsInput) -> ProjectDefaults {
    ProjectDefaults {
        summary_template_id: input
            .summary_template_id
            .unwrap_or_else(|| DEFAULT_SUMMARY_TEMPLATE_ID.to_string()),
        translation_language: input
            .translation_language
            .unwrap_or_else(|| DEFAULT_TRANSLATION_LANGUAGE.to_string()),
        polish_preset_id: input
            .polish_preset_id
            .unwrap_or_else(|| DEFAULT_POLISH_PRESET_ID.to_string()),
        polish_scenario: input.polish_scenario,
        polish_context: input.polish_context,
        export_file_name_prefix: input.export_file_name_prefix.unwrap_or_default(),
        enabled_text_replacement_set_ids: input
            .enabled_text_replacement_set_ids
            .unwrap_or_default(),
        enabled_hotword_set_ids: input.enabled_hotword_set_ids.unwrap_or_default(),
        enabled_polish_keyword_set_ids: input.enabled_polish_keyword_set_ids.unwrap_or_default(),
        enabled_speaker_profile_ids: input.enabled_speaker_profile_ids.unwrap_or_default(),
    }
}

fn normalize_replacement_project(input: &Value) -> ProjectRecord {
    let source = input.as_object();
    let defaults = source
        .and_then(|object| object.get("defaults"))
        .and_then(Value::as_object);

    ProjectRecord {
        id: replacement_string(source, "id"),
        name: replacement_string(source, "name"),
        description: replacement_string(source, "description"),
        icon: replacement_string(source, "icon"),
        created_at: replacement_timestamp(source, "createdAt"),
        updated_at: replacement_timestamp(source, "updatedAt"),
        defaults: normalize_defaults(defaults, &ProjectListOptions::default()),
    }
}

fn replacement_string(source: Option<&Map<String, Value>>, key: &str) -> String {
    source
        .and_then(|object| object.get(key))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn replacement_timestamp(source: Option<&Map<String, Value>>, key: &str) -> u64 {
    source
        .and_then(|object| object.get(key))
        .and_then(Value::as_u64)
        .unwrap_or_default()
}

fn parse_project_patch(updates: &Map<String, Value>) -> ProjectPatch {
    ProjectPatch {
        name: string_patch(updates.get("name")),
        icon: string_patch(updates.get("icon")),
        description: string_patch(updates.get("description")),
        defaults: updates
            .get("defaults")
            .and_then(Value::as_object)
            .map(parse_defaults_patch)
            .unwrap_or_default(),
    }
}

fn parse_defaults_patch(updates: &Map<String, Value>) -> ProjectDefaultsPatch {
    ProjectDefaultsPatch {
        summary_template_id: string_patch(updates.get("summaryTemplateId")),
        translation_language: string_patch(updates.get("translationLanguage")),
        polish_preset_id: string_patch(updates.get("polishPresetId")),
        polish_scenario: string_patch(updates.get("polishScenario")),
        polish_context: string_patch(updates.get("polishContext")),
        export_file_name_prefix: string_patch(updates.get("exportFileNamePrefix")),
        enabled_text_replacement_set_ids: string_array_patch(
            updates.get("enabledTextReplacementSetIds"),
        ),
        enabled_hotword_set_ids: string_array_patch(updates.get("enabledHotwordSetIds")),
        enabled_polish_keyword_set_ids: string_array_patch(
            updates.get("enabledPolishKeywordSetIds"),
        ),
        enabled_speaker_profile_ids: string_array_patch(updates.get("enabledSpeakerProfileIds")),
    }
}

fn string_patch(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(ToOwned::to_owned)
}

fn string_array_patch(value: Option<&Value>) -> Option<Vec<String>> {
    Some(
        value?
            .as_array()?
            .iter()
            .filter_map(Value::as_str)
            .map(ToOwned::to_owned)
            .collect(),
    )
}
