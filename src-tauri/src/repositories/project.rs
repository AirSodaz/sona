use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_store::StoreExt;
use uuid::Uuid;

const PROJECTS_DIR_NAME: &str = "projects";
const PROJECTS_INDEX_FILE_NAME: &str = "index.json";
const SETTINGS_FILE_NAME: &str = "settings.json";
const ACTIVE_PROJECT_SETTINGS_KEY: &str = "sona-active-project-id";
const DEFAULT_SUMMARY_TEMPLATE_ID: &str = "general";
const DEFAULT_TRANSLATION_LANGUAGE: &str = "zh";
const DEFAULT_POLISH_PRESET_ID: &str = "general";

static PROJECT_REPOSITORY_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug, Default)]
pub struct ProjectListOptions {
    pub fallback_enabled_polish_keyword_set_ids: Vec<String>,
    pub fallback_enabled_speaker_profile_ids: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDefaultsInput {
    pub summary_template_id: Option<String>,
    pub summary_template: Option<String>,
    pub translation_language: Option<String>,
    pub polish_preset_id: Option<String>,
    pub polish_scenario: Option<String>,
    pub polish_context: Option<String>,
    pub export_file_name_prefix: Option<String>,
    pub enabled_text_replacement_set_ids: Option<Vec<String>>,
    pub enabled_hotword_set_ids: Option<Vec<String>>,
    pub enabled_polish_keyword_set_ids: Option<Vec<String>>,
    pub enabled_speaker_profile_ids: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCreateInput {
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub defaults: ProjectDefaultsInput,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDefaults {
    pub summary_template_id: String,
    pub translation_language: String,
    pub polish_preset_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub polish_scenario: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub polish_context: Option<String>,
    pub export_file_name_prefix: String,
    pub enabled_text_replacement_set_ids: Vec<String>,
    pub enabled_hotword_set_ids: Vec<String>,
    pub enabled_polish_keyword_set_ids: Vec<String>,
    pub enabled_speaker_profile_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRecord {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub defaults: ProjectDefaults,
}

#[derive(Clone)]
pub struct ProjectRepository {
    app_local_data_dir: PathBuf,
}

impl ProjectRepository {
    pub fn new(app_local_data_dir: PathBuf) -> Self {
        Self { app_local_data_dir }
    }

    fn projects_dir(&self) -> PathBuf {
        self.app_local_data_dir.join(PROJECTS_DIR_NAME)
    }

    fn projects_index_path(&self) -> PathBuf {
        self.projects_dir().join(PROJECTS_INDEX_FILE_NAME)
    }

    pub fn ensure_ready(&self) -> Result<(), String> {
        fs::create_dir_all(self.projects_dir()).map_err(|error| error.to_string())?;
        let index_path = self.projects_index_path();
        if !index_path.exists() {
            write_json_pretty_atomic(&index_path, &Value::Array(Vec::new()))?;
        }
        Ok(())
    }

    pub fn list(&self, options: ProjectListOptions) -> Result<Vec<ProjectRecord>, String> {
        self.ensure_ready()?;
        let raw = read_json_value(&self.projects_index_path())?;
        let raw_projects = raw
            .as_array()
            .ok_or_else(|| "Project index must be an array.".to_string())?;

        let projects = raw_projects
            .iter()
            .map(|item| normalize_project_value(item, &options))
            .collect::<Vec<_>>();
        let normalized = serde_json::to_value(&projects).map_err(|error| error.to_string())?;
        if normalized != raw {
            self.write_projects(&projects)?;
        }
        Ok(projects)
    }

    pub fn save_all_values(&self, projects: Vec<Value>) -> Result<(), String> {
        self.ensure_ready()?;
        let normalized = projects
            .iter()
            .map(|project| normalize_project_value(project, &ProjectListOptions::default()))
            .collect::<Vec<_>>();
        self.write_projects(&normalized)
    }

    pub fn create(&self, input: ProjectCreateInput) -> Result<ProjectRecord, String> {
        let now = current_time_millis()?;
        let project = normalize_project_value(
            &json_object([
                ("id", Value::String(Uuid::new_v4().to_string())),
                ("name", Value::String(input.name)),
                (
                    "description",
                    Value::String(input.description.unwrap_or_default()),
                ),
                ("icon", Value::String(input.icon.unwrap_or_default())),
                ("createdAt", Value::Number(now.into())),
                ("updatedAt", Value::Number(now.into())),
                (
                    "defaults",
                    serde_json::to_value(input.defaults).map_err(|error| error.to_string())?,
                ),
            ]),
            &ProjectListOptions::default(),
        );

        let mut projects = self.list(ProjectListOptions::default())?;
        projects.insert(0, project.clone());
        self.write_projects(&projects)?;
        Ok(project)
    }

    pub fn update(
        &self,
        project_id: &str,
        updates: Value,
    ) -> Result<Option<ProjectRecord>, String> {
        let mut projects = self.list(ProjectListOptions::default())?;
        let Some(index) = projects.iter().position(|project| project.id == project_id) else {
            return Ok(None);
        };
        let Some(updates) = updates.as_object() else {
            return Ok(Some(projects[index].clone()));
        };

        let mut value =
            serde_json::to_value(&projects[index]).map_err(|error| error.to_string())?;
        let source = value
            .as_object_mut()
            .ok_or_else(|| "Normalized project must be an object.".to_string())?;

        copy_string_update(updates, source, "name");
        copy_string_update(updates, source, "description");
        copy_string_update(updates, source, "icon");

        if let Some(default_updates) = updates.get("defaults").and_then(Value::as_object) {
            let defaults = source
                .entry("defaults")
                .or_insert_with(|| Value::Object(Map::new()))
                .as_object_mut()
                .ok_or_else(|| "Project defaults must be an object.".to_string())?;
            for (key, value) in default_updates {
                defaults.insert(key.clone(), value.clone());
            }
        }

        source.insert(
            "updatedAt".to_string(),
            Value::Number(current_time_millis()?.into()),
        );

        let updated = normalize_project_value(&value, &ProjectListOptions::default());
        projects[index] = updated.clone();
        self.write_projects(&projects)?;
        Ok(Some(updated))
    }

    pub fn delete(&self, project_id: &str) -> Result<(), String> {
        let mut projects = self.list(ProjectListOptions::default())?;
        projects.retain(|project| project.id != project_id);
        self.write_projects(&projects)
    }

    pub fn reorder(&self, project_ids: Vec<String>) -> Result<Vec<ProjectRecord>, String> {
        let projects = self.list(ProjectListOptions::default())?;
        let mut by_id = projects
            .iter()
            .cloned()
            .map(|project| (project.id.clone(), project))
            .collect::<HashMap<_, _>>();
        let mut added_ids = HashSet::new();
        let mut reordered = Vec::with_capacity(projects.len());

        for project_id in project_ids {
            if added_ids.contains(&project_id) {
                continue;
            }
            if let Some(project) = by_id.remove(&project_id) {
                added_ids.insert(project_id);
                reordered.push(project);
            }
        }

        for project in projects {
            if !added_ids.contains(&project.id) {
                reordered.push(project);
            }
        }

        self.write_projects(&reordered)?;
        Ok(reordered)
    }

    fn write_projects(&self, projects: &[ProjectRecord]) -> Result<(), String> {
        write_json_pretty_atomic(&self.projects_index_path(), projects)
    }
}

pub(crate) fn normalize_project_record_for_import(input: &Value) -> Result<Value, String> {
    let project = normalize_project_value(input, &ProjectListOptions::default());
    serde_json::to_value(project).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn project_list<R: Runtime>(
    app: AppHandle<R>,
    fallback_enabled_polish_keyword_set_ids: Option<Vec<String>>,
    fallback_enabled_speaker_profile_ids: Option<Vec<String>>,
) -> Result<Vec<ProjectRecord>, String> {
    run_project_task(app, move |repository| {
        repository.list(ProjectListOptions {
            fallback_enabled_polish_keyword_set_ids: fallback_enabled_polish_keyword_set_ids
                .unwrap_or_default(),
            fallback_enabled_speaker_profile_ids: fallback_enabled_speaker_profile_ids
                .unwrap_or_default(),
        })
    })
    .await
}

#[tauri::command]
pub async fn project_save_all<R: Runtime>(
    app: AppHandle<R>,
    projects: Vec<Value>,
) -> Result<(), String> {
    run_project_task(app, move |repository| repository.save_all_values(projects)).await
}

#[tauri::command]
pub async fn project_create<R: Runtime>(
    app: AppHandle<R>,
    name: String,
    description: Option<String>,
    icon: Option<String>,
    defaults: ProjectDefaultsInput,
) -> Result<ProjectRecord, String> {
    run_project_task(app, move |repository| {
        repository.create(ProjectCreateInput {
            name,
            description,
            icon,
            defaults,
        })
    })
    .await
}

#[tauri::command]
pub async fn project_update<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
    updates: Value,
) -> Result<Option<ProjectRecord>, String> {
    run_project_task(app, move |repository| {
        repository.update(&project_id, updates)
    })
    .await
}

#[tauri::command]
pub async fn project_delete<R: Runtime>(
    app: AppHandle<R>,
    project_id: String,
) -> Result<(), String> {
    run_project_task(app, move |repository| repository.delete(&project_id)).await
}

#[tauri::command]
pub async fn project_reorder<R: Runtime>(
    app: AppHandle<R>,
    project_ids: Vec<String>,
) -> Result<Vec<ProjectRecord>, String> {
    run_project_task(app, move |repository| repository.reorder(project_ids)).await
}

#[tauri::command]
pub async fn project_get_active_id<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Option<String>, String> {
    let store = app
        .store(SETTINGS_FILE_NAME)
        .map_err(|error| error.to_string())?;
    Ok(store
        .get(ACTIVE_PROJECT_SETTINGS_KEY)
        .and_then(|value| value.as_str().map(str::trim).map(ToOwned::to_owned))
        .filter(|value| !value.is_empty()))
}

#[tauri::command]
pub async fn project_set_active_id<R: Runtime>(
    app: AppHandle<R>,
    project_id: Option<String>,
) -> Result<(), String> {
    let store = app
        .store(SETTINGS_FILE_NAME)
        .map_err(|error| error.to_string())?;
    store.set(
        ACTIVE_PROJECT_SETTINGS_KEY,
        project_id.map(Value::String).unwrap_or(Value::Null),
    );
    store.save().map_err(|error| error.to_string())
}

#[allow(dead_code)]
pub fn get_active_project_id_from_dir(app_data_dir: &Path) -> Result<Option<String>, String> {
    let settings = read_settings(app_data_dir)?;
    Ok(settings
        .get(ACTIVE_PROJECT_SETTINGS_KEY)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned))
}

#[allow(dead_code)]
pub fn set_active_project_id_in_dir(
    app_data_dir: &Path,
    project_id: Option<String>,
) -> Result<(), String> {
    let mut settings = read_settings(app_data_dir)?;
    settings.insert(
        ACTIVE_PROJECT_SETTINGS_KEY.to_string(),
        project_id.map(Value::String).unwrap_or(Value::Null),
    );
    write_json_pretty_atomic(
        &app_data_dir.join(SETTINGS_FILE_NAME),
        &Value::Object(settings),
    )
}

async fn run_project_task<R, T, F>(app: AppHandle<R>, task: F) -> Result<T, String>
where
    R: Runtime,
    T: Send + 'static,
    F: FnOnce(ProjectRepository) -> Result<T, String> + Send + 'static,
{
    let app_local_data_dir = resolve_app_local_data_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = PROJECT_REPOSITORY_LOCK
            .lock()
            .map_err(|error| error.to_string())?;
        task(ProjectRepository::new(app_local_data_dir))
    })
    .await
    .map_err(|error| error.to_string())?
}

fn resolve_app_local_data_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    app.path()
        .app_local_data_dir()
        .map_err(|error| error.to_string())
}

fn normalize_project_value(input: &Value, options: &ProjectListOptions) -> ProjectRecord {
    let now = current_time_millis().unwrap_or(0);
    let source = input.as_object();
    let defaults = source
        .and_then(|object| object.get("defaults"))
        .and_then(Value::as_object);
    let created_at =
        positive_millis(source.and_then(|object| object.get("createdAt"))).unwrap_or(now);

    ProjectRecord {
        id: string_value(source.and_then(|object| object.get("id"))).unwrap_or_default(),
        name: non_empty_trimmed_string(source.and_then(|object| object.get("name")))
            .unwrap_or_else(|| "Untitled Project".to_string()),
        description: string_value(source.and_then(|object| object.get("description")))
            .unwrap_or_default(),
        icon: string_value(source.and_then(|object| object.get("icon"))).unwrap_or_default(),
        created_at,
        updated_at: positive_millis(source.and_then(|object| object.get("updatedAt")))
            .unwrap_or(created_at),
        defaults: normalize_defaults(defaults, options),
    }
}

fn normalize_defaults(
    source: Option<&Map<String, Value>>,
    options: &ProjectListOptions,
) -> ProjectDefaults {
    let polish_scenario = string_value(source.and_then(|object| object.get("polishScenario")));
    let polish_context = string_value(source.and_then(|object| object.get("polishContext")));
    let polish_preset_id = string_value(source.and_then(|object| object.get("polishPresetId")))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            if polish_scenario.as_deref().unwrap_or_default().is_empty()
                && polish_context.as_deref().unwrap_or_default().is_empty()
            {
                DEFAULT_POLISH_PRESET_ID.to_string()
            } else {
                String::new()
            }
        });

    ProjectDefaults {
        summary_template_id: non_empty_trimmed_string(
            source.and_then(|object| object.get("summaryTemplateId")),
        )
        .or_else(|| {
            non_empty_trimmed_string(source.and_then(|object| object.get("summaryTemplate")))
        })
        .unwrap_or_else(|| DEFAULT_SUMMARY_TEMPLATE_ID.to_string()),
        translation_language: string_value(
            source.and_then(|object| object.get("translationLanguage")),
        )
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_TRANSLATION_LANGUAGE.to_string()),
        polish_preset_id,
        polish_scenario,
        polish_context,
        export_file_name_prefix: string_value(
            source.and_then(|object| object.get("exportFileNamePrefix")),
        )
        .unwrap_or_default(),
        enabled_text_replacement_set_ids: string_array(
            source.and_then(|object| object.get("enabledTextReplacementSetIds")),
        )
        .unwrap_or_default(),
        enabled_hotword_set_ids: string_array(
            source.and_then(|object| object.get("enabledHotwordSetIds")),
        )
        .unwrap_or_default(),
        enabled_polish_keyword_set_ids: string_array(
            source.and_then(|object| object.get("enabledPolishKeywordSetIds")),
        )
        .unwrap_or_else(|| options.fallback_enabled_polish_keyword_set_ids.clone()),
        enabled_speaker_profile_ids: string_array(
            source.and_then(|object| object.get("enabledSpeakerProfileIds")),
        )
        .unwrap_or_else(|| options.fallback_enabled_speaker_profile_ids.clone()),
    }
}

#[allow(dead_code)]
fn read_settings(app_data_dir: &Path) -> Result<Map<String, Value>, String> {
    let path = app_data_dir.join(SETTINGS_FILE_NAME);
    match fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str::<Value>(&content) {
            Ok(Value::Object(settings)) => Ok(settings),
            Ok(_) => Ok(Map::new()),
            Err(error) => Err(error.to_string()),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Map::new()),
        Err(error) => Err(error.to_string()),
    }
}

fn read_json_value(path: &Path) -> Result<Value, String> {
    let content = fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&content).map_err(|error| error.to_string())
}

fn write_json_pretty_atomic<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), String> {
    let serialized = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    write_binary_atomic(path, &serialized)
}

fn write_binary_atomic(path: &Path, contents: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("json");
    let temp_path = path.with_extension(format!("{extension}.tmp-{}", Uuid::new_v4()));
    fs::write(&temp_path, contents).map_err(|error| error.to_string())?;
    match fs::rename(&temp_path, path) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(&temp_path);
            Err(error.to_string())
        }
    }
}

fn current_time_millis() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .map_err(|error| error.to_string())
}

fn positive_millis(value: Option<&Value>) -> Option<u64> {
    match value.and_then(Value::as_f64) {
        Some(value) if value.is_finite() && value > 0.0 => Some(value.round() as u64),
        _ => None,
    }
}

fn string_value(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(ToOwned::to_owned)
}

fn non_empty_trimmed_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn string_array(value: Option<&Value>) -> Option<Vec<String>> {
    Some(
        value?
            .as_array()?
            .iter()
            .filter_map(Value::as_str)
            .map(ToOwned::to_owned)
            .collect(),
    )
}

fn json_object<const N: usize>(entries: [(&str, Value); N]) -> Value {
    Value::Object(
        entries
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
    )
}

fn copy_string_update(updates: &Map<String, Value>, target: &mut Map<String, Value>, key: &str) {
    if let Some(value) = updates.get(key).and_then(Value::as_str) {
        target.insert(key.to_string(), Value::String(value.to_string()));
    }
}
