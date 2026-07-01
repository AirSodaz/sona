use crate::core::database::Database;
use serde_json::{Map, Value, json};
use std::path::PathBuf;

use super::types::{ProjectCreateInput, ProjectDefaults, ProjectListOptions, ProjectRecord};

#[derive(Clone)]
pub struct SqliteProjectRepository {
    #[allow(dead_code)]
    app_local_data_dir: PathBuf,
    db: crate::core::database::DbProvider,
}

impl SqliteProjectRepository {
    pub fn new(app_local_data_dir: PathBuf) -> Self {
        Self {
            app_local_data_dir,
            db: crate::core::database::DbProvider::default(),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_db(app_local_data_dir: PathBuf, db: Database) -> Self {
        Self {
            app_local_data_dir,
            db: crate::core::database::DbProvider::new(Some(std::sync::Arc::new(db))),
        }
    }

    fn get_db(&self) -> &Database {
        self.db.get()
    }

    pub fn list(&self, _options: ProjectListOptions) -> Result<Vec<ProjectRecord>, String> {
        self.get_db().with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, icon, color, sort_order, created_at, updated_at, summary_template_id, translation_language, polish_preset_id, settings
                 FROM projects
                 ORDER BY sort_order"
            )?;
            let rows = stmt.query_map([], map_row_to_project)?;
            let mut projects = Vec::new();
            for row in rows {
                projects.push(row?);
            }
            Ok(projects)
        })
    }

    pub fn create(&self, input: ProjectCreateInput) -> Result<ProjectRecord, String> {
        let now = crate::repositories::project::repository::current_time_millis()?;
        let id = uuid::Uuid::new_v4().to_string();

        let defaults = self.build_defaults(&input)?;
        let mut settings_val = serde_json::to_value(&defaults).map_err(|e| e.to_string())?;
        if let Some(obj) = settings_val.as_object_mut() {
            obj.insert("description".to_string(), json!(input.description));
        }
        let settings_str = settings_val.to_string();
        let icon = input.icon.clone().unwrap_or_default();
        let name = input.name.clone();

        self.get_db().with_connection(|conn| {
            conn.execute(
                "INSERT INTO projects (id, name, icon, color, sort_order, created_at, updated_at, summary_template_id, translation_language, polish_preset_id, settings)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                rusqlite::params![
                    id,
                    &name,
                    &icon,
                    "",
                    0,
                    now as i64,
                    now as i64,
                    defaults.summary_template_id,
                    defaults.translation_language,
                    defaults.polish_preset_id,
                    settings_str,
                ],
            )?;
            Ok(())
        })?;

        Ok(ProjectRecord {
            id,
            name,
            description: input.description.unwrap_or_default(),
            icon,
            created_at: now,
            updated_at: now,
            defaults,
        })
    }

    pub fn update(
        &self,
        project_id: &str,
        updates: Value,
    ) -> Result<Option<ProjectRecord>, String> {
        let Some(updates_obj) = updates.as_object() else {
            return self.get_by_id(project_id);
        };

        let existing = match self.get_by_id(project_id)? {
            Some(p) => p,
            None => return Ok(None),
        };

        let name = updates_obj
            .get("name")
            .and_then(Value::as_str)
            .map(|s| s.to_string())
            .unwrap_or(existing.name);

        let icon = updates_obj
            .get("icon")
            .and_then(Value::as_str)
            .map(|s| s.to_string())
            .unwrap_or(existing.icon);

        let description = updates_obj
            .get("description")
            .and_then(Value::as_str)
            .map(|s| s.to_string())
            .unwrap_or(existing.description);

        let mut defaults = existing.defaults;
        if let Some(default_updates) = updates_obj.get("defaults").and_then(Value::as_object) {
            if let Some(v) = default_updates
                .get("summaryTemplateId")
                .and_then(Value::as_str)
            {
                defaults.summary_template_id = v.to_string();
            }
            if let Some(v) = default_updates
                .get("translationLanguage")
                .and_then(Value::as_str)
            {
                defaults.translation_language = v.to_string();
            }
            if let Some(v) = default_updates
                .get("polishPresetId")
                .and_then(Value::as_str)
            {
                defaults.polish_preset_id = v.to_string();
            }
            if let Some(v) = default_updates
                .get("polishScenario")
                .and_then(Value::as_str)
            {
                defaults.polish_scenario = Some(v.to_string());
            }
            if let Some(v) = default_updates.get("polishContext").and_then(Value::as_str) {
                defaults.polish_context = Some(v.to_string());
            }
            if let Some(v) = default_updates
                .get("exportFileNamePrefix")
                .and_then(Value::as_str)
            {
                defaults.export_file_name_prefix = v.to_string();
            }
            if let Some(v) = default_updates
                .get("enabledTextReplacementSetIds")
                .and_then(Value::as_array)
            {
                defaults.enabled_text_replacement_set_ids = v
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|s| s.to_string())
                    .collect();
            }
            if let Some(v) = default_updates
                .get("enabledHotwordSetIds")
                .and_then(Value::as_array)
            {
                defaults.enabled_hotword_set_ids = v
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|s| s.to_string())
                    .collect();
            }
            if let Some(v) = default_updates
                .get("enabledPolishKeywordSetIds")
                .and_then(Value::as_array)
            {
                defaults.enabled_polish_keyword_set_ids = v
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|s| s.to_string())
                    .collect();
            }
            if let Some(v) = default_updates
                .get("enabledSpeakerProfileIds")
                .and_then(Value::as_array)
            {
                defaults.enabled_speaker_profile_ids = v
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|s| s.to_string())
                    .collect();
            }
        }

        let now = crate::repositories::project::repository::current_time_millis()?;
        let mut settings_val = serde_json::to_value(&defaults).map_err(|e| e.to_string())?;
        if let Some(obj) = settings_val.as_object_mut() {
            obj.insert("description".to_string(), json!(description));
        }
        let settings_str = settings_val.to_string();

        self.get_db().with_connection(|conn| {
            conn.execute(
                "UPDATE projects SET name = ?1, icon = ?2, updated_at = ?3, summary_template_id = ?4, translation_language = ?5, polish_preset_id = ?6, settings = ?7 WHERE id = ?8",
                rusqlite::params![
                    name,
                    icon,
                    now as i64,
                    defaults.summary_template_id,
                    defaults.translation_language,
                    defaults.polish_preset_id,
                    settings_str,
                    project_id,
                ],
            )?;
            Ok(())
        })?;

        Ok(Some(ProjectRecord {
            id: project_id.to_string(),
            name,
            description,
            icon,
            created_at: existing.created_at,
            updated_at: now,
            defaults,
        }))
    }

    pub fn delete(&self, project_id: &str) -> Result<(), String> {
        self.get_db().with_connection(|conn| {
            conn.execute("DELETE FROM projects WHERE id = ?1", [project_id])?;
            Ok(())
        })
    }

    pub fn save_all_values(&self, projects: Vec<Value>) -> Result<(), String> {
        self.get_db().with_transaction(|tx| {
            tx.execute("DELETE FROM projects", [])?;
            let mut stmt = tx.prepare(
                "INSERT INTO projects (id, name, icon, color, sort_order, created_at, updated_at, summary_template_id, translation_language, polish_preset_id, settings)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"
            )?;
            for (i, project) in projects.iter().enumerate() {
                let id = project.get("id").and_then(Value::as_str).unwrap_or("").to_string();
                let name = project.get("name").and_then(Value::as_str).unwrap_or("").to_string();
                let icon = project.get("icon").and_then(Value::as_str).unwrap_or("").to_string();
                let created_at = project.get("createdAt").and_then(Value::as_u64).unwrap_or(0) as i64;
                let updated_at = project.get("updatedAt").and_then(Value::as_u64).unwrap_or(0) as i64;

                let defaults = project.get("defaults").and_then(Value::as_object);
                let summary_template_id = defaults
                    .and_then(|d| d.get("summaryTemplateId"))
                    .or_else(|| defaults.and_then(|d| d.get("summaryTemplate")))
                    .and_then(Value::as_str)
                    .unwrap_or("general");
                let translation_language = defaults
                    .and_then(|d| d.get("translationLanguage"))
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .unwrap_or("zh");
                let polish_preset_id = defaults
                    .and_then(|d| d.get("polishPresetId"))
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .unwrap_or("general");

                let mut settings = serde_json::Map::new();
                if let Some(desc) = project.get("description").and_then(Value::as_str) {
                    settings.insert("description".to_string(), json!(desc));
                }
                if let Some(d) = defaults {
                    for key in &["polishScenario", "polishContext", "exportFileNamePrefix",
                        "enabledTextReplacementSetIds", "enabledHotwordSetIds",
                        "enabledPolishKeywordSetIds", "enabledSpeakerProfileIds"]
                    {
                        if let Some(val) = d.get(*key) {
                            settings.insert(key.to_string(), val.clone());
                        }
                    }
                }
                let settings_str = serde_json::to_string(&Value::Object(settings)).unwrap_or_else(|_| "{}".to_string());

                stmt.execute(rusqlite::params![
                    id, name, icon, "", i as i64, created_at, updated_at,
                    summary_template_id, translation_language, polish_preset_id, settings_str,
                ])?;
            }
            Ok(())
        })
    }

    pub fn reorder(&self, project_ids: Vec<String>) -> Result<Vec<ProjectRecord>, String> {
        self.get_db().with_transaction(|tx| {
            let mut stmt = tx.prepare("UPDATE projects SET sort_order = ?1 WHERE id = ?2")?;
            for (i, id) in project_ids.iter().enumerate() {
                stmt.execute(rusqlite::params![i as i64, id])?;
            }
            Ok(())
        })?;

        self.list(ProjectListOptions::default())
    }

    fn get_by_id(&self, project_id: &str) -> Result<Option<ProjectRecord>, String> {
        self.get_db().with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, icon, color, sort_order, created_at, updated_at, summary_template_id, translation_language, polish_preset_id, settings
                 FROM projects WHERE id = ?1"
            )?;
            let mut rows = stmt.query([project_id])?;
            if let Some(row) = rows.next()? {
                Ok(Some(map_row_to_project(row)?))
            } else {
                Ok(None)
            }
        })
    }

    fn build_defaults(&self, input: &ProjectCreateInput) -> Result<ProjectDefaults, String> {
        let d = &input.defaults;
        Ok(ProjectDefaults {
            summary_template_id: d
                .summary_template_id
                .clone()
                .unwrap_or_else(|| "general".to_string()),
            translation_language: d
                .translation_language
                .clone()
                .unwrap_or_else(|| "zh".to_string()),
            polish_preset_id: d
                .polish_preset_id
                .clone()
                .unwrap_or_else(|| "general".to_string()),
            polish_scenario: d.polish_scenario.clone(),
            polish_context: d.polish_context.clone(),
            export_file_name_prefix: d.export_file_name_prefix.clone().unwrap_or_default(),
            enabled_text_replacement_set_ids: d
                .enabled_text_replacement_set_ids
                .clone()
                .unwrap_or_default(),
            enabled_hotword_set_ids: d.enabled_hotword_set_ids.clone().unwrap_or_default(),
            enabled_polish_keyword_set_ids: d
                .enabled_polish_keyword_set_ids
                .clone()
                .unwrap_or_default(),
            enabled_speaker_profile_ids: d.enabled_speaker_profile_ids.clone().unwrap_or_default(),
        })
    }
}

fn map_row_to_project(row: &rusqlite::Row) -> rusqlite::Result<ProjectRecord> {
    let id: String = row.get(0)?;
    let name: String = row.get(1)?;
    let icon: String = row.get(2)?;
    let _color: String = row.get(3)?;
    let _sort_order: i64 = row.get(4)?;
    let created_at: i64 = row.get(5)?;
    let updated_at: i64 = row.get(6)?;
    let summary_template_id: String = row.get(7)?;
    let translation_language: String = row.get(8)?;
    let polish_preset_id: String = row.get(9)?;
    let settings_str: String = row.get(10)?;

    let settings: Map<String, Value> = serde_json::from_str(&settings_str).unwrap_or_default();

    let description = settings
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    let defaults = ProjectDefaults {
        summary_template_id,
        translation_language,
        polish_preset_id,
        polish_scenario: settings
            .get("polishScenario")
            .and_then(Value::as_str)
            .map(|s| s.to_string()),
        polish_context: settings
            .get("polishContext")
            .and_then(Value::as_str)
            .map(|s| s.to_string()),
        export_file_name_prefix: settings
            .get("exportFileNamePrefix")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        enabled_text_replacement_set_ids: settings
            .get("enabledTextReplacementSetIds")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default(),
        enabled_hotword_set_ids: settings
            .get("enabledHotwordSetIds")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default(),
        enabled_polish_keyword_set_ids: settings
            .get("enabledPolishKeywordSetIds")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default(),
        enabled_speaker_profile_ids: settings
            .get("enabledSpeakerProfileIds")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default(),
    };

    Ok(ProjectRecord {
        id,
        name,
        description,
        icon,
        created_at: created_at as u64,
        updated_at: updated_at as u64,
        defaults,
    })
}

#[async_trait::async_trait]
impl crate::core::dashboard::ports::ProjectRepository for SqliteProjectRepository {
    async fn count_projects(
        &self,
    ) -> Result<u64, crate::core::dashboard::error::DashboardServiceError> {
        let projects = self
            .list(ProjectListOptions::default())
            .map_err(crate::core::dashboard::error::DashboardServiceError::ProjectRepository)?;
        Ok(projects.len() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::database::Database;
    use serde_json::json;

    #[test]
    fn test_project_crud() {
        let db = Database::open_in_memory().unwrap();
        let repo = SqliteProjectRepository::with_db(PathBuf::new(), db);

        // Create
        let created = repo
            .create(ProjectCreateInput {
                name: "Test Project".to_string(),
                description: Some("A test".to_string()),
                icon: Some("folder".to_string()),
                defaults: Default::default(),
            })
            .unwrap();
        assert_eq!(created.name, "Test Project");
        assert_eq!(created.description, "A test");
        assert_eq!(created.icon, "folder");

        // List
        let projects = repo.list(ProjectListOptions::default()).unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].id, created.id);

        // Update
        let updated = repo
            .update(&created.id, json!({"name": "Updated"}))
            .unwrap();
        assert!(updated.is_some());
        assert_eq!(updated.unwrap().name, "Updated");

        // Reorder
        let reordered = repo.reorder(vec![created.id.clone()]).unwrap();
        assert_eq!(reordered.len(), 1);

        // Delete
        repo.delete(&created.id).unwrap();
        let projects = repo.list(ProjectListOptions::default()).unwrap();
        assert!(projects.is_empty());
    }

    #[test]
    fn test_project_update_nonexistent() {
        let db = Database::open_in_memory().unwrap();
        let repo = SqliteProjectRepository::with_db(PathBuf::new(), db);

        let result = repo.update("nonexistent", json!({"name": "Nope"})).unwrap();
        assert!(result.is_none());
    }
}
