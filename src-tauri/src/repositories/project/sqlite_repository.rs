use crate::core::database::DatabaseError;
use serde_json::{Map, Value, json};

use super::types::{ProjectCreateInput, ProjectDefaults, ProjectListOptions, ProjectRecord};

#[derive(Clone)]
pub struct SqliteProjectRepository {
    db: crate::core::database::DbProvider,
}

crate::impl_db_repository!(SqliteProjectRepository);

const PROJECT_COLUMNS: [&str; 11] = [
    "id",
    "name",
    "icon",
    "color",
    "sort_order",
    "created_at",
    "updated_at",
    "summary_template_id",
    "translation_language",
    "polish_preset_id",
    "settings",
];

const PROJECT_UPDATE_COLUMNS: [&str; 7] = [
    "name",
    "icon",
    "updated_at",
    "summary_template_id",
    "translation_language",
    "polish_preset_id",
    "settings",
];

fn project_column_list(columns: &[&str]) -> String {
    columns.join(", ")
}

fn project_named_param_list(columns: &[&str]) -> String {
    columns
        .iter()
        .map(|column| format!(":{column}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn project_select_columns() -> String {
    project_column_list(&PROJECT_COLUMNS)
}

fn project_insert_sql() -> String {
    format!(
        "INSERT INTO projects ({}) VALUES ({})",
        project_column_list(&PROJECT_COLUMNS),
        project_named_param_list(&PROJECT_COLUMNS)
    )
}

fn project_update_sql() -> String {
    let assignments = PROJECT_UPDATE_COLUMNS
        .iter()
        .map(|column| format!("{column} = :{column}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("UPDATE projects SET {assignments} WHERE id = :id")
}

impl SqliteProjectRepository {
    pub fn list(&self, _options: ProjectListOptions) -> Result<Vec<ProjectRecord>, DatabaseError> {
        self.get_db()?.with_connection(Self::list_projects)
    }

    pub fn create(&self, input: ProjectCreateInput) -> Result<ProjectRecord, DatabaseError> {
        let now = crate::repositories::project::repository::current_time_millis()
            .map_err(DatabaseError::Internal)?;
        let id = uuid::Uuid::new_v4().to_string();

        let defaults = self.build_defaults(&input)?;
        let mut settings_val = serde_json::to_value(&defaults)?;
        if let Some(obj) = settings_val.as_object_mut() {
            obj.insert("description".to_string(), json!(input.description));
        }
        let settings_str = settings_val.to_string();
        let icon = input.icon.clone().unwrap_or_default();
        let name = input.name.clone();

        self.get_db()?.with_connection(|conn| {
            conn.execute(
                &project_insert_sql(),
                rusqlite::named_params! {
                    ":id": &id,
                    ":name": &name,
                    ":icon": &icon,
                    ":color": "",
                    ":sort_order": 0_i64,
                    ":created_at": now as i64,
                    ":updated_at": now as i64,
                    ":summary_template_id": &defaults.summary_template_id,
                    ":translation_language": &defaults.translation_language,
                    ":polish_preset_id": &defaults.polish_preset_id,
                    ":settings": &settings_str,
                },
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
    ) -> Result<Option<ProjectRecord>, DatabaseError> {
        let Some(updates_obj) = updates.as_object() else {
            return self.get_by_id(project_id);
        };

        self.get_db()?.with_rw_transaction(|tx| {
            let existing = match Self::get_by_id_from_conn(tx, project_id)? {
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

            let now = crate::repositories::project::repository::current_time_millis()
                .map_err(DatabaseError::Internal)?;
            let mut settings_val = serde_json::to_value(&defaults)?;
            if let Some(obj) = settings_val.as_object_mut() {
                obj.insert("description".to_string(), json!(description));
            }
            let settings_str = settings_val.to_string();

            let rows_affected = tx.execute(
                &project_update_sql(),
                rusqlite::named_params! {
                    ":name": &name,
                    ":icon": &icon,
                    ":updated_at": now as i64,
                    ":summary_template_id": &defaults.summary_template_id,
                    ":translation_language": &defaults.translation_language,
                    ":polish_preset_id": &defaults.polish_preset_id,
                    ":settings": &settings_str,
                    ":id": project_id,
                },
            )?;
            if rows_affected == 0 {
                return Ok(None);
            }

            Ok(Some(ProjectRecord {
                id: project_id.to_string(),
                name,
                description,
                icon,
                created_at: existing.created_at,
                updated_at: now,
                defaults,
            }))
        })
    }

    pub fn delete(&self, project_id: &str) -> Result<(), DatabaseError> {
        self.get_db()?.with_transaction(|tx| {
            tx.execute(
                "UPDATE history_items SET project_id = NULL WHERE project_id = ?1",
                [project_id],
            )?;
            tx.execute("DELETE FROM projects WHERE id = ?1", [project_id])?;
            Ok(())
        })
    }

    pub fn save_all_values(&self, projects: Vec<Value>) -> Result<(), DatabaseError> {
        self.get_db()?.with_transaction(|tx| {
            tx.execute("DELETE FROM projects", [])?;
            let insert_sql = project_insert_sql();
            let mut stmt = tx.prepare_cached(&insert_sql)?;
            for (i, project) in projects.iter().enumerate() {
                let id = project
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let name = project
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let icon = project
                    .get("icon")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let created_at = project
                    .get("createdAt")
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as i64;
                let updated_at = project
                    .get("updatedAt")
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as i64;

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
                    for key in &[
                        "polishScenario",
                        "polishContext",
                        "exportFileNamePrefix",
                        "enabledTextReplacementSetIds",
                        "enabledHotwordSetIds",
                        "enabledPolishKeywordSetIds",
                        "enabledSpeakerProfileIds",
                    ] {
                        if let Some(val) = d.get(*key) {
                            settings.insert(key.to_string(), val.clone());
                        }
                    }
                }
                let settings_str = serde_json::to_string(&Value::Object(settings))
                    .unwrap_or_else(|_| "{}".to_string());

                stmt.execute(rusqlite::named_params! {
                    ":id": &id,
                    ":name": &name,
                    ":icon": &icon,
                    ":color": "",
                    ":sort_order": i as i64,
                    ":created_at": created_at,
                    ":updated_at": updated_at,
                    ":summary_template_id": summary_template_id,
                    ":translation_language": translation_language,
                    ":polish_preset_id": polish_preset_id,
                    ":settings": &settings_str,
                })?;
            }
            tx.execute(
                "UPDATE history_items
                 SET project_id = NULL
                 WHERE project_id IS NOT NULL
                   AND project_id NOT IN (SELECT id FROM projects)",
                [],
            )?;
            Ok(())
        })
    }

    pub fn reorder(&self, project_ids: Vec<String>) -> Result<Vec<ProjectRecord>, DatabaseError> {
        self.get_db()?.with_transaction(|tx| {
            let mut stmt =
                tx.prepare_cached("UPDATE projects SET sort_order = ?1 WHERE id = ?2")?;
            for (i, id) in project_ids.iter().enumerate() {
                stmt.execute(rusqlite::params![i as i64, id])?;
            }
            Self::list_projects(tx)
        })
    }

    fn get_by_id(&self, project_id: &str) -> Result<Option<ProjectRecord>, DatabaseError> {
        self.get_db()?
            .with_connection(|conn| Self::get_by_id_from_conn(conn, project_id))
    }

    fn list_projects(conn: &rusqlite::Connection) -> Result<Vec<ProjectRecord>, DatabaseError> {
        let columns = project_select_columns();
        let mut stmt = conn.prepare_cached(&format!(
            "SELECT {columns} FROM projects ORDER BY sort_order"
        ))?;
        let rows = stmt.query_map([], map_row_to_project)?;
        let mut projects = Vec::new();
        for row in rows {
            projects.push(row?);
        }
        Ok(projects)
    }

    fn get_by_id_from_conn(
        conn: &rusqlite::Connection,
        project_id: &str,
    ) -> Result<Option<ProjectRecord>, DatabaseError> {
        let columns = project_select_columns();
        let mut stmt =
            conn.prepare_cached(&format!("SELECT {columns} FROM projects WHERE id = ?1"))?;
        let mut rows = stmt.query([project_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(map_row_to_project(row)?))
        } else {
            Ok(None)
        }
    }

    fn build_defaults(&self, input: &ProjectCreateInput) -> Result<ProjectDefaults, DatabaseError> {
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
    let id: String = row.get("id")?;
    let name: String = row.get("name")?;
    let icon: String = row.get("icon")?;
    let _color: String = row.get("color")?;
    let _sort_order: i64 = row.get("sort_order")?;
    let created_at: i64 = row.get("created_at")?;
    let updated_at: i64 = row.get("updated_at")?;
    let summary_template_id: String = row.get("summary_template_id")?;
    let translation_language: String = row.get("translation_language")?;
    let polish_preset_id: String = row.get("polish_preset_id")?;
    let settings_str: String = row.get("settings")?;

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
    use std::path::PathBuf;

    fn table_columns(conn: &rusqlite::Connection, table: &str) -> Vec<String> {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .unwrap();
        let rows = stmt.query_map([], |row| row.get::<_, String>(1)).unwrap();

        rows.collect::<Result<Vec<_>, _>>().unwrap()
    }

    #[test]
    fn project_column_shape_matches_schema() {
        let db = Database::open_in_memory().unwrap();
        db.with_connection(|conn| {
            let expected: Vec<String> = PROJECT_COLUMNS
                .iter()
                .map(|column| (*column).to_string())
                .collect();

            assert_eq!(table_columns(conn, "projects"), expected);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn project_row_mapper_reads_columns_by_name() {
        let db = Database::open_in_memory().unwrap();
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO projects (
                    id, name, icon, color, sort_order, created_at, updated_at,
                    summary_template_id, translation_language, polish_preset_id, settings
                )
                VALUES (
                    'project-name-map', 'Mapped project', 'folder', '#fff', 7, 111, 222,
                    'summary-special', 'en', 'polish-special',
                    '{\"description\":\"Mapped description\",\"exportFileNamePrefix\":\"map-\"}'
                )",
                [],
            )?;

            let mut stmt = conn.prepare(
                "SELECT
                    settings AS settings,
                    polish_preset_id AS polish_preset_id,
                    translation_language AS translation_language,
                    summary_template_id AS summary_template_id,
                    updated_at AS updated_at,
                    created_at AS created_at,
                    sort_order AS sort_order,
                    color AS color,
                    icon AS icon,
                    name AS name,
                    id AS id
                 FROM projects
                 WHERE id = 'project-name-map'",
            )?;
            let project = stmt.query_row([], map_row_to_project)?;

            assert_eq!(project.id, "project-name-map");
            assert_eq!(project.name, "Mapped project");
            assert_eq!(project.description, "Mapped description");
            assert_eq!(project.icon, "folder");
            assert_eq!(project.created_at, 111);
            assert_eq!(project.updated_at, 222);
            assert_eq!(project.defaults.summary_template_id, "summary-special");
            assert_eq!(project.defaults.translation_language, "en");
            assert_eq!(project.defaults.polish_preset_id, "polish-special");
            assert_eq!(project.defaults.export_file_name_prefix, "map-");
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn project_insert_and_update_sql_use_named_params_for_expected_columns() {
        let insert_sql = project_insert_sql();
        assert!(!insert_sql.contains('?'));
        for column in PROJECT_COLUMNS {
            assert!(
                insert_sql.contains(&format!(":{column}")),
                "missing insert named param for {column} in {insert_sql}"
            );
        }
        assert_eq!(insert_sql.matches(':').count(), PROJECT_COLUMNS.len());

        let update_sql = project_update_sql();
        assert!(!update_sql.contains('?'));
        for column in PROJECT_UPDATE_COLUMNS {
            assert!(
                update_sql.contains(&format!("{column} = :{column}")),
                "missing update assignment for {column} in {update_sql}"
            );
        }
        assert!(update_sql.contains("WHERE id = :id"));
        assert_eq!(
            update_sql.matches(':').count(),
            PROJECT_UPDATE_COLUMNS.len() + 1
        );
    }

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

    #[test]
    fn delete_nulls_history_assignments_and_removes_project() {
        let db = Database::open_in_memory().unwrap();
        let repo = SqliteProjectRepository::with_db(PathBuf::new(), db);

        repo.get_db()
            .unwrap()
            .with_transaction(|tx| {
                tx.execute(
                    "INSERT INTO projects (id, name, icon, color, sort_order, created_at, updated_at, summary_template_id, translation_language, polish_preset_id, settings)
                 VALUES ('project-delete', 'Delete Me', 'folder', '', 0, 1000, 1000, 'general', 'zh', 'general', '{}')",
                    [],
                )?;
                tx.execute(
                    "INSERT INTO history_items (id, timestamp, duration, title, kind, project_id, status)
                 VALUES ('item-delete', 1000, 1.0, 'Assigned Item', 'recording', 'project-delete', 'complete')",
                    [],
                )?;
                Ok(())
            })
            .unwrap();

        repo.delete("project-delete").unwrap();

        repo.get_db()
            .unwrap()
            .with_connection(|conn| {
                let project_count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM projects WHERE id = 'project-delete'",
                    [],
                    |row| row.get(0),
                )?;
                let history_project_id: Option<String> = conn.query_row(
                    "SELECT project_id FROM history_items WHERE id = 'item-delete'",
                    [],
                    |row| row.get(0),
                )?;

                assert_eq!(project_count, 0);
                assert_eq!(history_project_id, None);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn reorder_returns_transaction_consistent_order() {
        let db = Database::open_in_memory().unwrap();
        let repo = SqliteProjectRepository::with_db(PathBuf::new(), db);

        repo.get_db()
            .unwrap()
            .with_transaction(|tx| {
                for (sort_order, id, name) in [
                    (0, "project-a", "Alpha"),
                    (1, "project-b", "Beta"),
                    (2, "project-c", "Gamma"),
                ] {
                    tx.execute(
                        "INSERT INTO projects (id, name, icon, color, sort_order, created_at, updated_at, summary_template_id, translation_language, polish_preset_id, settings)
                     VALUES (?1, ?2, 'folder', '', ?3, 1000, 1000, 'general', 'zh', 'general', '{}')",
                        rusqlite::params![id, name, sort_order],
                    )?;
                }
                Ok(())
            })
            .unwrap();

        let reordered = repo
            .reorder(vec![
                "project-c".to_string(),
                "project-a".to_string(),
                "project-b".to_string(),
            ])
            .unwrap();

        let ids: Vec<&str> = reordered
            .iter()
            .map(|project| project.id.as_str())
            .collect();
        assert_eq!(ids, vec!["project-c", "project-a", "project-b"]);
    }

    #[test]
    fn save_all_values_preserves_surviving_project_assignments() {
        let db = Database::open_in_memory().unwrap();
        let repo = SqliteProjectRepository::with_db(PathBuf::new(), db);

        repo.get_db()
            .unwrap()
            .with_transaction(|tx| {
                tx.execute(
                    "INSERT INTO projects (id, name, icon, color, sort_order, created_at, updated_at, summary_template_id, translation_language, polish_preset_id, settings)
                 VALUES ('project-kept', 'Kept', 'folder', '', 0, 1000, 1000, 'general', 'zh', 'general', '{}')",
                    [],
                )?;
                tx.execute(
                    "INSERT INTO projects (id, name, icon, color, sort_order, created_at, updated_at, summary_template_id, translation_language, polish_preset_id, settings)
                 VALUES ('project-removed', 'Removed', 'folder', '', 1, 1000, 1000, 'general', 'zh', 'general', '{}')",
                    [],
                )?;
                tx.execute(
                    "INSERT INTO history_items (id, timestamp, duration, title, kind, project_id, status)
                 VALUES ('item-kept', 1000, 1.0, 'Kept Item', 'recording', 'project-kept', 'complete')",
                    [],
                )?;
                tx.execute(
                    "INSERT INTO history_items (id, timestamp, duration, title, kind, project_id, status)
                 VALUES ('item-orphaned', 1001, 1.0, 'Orphaned Item', 'recording', 'project-removed', 'complete')",
                    [],
                )?;
                Ok(())
            })
            .unwrap();

        repo.save_all_values(vec![json!({
            "id": "project-kept",
            "name": "Kept",
            "icon": "folder",
            "createdAt": 1000,
            "updatedAt": 2000,
            "defaults": {}
        })])
        .unwrap();

        repo.get_db()
            .unwrap()
            .with_connection(|conn| {
                let kept_project_id: Option<String> = conn.query_row(
                    "SELECT project_id FROM history_items WHERE id = 'item-kept'",
                    [],
                    |row| row.get(0),
                )?;
                let orphaned_project_id: Option<String> = conn.query_row(
                    "SELECT project_id FROM history_items WHERE id = 'item-orphaned'",
                    [],
                    |row| row.get(0),
                )?;

                assert_eq!(kept_project_id.as_deref(), Some("project-kept"));
                assert_eq!(orphaned_project_id, None);
                Ok(())
            })
            .unwrap();
    }
}
