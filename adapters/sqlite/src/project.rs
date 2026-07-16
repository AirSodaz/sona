use crate::DatabaseError;
use crate::ports::Database as DatabasePort;
use rusqlite::OptionalExtension;
use rusqlite::types::Type;
use serde_json::Value;
use sona_core::ports::time::UnixMillisClock;
use sona_core::project::{
    ActiveProjectSelection, ProjectCreateInput, ProjectDefaults, ProjectIdGenerator,
    ProjectListOptions, ProjectPatch, ProjectRecord, ProjectRepositoryService,
    ProjectRepositorySnapshot, ProjectStore, ProjectStoredState, ProjectUpdateInput,
};
use sona_core::sync::SyncEntityKind;
use sona_core::tag::ACTIVE_TAG_SETTINGS_KEY;
use std::sync::Arc;

use crate::sync_repository::{
    record_local_delete_in_transaction, record_local_field_change_in_transaction, sync_now_ms,
};

#[derive(Clone)]
pub struct SqliteProjectRepository<D = crate::Database>
where
    D: DatabasePort,
{
    db: Arc<D>,
}

crate::impl_db_repository!(SqliteProjectRepository);

pub struct SqliteProjectAdapter<D = crate::Database>
where
    D: DatabasePort,
{
    repository: SqliteProjectRepository<D>,
    ids: Arc<dyn ProjectIdGenerator>,
    clock: Arc<dyn UnixMillisClock>,
}

impl<D> SqliteProjectAdapter<D>
where
    D: DatabasePort,
{
    pub fn new(
        db: Arc<D>,
        ids: Arc<dyn ProjectIdGenerator>,
        clock: Arc<dyn UnixMillisClock>,
    ) -> Self {
        Self {
            repository: SqliteProjectRepository::new(db),
            ids,
            clock,
        }
    }

    pub fn load_state(&self) -> Result<ProjectRepositorySnapshot, String> {
        self.service().load_state()
    }

    pub fn list_projects(&self, options: ProjectListOptions) -> Result<Vec<ProjectRecord>, String> {
        self.service().list_projects(options)
    }

    pub fn replace_projects_json(&self, projects: Vec<Value>) -> Result<(), String> {
        self.service().replace_projects_json(projects)
    }

    pub fn replace_projects(&self, projects: Vec<ProjectRecord>) -> Result<(), String> {
        self.service().replace_projects(projects)
    }

    pub fn create_project(&self, input: ProjectCreateInput) -> Result<ProjectRecord, String> {
        self.service().create_project(input)
    }

    pub fn update_project_json(
        &self,
        project_id: &str,
        updates: Value,
    ) -> Result<Option<ProjectRecord>, String> {
        self.service().update_project_json(project_id, updates)
    }

    pub fn update_project(
        &self,
        project_id: &str,
        updates: ProjectUpdateInput,
    ) -> Result<Option<ProjectRecord>, String> {
        self.service().update_project(project_id, updates)
    }

    pub fn delete_project(&self, project_id: &str) -> Result<(), String> {
        self.service().delete_project(project_id)
    }

    pub fn reorder_projects(&self, project_ids: Vec<String>) -> Result<Vec<ProjectRecord>, String> {
        self.service().reorder_projects(project_ids)
    }

    pub fn get_active_project_selection(&self) -> Result<ActiveProjectSelection, String> {
        self.service().get_active_project_selection()
    }

    pub fn set_active_project_id(&self, project_id: Option<String>) -> Result<(), String> {
        self.service().set_active_project_id(project_id)
    }

    fn service(&self) -> ProjectRepositoryService<'_> {
        ProjectRepositoryService::new(&self.repository, self.ids.as_ref(), self.clock.as_ref())
    }
}

const PROJECT_COLUMNS: [&str; 14] = [
    "id",
    "name",
    "description",
    "icon",
    "color",
    "sort_order",
    "created_at",
    "updated_at",
    "summary_template_id",
    "translation_language",
    "polish_preset_id",
    "polish_scenario",
    "polish_context",
    "export_file_name_prefix",
];

const PROJECT_UPDATE_COLUMNS: [&str; 10] = [
    "name",
    "icon",
    "description",
    "updated_at",
    "summary_template_id",
    "translation_language",
    "polish_preset_id",
    "polish_scenario",
    "polish_context",
    "export_file_name_prefix",
];

const LINK_KIND_TEXT_REPLACEMENT: &str = "text_replacement";
const LINK_KIND_HOTWORD: &str = "hotword";
const LINK_KIND_POLISH_KEYWORD: &str = "polish_keyword";
const LINK_KIND_SPEAKER_PROFILE: &str = "speaker_profile";

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
        "INSERT INTO tags ({}) VALUES ({})",
        project_column_list(&PROJECT_COLUMNS),
        project_named_param_list(&PROJECT_COLUMNS)
    )
}

fn project_upsert_sql() -> String {
    let update_assignments = PROJECT_COLUMNS
        .iter()
        .filter(|&&col| col != "id")
        .map(|col| format!("{col} = excluded.{col}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "INSERT INTO tags ({}) VALUES ({}) ON CONFLICT(id) DO UPDATE SET {}",
        project_column_list(&PROJECT_COLUMNS),
        project_named_param_list(&PROJECT_COLUMNS),
        update_assignments
    )
}

fn project_update_sql() -> String {
    let assignments = PROJECT_UPDATE_COLUMNS
        .iter()
        .map(|column| format!("{column} = :{column}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("UPDATE tags SET {assignments} WHERE id = :id")
}

fn load_projects(conn: &rusqlite::Connection) -> Result<Vec<ProjectRecord>, DatabaseError> {
    let columns = project_select_columns();
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {columns} FROM tags ORDER BY sort_order, id"
    ))?;
    let rows = stmt.query_map([], map_row_to_project)?;
    let mut projects = Vec::new();
    for row in rows {
        let mut project = row?;
        hydrate_tag_default_links(conn, &mut project)?;
        projects.push(project);
    }
    Ok(projects)
}

pub(crate) fn load_projects_in_transaction(
    tx: &rusqlite::Transaction<'_>,
) -> Result<Vec<ProjectRecord>, DatabaseError> {
    load_projects(tx)
}

pub(crate) fn insert_projects_in_transaction(
    tx: &rusqlite::Transaction<'_>,
    projects: &[ProjectRecord],
) -> Result<(), DatabaseError> {
    let sql = project_insert_sql();
    for (sort_order, project) in projects.iter().enumerate() {
        write_project_row(tx, &sql, project, sort_order as i64)?;
        replace_tag_default_links(tx, &project.id, &project.defaults)?;
    }
    Ok(())
}

fn upsert_projects_in_transaction(
    tx: &rusqlite::Transaction<'_>,
    projects: &[ProjectRecord],
) -> Result<(), DatabaseError> {
    let sql = project_upsert_sql();
    for (sort_order, project) in projects.iter().enumerate() {
        write_project_row(tx, &sql, project, sort_order as i64)?;
        replace_tag_default_links(tx, &project.id, &project.defaults)?;
    }
    Ok(())
}

pub(crate) fn delete_projects_in_transaction(
    tx: &rusqlite::Transaction<'_>,
) -> Result<(), DatabaseError> {
    tx.execute("DELETE FROM tag_default_links", [])?;
    tx.execute("DELETE FROM tags", [])?;
    Ok(())
}

pub(crate) fn replace_projects_in_transaction(
    tx: &rusqlite::Transaction<'_>,
    projects: &[ProjectRecord],
) -> Result<(), DatabaseError> {
    tx.execute(
        "CREATE TEMPORARY TABLE keep_projects (id TEXT PRIMARY KEY)",
        [],
    )?;
    {
        let mut keep = tx.prepare_cached("INSERT INTO keep_projects (id) VALUES (?1)")?;
        for project in projects.iter().filter(|project| !project.id.is_empty()) {
            keep.execute([project.id.as_str()])?;
        }
    }
    tx.execute(
        "DELETE FROM tags WHERE id NOT IN (SELECT id FROM keep_projects)",
        [],
    )?;
    tx.execute("DROP TABLE keep_projects", [])?;
    upsert_projects_in_transaction(tx, projects)
}

impl<D> SqliteProjectRepository<D>
where
    D: DatabasePort,
{
    fn list_projects(conn: &rusqlite::Connection) -> Result<Vec<ProjectRecord>, DatabaseError> {
        load_projects(conn)
    }

    fn get_by_id_from_conn(
        conn: &rusqlite::Connection,
        project_id: &str,
    ) -> Result<Option<ProjectRecord>, DatabaseError> {
        let columns = project_select_columns();
        let mut stmt = conn.prepare_cached(&format!("SELECT {columns} FROM tags WHERE id = ?1"))?;
        let mut rows = stmt.query([project_id])?;
        if let Some(row) = rows.next()? {
            let mut project = map_row_to_project(row)?;
            hydrate_tag_default_links(conn, &mut project)?;
            Ok(Some(project))
        } else {
            Ok(None)
        }
    }
}

impl<D> ProjectStore for SqliteProjectRepository<D>
where
    D: DatabasePort,
{
    fn load_state(&self) -> Result<ProjectStoredState, String> {
        self.get_db()
            .and_then(|db| {
                db.with_read_connection(|conn| {
                    let tx = conn
                        .unchecked_transaction()
                        .map_err(DatabaseError::QueryError)?;
                    let projects = Self::list_projects(&tx)?;
                    let active_project_setting_json = tx
                        .query_row(
                            "SELECT value FROM app_settings WHERE key = ?1",
                            [ACTIVE_TAG_SETTINGS_KEY],
                            |row| row.get(0),
                        )
                        .optional()?;
                    tx.commit().map_err(DatabaseError::QueryError)?;
                    Ok(ProjectStoredState {
                        projects,
                        active_project_setting_json,
                    })
                })
            })
            .map_err(|error| error.to_string())
    }

    fn insert_project(&self, project: ProjectRecord) -> Result<ProjectRecord, String> {
        self.get_db()
            .and_then(|db| {
                db.with_transaction(|tx| {
                    write_project_row(tx, &project_insert_sql(), &project, 0)?;
                    replace_tag_default_links(tx, &project.id, &project.defaults)?;
                    record_project_sync_fields(tx, &project, None)?;
                    Ok(())
                })
            })
            .map_err(|error| error.to_string())?;
        Ok(project)
    }

    fn update_project(
        &self,
        project_id: &str,
        patch: ProjectPatch,
        updated_at: u64,
    ) -> Result<Option<ProjectRecord>, String> {
        self.get_db()
            .and_then(|db| {
                db.with_rw_transaction(|tx| {
                    let Some(mut project) = Self::get_by_id_from_conn(tx, project_id)? else {
                        return Ok(None);
                    };
                    let replace_text_links =
                        patch.defaults.enabled_text_replacement_set_ids.is_some();
                    let replace_hotword_links = patch.defaults.enabled_hotword_set_ids.is_some();
                    let replace_keyword_links =
                        patch.defaults.enabled_polish_keyword_set_ids.is_some();
                    let replace_speaker_links =
                        patch.defaults.enabled_speaker_profile_ids.is_some();
                    apply_patch(&mut project, patch);
                    project.updated_at = updated_at;
                    update_project_row(tx, &project)?;
                    if replace_text_links {
                        replace_project_links_for_kind(
                            tx,
                            project_id,
                            LINK_KIND_TEXT_REPLACEMENT,
                            &project.defaults.enabled_text_replacement_set_ids,
                        )?;
                    }
                    if replace_hotword_links {
                        replace_project_links_for_kind(
                            tx,
                            project_id,
                            LINK_KIND_HOTWORD,
                            &project.defaults.enabled_hotword_set_ids,
                        )?;
                    }
                    if replace_keyword_links {
                        replace_project_links_for_kind(
                            tx,
                            project_id,
                            LINK_KIND_POLISH_KEYWORD,
                            &project.defaults.enabled_polish_keyword_set_ids,
                        )?;
                    }
                    if replace_speaker_links {
                        replace_project_links_for_kind(
                            tx,
                            project_id,
                            LINK_KIND_SPEAKER_PROFILE,
                            &project.defaults.enabled_speaker_profile_ids,
                        )?;
                    }
                    record_project_sync_fields(tx, &project, None)?;
                    Ok(Some(project))
                })
            })
            .map_err(|error| error.to_string())
    }

    fn delete_project(&self, project_id: &str) -> Result<(), String> {
        self.get_db()
            .and_then(|db| {
                db.with_transaction(|tx| {
                    tx.execute("DELETE FROM tags WHERE id = ?1", [project_id])?;
                    record_local_delete_in_transaction(
                        tx,
                        SyncEntityKind::Tag,
                        project_id,
                        sync_now_ms(),
                    )?;
                    Ok(())
                })
            })
            .map_err(|error| error.to_string())
    }

    fn replace_projects(&self, projects: Vec<ProjectRecord>) -> Result<(), String> {
        self.get_db()
            .and_then(|db| {
                db.with_transaction(|tx| {
                    let existing_ids = load_projects(tx)?
                        .into_iter()
                        .map(|project| project.id)
                        .collect::<Vec<_>>();
                    replace_projects_in_transaction(tx, &projects)?;
                    let now_ms = sync_now_ms();
                    for existing_id in existing_ids.iter().filter(|existing_id| {
                        !projects.iter().any(|project| project.id == **existing_id)
                    }) {
                        record_local_delete_in_transaction(
                            tx,
                            SyncEntityKind::Tag,
                            existing_id,
                            now_ms,
                        )?;
                    }
                    for (sort_order, project) in projects.iter().enumerate() {
                        record_project_sync_fields(tx, project, Some(sort_order))?;
                    }
                    Ok(())
                })
            })
            .map_err(|error| error.to_string())
    }

    fn reorder_projects(&self, project_ids: Vec<String>) -> Result<Vec<ProjectRecord>, String> {
        self.get_db()
            .and_then(|db| {
                db.with_rw_transaction(|tx| {
                    let mut stmt =
                        tx.prepare_cached("UPDATE tags SET sort_order = ?1 WHERE id = ?2")?;
                    for (sort_order, id) in project_ids.iter().enumerate() {
                        stmt.execute(rusqlite::params![sort_order as i64, id])?;
                    }
                    drop(stmt);
                    let projects = Self::list_projects(tx)?;
                    let now_ms = sync_now_ms();
                    for (sort_order, project) in projects.iter().enumerate() {
                        record_local_field_change_in_transaction(
                            tx,
                            SyncEntityKind::Tag,
                            &project.id,
                            "sortOrder",
                            serde_json::json!(sort_order),
                            now_ms,
                        )?;
                    }
                    Ok(projects)
                })
            })
            .map_err(|error| error.to_string())
    }

    fn set_active_project_setting_json(&self, setting_json: String) -> Result<(), String> {
        self.get_db()
            .and_then(|db| {
                db.with_transaction(|tx| {
                    tx.execute(
                        "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
                         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                        rusqlite::params![ACTIVE_TAG_SETTINGS_KEY, setting_json],
                    )?;
                    Ok(())
                })
            })
            .map_err(|error| error.to_string())
    }
}

pub(crate) fn record_project_sync_fields(
    tx: &rusqlite::Transaction<'_>,
    project: &ProjectRecord,
    sort_order: Option<usize>,
) -> Result<(), DatabaseError> {
    let now_ms = project.updated_at.max(project.created_at);
    let mut fields = vec![
        ("name", serde_json::json!(project.name)),
        ("description", serde_json::json!(project.description)),
        ("icon", serde_json::json!(project.icon)),
        ("createdAt", serde_json::json!(project.created_at)),
        ("updatedAt", serde_json::json!(project.updated_at)),
        (
            "summaryTemplateId",
            serde_json::json!(project.defaults.summary_template_id),
        ),
        (
            "translationLanguage",
            serde_json::json!(project.defaults.translation_language),
        ),
        (
            "polishPresetId",
            serde_json::json!(project.defaults.polish_preset_id),
        ),
        (
            "polishScenario",
            serde_json::json!(project.defaults.polish_scenario),
        ),
        (
            "polishContext",
            serde_json::json!(project.defaults.polish_context),
        ),
        (
            "exportFileNamePrefix",
            serde_json::json!(project.defaults.export_file_name_prefix),
        ),
        (
            "enabledTextReplacementSetIds",
            serde_json::json!(project.defaults.enabled_text_replacement_set_ids),
        ),
        (
            "enabledHotwordSetIds",
            serde_json::json!(project.defaults.enabled_hotword_set_ids),
        ),
        (
            "enabledPolishKeywordSetIds",
            serde_json::json!(project.defaults.enabled_polish_keyword_set_ids),
        ),
        (
            "enabledSpeakerProfileIds",
            serde_json::json!(project.defaults.enabled_speaker_profile_ids),
        ),
    ];
    if let Some(sort_order) = sort_order {
        fields.push(("sortOrder", serde_json::json!(sort_order)));
    }
    for (field, value) in fields {
        record_local_field_change_in_transaction(
            tx,
            SyncEntityKind::Tag,
            &project.id,
            field,
            value,
            now_ms,
        )?;
    }
    Ok(())
}

fn write_project_row(
    conn: &rusqlite::Connection,
    sql: &str,
    project: &ProjectRecord,
    sort_order: i64,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        sql,
        rusqlite::named_params! {
            ":id": &project.id,
            ":name": &project.name,
            ":description": &project.description,
            ":icon": &project.icon,
            ":color": "",
            ":sort_order": sort_order,
            ":created_at": project.created_at as i64,
            ":updated_at": project.updated_at as i64,
            ":summary_template_id": &project.defaults.summary_template_id,
            ":translation_language": &project.defaults.translation_language,
            ":polish_preset_id": &project.defaults.polish_preset_id,
            ":polish_scenario": &project.defaults.polish_scenario,
            ":polish_context": &project.defaults.polish_context,
            ":export_file_name_prefix": &project.defaults.export_file_name_prefix,
        },
    )?;
    Ok(())
}

fn update_project_row(
    conn: &rusqlite::Connection,
    project: &ProjectRecord,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        &project_update_sql(),
        rusqlite::named_params! {
            ":name": &project.name,
            ":icon": &project.icon,
            ":description": &project.description,
            ":updated_at": project.updated_at as i64,
            ":summary_template_id": &project.defaults.summary_template_id,
            ":translation_language": &project.defaults.translation_language,
            ":polish_preset_id": &project.defaults.polish_preset_id,
            ":polish_scenario": &project.defaults.polish_scenario,
            ":polish_context": &project.defaults.polish_context,
            ":export_file_name_prefix": &project.defaults.export_file_name_prefix,
            ":id": &project.id,
        },
    )?;
    Ok(())
}

fn apply_patch(project: &mut ProjectRecord, patch: ProjectPatch) {
    if let Some(value) = patch.name {
        project.name = value;
    }
    if let Some(value) = patch.icon {
        project.icon = value;
    }
    if let Some(value) = patch.description {
        project.description = value;
    }
    if let Some(value) = patch.defaults.summary_template_id {
        project.defaults.summary_template_id = value;
    }
    if let Some(value) = patch.defaults.translation_language {
        project.defaults.translation_language = value;
    }
    if let Some(value) = patch.defaults.polish_preset_id {
        project.defaults.polish_preset_id = value;
    }
    if let Some(value) = patch.defaults.polish_scenario {
        project.defaults.polish_scenario = Some(value);
    }
    if let Some(value) = patch.defaults.polish_context {
        project.defaults.polish_context = Some(value);
    }
    if let Some(value) = patch.defaults.export_file_name_prefix {
        project.defaults.export_file_name_prefix = value;
    }
    if let Some(value) = patch.defaults.enabled_text_replacement_set_ids {
        project.defaults.enabled_text_replacement_set_ids = value;
    }
    if let Some(value) = patch.defaults.enabled_hotword_set_ids {
        project.defaults.enabled_hotword_set_ids = value;
    }
    if let Some(value) = patch.defaults.enabled_polish_keyword_set_ids {
        project.defaults.enabled_polish_keyword_set_ids = value;
    }
    if let Some(value) = patch.defaults.enabled_speaker_profile_ids {
        project.defaults.enabled_speaker_profile_ids = value;
    }
}

fn map_row_to_project(row: &rusqlite::Row) -> rusqlite::Result<ProjectRecord> {
    let id: String = row.get("id")?;
    let name: String = row.get("name")?;
    let description: String = row.get("description")?;
    let icon: String = row.get("icon")?;
    let _color: String = row.get("color")?;
    let _sort_order: i64 = row.get("sort_order")?;
    let created_at: i64 = row.get("created_at")?;
    let updated_at: i64 = row.get("updated_at")?;
    let summary_template_id: String = row.get("summary_template_id")?;
    let translation_language: String = row.get("translation_language")?;
    let polish_preset_id: String = row.get("polish_preset_id")?;
    let polish_scenario: Option<String> = row.get("polish_scenario")?;
    let polish_context: Option<String> = row.get("polish_context")?;
    let export_file_name_prefix: String = row.get("export_file_name_prefix")?;

    let defaults = ProjectDefaults {
        summary_template_id,
        translation_language,
        polish_preset_id,
        polish_scenario,
        polish_context,
        export_file_name_prefix,
        enabled_text_replacement_set_ids: Vec::new(),
        enabled_hotword_set_ids: Vec::new(),
        enabled_polish_keyword_set_ids: Vec::new(),
        enabled_speaker_profile_ids: Vec::new(),
    };

    Ok(ProjectRecord {
        id,
        name,
        description,
        icon,
        created_at: checked_u64_column(row, "created_at", created_at)?,
        updated_at: checked_u64_column(row, "updated_at", updated_at)?,
        defaults,
    })
}

fn checked_u64_column(row: &rusqlite::Row<'_>, column: &str, value: i64) -> rusqlite::Result<u64> {
    let column_index = row.as_ref().column_index(column)?;
    u64::try_from(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(column_index, Type::Integer, Box::new(error))
    })
}

fn replace_tag_default_links(
    tx: &rusqlite::Transaction,
    project_id: &str,
    defaults: &ProjectDefaults,
) -> Result<(), rusqlite::Error> {
    tx.execute(
        "DELETE FROM tag_default_links WHERE tag_id = ?1",
        [project_id],
    )?;
    insert_project_links_for_kind(
        tx,
        project_id,
        LINK_KIND_TEXT_REPLACEMENT,
        &defaults.enabled_text_replacement_set_ids,
    )?;
    insert_project_links_for_kind(
        tx,
        project_id,
        LINK_KIND_HOTWORD,
        &defaults.enabled_hotword_set_ids,
    )?;
    insert_project_links_for_kind(
        tx,
        project_id,
        LINK_KIND_POLISH_KEYWORD,
        &defaults.enabled_polish_keyword_set_ids,
    )?;
    insert_project_links_for_kind(
        tx,
        project_id,
        LINK_KIND_SPEAKER_PROFILE,
        &defaults.enabled_speaker_profile_ids,
    )
}

fn replace_project_links_for_kind(
    tx: &rusqlite::Transaction,
    project_id: &str,
    kind: &str,
    target_ids: &[String],
) -> Result<(), rusqlite::Error> {
    tx.execute(
        "DELETE FROM tag_default_links WHERE tag_id = ?1 AND kind = ?2",
        rusqlite::params![project_id, kind],
    )?;
    insert_project_links_for_kind(tx, project_id, kind, target_ids)
}

fn insert_project_links_for_kind(
    tx: &rusqlite::Transaction,
    project_id: &str,
    kind: &str,
    target_ids: &[String],
) -> Result<(), rusqlite::Error> {
    let mut stmt = tx.prepare_cached(
        "INSERT OR REPLACE INTO tag_default_links (tag_id, kind, target_id, sort_order)
         VALUES (?1, ?2, ?3, ?4)",
    )?;
    for (sort_order, target_id) in target_ids.iter().enumerate() {
        stmt.execute(rusqlite::params![
            project_id,
            kind,
            target_id,
            sort_order as i64
        ])?;
    }
    Ok(())
}

fn hydrate_tag_default_links(
    conn: &rusqlite::Connection,
    project: &mut ProjectRecord,
) -> Result<(), DatabaseError> {
    let mut stmt = conn.prepare_cached(
        "SELECT kind, target_id FROM tag_default_links
         WHERE tag_id = ?1
         ORDER BY kind, sort_order, target_id",
    )?;
    let mut rows = stmt.query([project.id.as_str()])?;
    while let Some(row) = rows.next()? {
        let kind: String = row.get(0)?;
        let target_id: String = row.get(1)?;
        match kind.as_str() {
            LINK_KIND_TEXT_REPLACEMENT => project
                .defaults
                .enabled_text_replacement_set_ids
                .push(target_id),
            LINK_KIND_HOTWORD => project.defaults.enabled_hotword_set_ids.push(target_id),
            LINK_KIND_POLISH_KEYWORD => project
                .defaults
                .enabled_polish_keyword_set_ids
                .push(target_id),
            LINK_KIND_SPEAKER_PROFILE => {
                project.defaults.enabled_speaker_profile_ids.push(target_id)
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Database;
    use sona_core::project::{ProjectDefaultsInput, ProjectDefaultsPatch};
    use std::path::PathBuf;

    fn record(id: &str, name: &str, timestamp: u64) -> ProjectRecord {
        ProjectRecord {
            id: id.to_string(),
            name: name.to_string(),
            description: format!("{name} description"),
            icon: "folder".to_string(),
            created_at: timestamp,
            updated_at: timestamp,
            defaults: ProjectDefaults {
                summary_template_id: "summary".to_string(),
                translation_language: "ja".to_string(),
                polish_preset_id: "meeting".to_string(),
                polish_scenario: Some("scenario".to_string()),
                polish_context: Some("context".to_string()),
                export_file_name_prefix: "prefix-".to_string(),
                enabled_text_replacement_set_ids: vec!["text-b".to_string(), "text-a".to_string()],
                enabled_hotword_set_ids: vec!["hotword-a".to_string()],
                enabled_polish_keyword_set_ids: vec!["keyword-a".to_string()],
                enabled_speaker_profile_ids: vec!["speaker-a".to_string()],
            },
        }
    }

    fn repository() -> SqliteProjectRepository {
        SqliteProjectRepository::with_db(PathBuf::new(), Database::open_in_memory().unwrap())
    }

    fn raw_sort_orders(repo: &SqliteProjectRepository) -> Vec<(String, i64)> {
        repo.get_db()
            .unwrap()
            .with_connection(|conn| {
                let mut stmt = conn.prepare("SELECT id, sort_order FROM tags ORDER BY id")?;
                stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(DatabaseError::QueryError)
            })
            .unwrap()
    }

    #[test]
    fn typed_state_loads_projects_links_and_raw_active_setting() {
        let repo = repository();
        ProjectStore::insert_project(&repo, record("project-a", "Alpha", 100)).unwrap();
        ProjectStore::set_active_project_setting_json(&repo, " { \"id\" : 7 } ".to_string())
            .unwrap();

        let state = ProjectStore::load_state(&repo).unwrap();

        assert_eq!(state.projects, vec![record("project-a", "Alpha", 100)]);
        assert_eq!(
            state.active_project_setting_json.as_deref(),
            Some(" { \"id\" : 7 } ")
        );
    }

    #[test]
    fn load_state_works_through_read_only_database() {
        let dir = tempfile::tempdir().unwrap();
        {
            let repo = SqliteProjectRepository::with_db(
                dir.path().to_path_buf(),
                Database::open(dir.path()).unwrap(),
            );
            ProjectStore::insert_project(&repo, record("project-a", "Alpha", 100)).unwrap();
            ProjectStore::set_active_project_setting_json(&repo, "\"project-a\"".to_string())
                .unwrap();
        }

        let repo = SqliteProjectRepository::with_db(
            dir.path().to_path_buf(),
            Database::open_read_only(dir.path()).unwrap(),
        );
        let state = ProjectStore::load_state(&repo).unwrap();

        assert_eq!(state.projects, vec![record("project-a", "Alpha", 100)]);
        assert_eq!(
            state.active_project_setting_json.as_deref(),
            Some("\"project-a\"")
        );
    }

    #[test]
    fn insert_writes_exact_row_defaults_and_ordered_link_kinds() {
        let repo = repository();
        let expected = record("project-a", "Alpha", 100);

        assert_eq!(
            ProjectStore::insert_project(&repo, expected.clone()).unwrap(),
            expected
        );
        repo.get_db().unwrap().with_connection(|conn| {
            let row: (String, String, i64, i64) = conn.query_row(
                "SELECT color, summary_template_id, created_at, updated_at FROM tags WHERE id = 'project-a'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )?;
            assert_eq!(row, ("".to_string(), "summary".to_string(), 100, 100));
            let mut stmt = conn.prepare(
                "SELECT kind, target_id, sort_order FROM tag_default_links WHERE tag_id = 'project-a' ORDER BY kind, sort_order",
            )?;
            let links = stmt
                .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?)))?
                .collect::<Result<Vec<_>, _>>()?;
            assert_eq!(links, vec![
                ("hotword".to_string(), "hotword-a".to_string(), 0),
                ("polish_keyword".to_string(), "keyword-a".to_string(), 0),
                ("speaker_profile".to_string(), "speaker-a".to_string(), 0),
                ("text_replacement".to_string(), "text-b".to_string(), 0),
                ("text_replacement".to_string(), "text-a".to_string(), 1),
            ]);
            Ok(())
        }).unwrap();
    }

    #[test]
    fn typed_patch_only_overlays_supplied_fields_and_arrays() {
        let repo = repository();
        let original = record("project-a", "Alpha", 100);
        ProjectStore::insert_project(&repo, original.clone()).unwrap();

        let updated = ProjectStore::update_project(
            &repo,
            "project-a",
            ProjectPatch {
                name: Some("Renamed".to_string()),
                defaults: ProjectDefaultsPatch {
                    polish_context: Some("new context".to_string()),
                    enabled_hotword_set_ids: Some(Vec::new()),
                    enabled_text_replacement_set_ids: Some(vec!["text-new".to_string()]),
                    ..Default::default()
                },
                ..Default::default()
            },
            200,
        )
        .unwrap()
        .unwrap();

        assert_eq!(updated.name, "Renamed");
        assert_eq!(updated.description, original.description);
        assert_eq!(updated.updated_at, 200);
        assert_eq!(
            updated.defaults.polish_context.as_deref(),
            Some("new context")
        );
        assert!(updated.defaults.enabled_hotword_set_ids.is_empty());
        assert_eq!(
            updated.defaults.enabled_text_replacement_set_ids,
            vec!["text-new"]
        );
        assert_eq!(
            updated.defaults.enabled_polish_keyword_set_ids,
            vec!["keyword-a"]
        );
        assert_eq!(
            updated.defaults.enabled_speaker_profile_ids,
            vec!["speaker-a"]
        );
    }

    #[test]
    fn typed_patch_does_not_rewrite_unsupplied_link_arrays() {
        let repo = repository();
        ProjectStore::insert_project(&repo, record("project-a", "Alpha", 100)).unwrap();
        repo.get_db()
            .unwrap()
            .with_transaction(|tx| {
                tx.execute_batch(
                    "CREATE TRIGGER reject_existing_hotword BEFORE INSERT ON tag_default_links
                     WHEN NEW.target_id = 'hotword-a'
                     BEGIN SELECT RAISE(ABORT, 'untouched link was rewritten'); END;",
                )?;
                Ok(())
            })
            .unwrap();

        let updated = ProjectStore::update_project(
            &repo,
            "project-a",
            ProjectPatch {
                name: Some("Renamed".to_string()),
                ..Default::default()
            },
            200,
        )
        .unwrap()
        .unwrap();

        assert_eq!(updated.name, "Renamed");
        assert_eq!(updated.defaults.enabled_hotword_set_ids, vec!["hotword-a"]);
    }

    #[test]
    fn replace_all_preserves_order_and_history_fk_behavior() {
        let repo = repository();
        ProjectStore::replace_projects(
            &repo,
            vec![
                record("kept", "Kept", 100),
                record("removed", "Removed", 100),
            ],
        )
        .unwrap();
        repo.get_db().unwrap().with_transaction(|tx| {
            tx.execute("INSERT INTO history_items (id, timestamp, duration, title, kind, status) VALUES ('kept-item', 1, 1.0, 'Kept', 'recording', 'complete')", [])?;
            tx.execute("INSERT INTO history_items (id, timestamp, duration, title, kind, status) VALUES ('removed-item', 2, 1.0, 'Removed', 'recording', 'complete')", [])?;
            tx.execute("INSERT INTO history_item_tags (history_id, tag_id) VALUES ('kept-item', 'kept')", [])?;
            tx.execute("INSERT INTO history_item_tags (history_id, tag_id) VALUES ('removed-item', 'removed')", [])?;
            Ok(())
        }).unwrap();

        ProjectStore::replace_projects(
            &repo,
            vec![
                record("new", "New", 300),
                record("kept", "Kept updated", 200),
            ],
        )
        .unwrap();

        let state = ProjectStore::load_state(&repo).unwrap();
        assert_eq!(
            state
                .projects
                .iter()
                .map(|p| p.id.as_str())
                .collect::<Vec<_>>(),
            vec!["new", "kept"]
        );
        repo.get_db()
            .unwrap()
            .with_connection(|conn| {
                let kept: Option<String> = conn.query_row(
                    "SELECT tag_id FROM history_item_tags WHERE history_id = 'kept-item'",
                    [],
                    |row| row.get(0),
                )?;
                let removed: Option<String> = conn
                    .query_row(
                        "SELECT tag_id FROM history_item_tags WHERE history_id = 'removed-item'",
                        [],
                        |row| row.get(0),
                    )
                    .optional()?;
                assert_eq!(kept.as_deref(), Some("kept"));
                assert_eq!(removed, None);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn replace_all_rolls_back_rows_links_and_history_on_link_failure() {
        let repo = repository();
        ProjectStore::replace_projects(
            &repo,
            vec![
                record("kept", "Before", 100),
                record("removed", "Removed", 100),
            ],
        )
        .unwrap();
        repo.get_db().unwrap().with_transaction(|tx| {
            tx.execute("INSERT INTO history_items (id, timestamp, duration, title, kind, status) VALUES ('item', 1, 1.0, 'Item', 'recording', 'complete')", [])?;
            tx.execute("INSERT INTO history_item_tags (history_id, tag_id) VALUES ('item', 'removed')", [])?;
            tx.execute_batch("CREATE TRIGGER reject_project_link BEFORE INSERT ON tag_default_links WHEN NEW.target_id = 'reject' BEGIN SELECT RAISE(ABORT, 'rejected link'); END;")?;
            Ok(())
        }).unwrap();
        let before = ProjectStore::load_state(&repo).unwrap();
        let mut bad = record("kept", "After", 200);
        bad.defaults.enabled_hotword_set_ids = vec!["reject".to_string()];

        assert!(ProjectStore::replace_projects(&repo, vec![bad]).is_err());
        assert_eq!(ProjectStore::load_state(&repo).unwrap(), before);
        repo.get_db()
            .unwrap()
            .with_connection(|conn| {
                let assignment: Option<String> = conn.query_row(
                    "SELECT tag_id FROM history_item_tags WHERE history_id = 'item'",
                    [],
                    |row| row.get(0),
                )?;
                assert_eq!(assignment.as_deref(), Some("removed"));
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn replace_all_accepts_single_empty_missing_and_non_string_ids() {
        for project in [
            serde_json::json!({"id": "", "name": "Empty"}),
            serde_json::json!({"name": "Missing"}),
            serde_json::json!({"id": 7, "name": "Non-string"}),
        ] {
            let repo = repository();
            let service = ProjectRepositoryService::new(&repo, &FixedId, &FixedClock);

            service
                .replace_projects_json(vec![project.clone()])
                .unwrap();

            let projects = service
                .list_projects(ProjectListOptions::default())
                .unwrap();
            assert_eq!(projects.len(), 1, "project {project}");
            assert_eq!(projects[0].id, "", "project {project}");
            assert_eq!(
                projects[0].name,
                project
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .unwrap()
            );
        }
    }

    #[test]
    fn replace_all_accepts_duplicate_normalized_empty_ids_and_last_upsert_wins() {
        let repo = repository();
        let service = ProjectRepositoryService::new(&repo, &FixedId, &FixedClock);

        service
            .replace_projects_json(vec![
                serde_json::json!({"id": "", "name": "Empty"}),
                serde_json::json!({"name": "Missing"}),
                serde_json::json!({"id": 7, "name": "Non-string last"}),
            ])
            .unwrap();

        let projects = service
            .list_projects(ProjectListOptions::default())
            .unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].id, "");
        assert_eq!(projects[0].name, "Non-string last");
    }

    #[test]
    fn replace_all_recreates_empty_id_and_nulls_existing_history_assignment() {
        let repo = repository();
        ProjectStore::insert_project(&repo, record("", "Before", 100)).unwrap();
        repo.get_db()
            .unwrap()
            .with_transaction(|tx| {
                tx.execute(
                    "INSERT INTO history_items (id, timestamp, duration, title, kind, status) VALUES ('empty-item', 1, 1.0, 'Empty', 'recording', 'complete')",
                    [],
                )?;
                tx.execute("INSERT INTO history_item_tags (history_id, tag_id) VALUES ('empty-item', '')", [])?;
                Ok(())
            })
            .unwrap();
        let service = ProjectRepositoryService::new(&repo, &FixedId, &FixedClock);

        service
            .replace_projects_json(vec![serde_json::json!({"name": "After"})])
            .unwrap();

        repo.get_db()
            .unwrap()
            .with_connection(|conn| {
                let assignment: Option<String> = conn
                    .query_row(
                        "SELECT tag_id FROM history_item_tags WHERE history_id = 'empty-item'",
                        [],
                        |row| row.get(0),
                    )
                    .optional()?;
                let name: String =
                    conn.query_row("SELECT name FROM tags WHERE id = ''", [], |row| row.get(0))?;
                assert_eq!(assignment, None);
                assert_eq!(name, "After");
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn replace_all_preserves_database_error_for_duplicate_non_empty_ids() {
        let repo = repository();

        let error = ProjectStore::replace_projects(
            &repo,
            vec![
                record("duplicate", "First", 1),
                record("duplicate", "Last", 2),
            ],
        )
        .unwrap_err();

        assert!(error.contains("UNIQUE constraint failed"), "{error}");
        assert!(ProjectStore::load_state(&repo).unwrap().projects.is_empty());
    }

    #[test]
    fn delete_nulls_history_and_preserves_active_setting() {
        let repo = repository();
        ProjectStore::insert_project(&repo, record("project-a", "Alpha", 100)).unwrap();
        ProjectStore::set_active_project_setting_json(&repo, "\"project-a\"".to_string()).unwrap();
        repo.get_db().unwrap().with_transaction(|tx| {
            tx.execute("INSERT INTO history_items (id, timestamp, duration, title, kind, status) VALUES ('item', 1, 1.0, 'Item', 'recording', 'complete')", [])?;
            tx.execute("INSERT INTO history_item_tags (history_id, tag_id) VALUES ('item', 'project-a')", [])?;
            Ok(())
        }).unwrap();

        ProjectStore::delete_project(&repo, "project-a").unwrap();

        let state = ProjectStore::load_state(&repo).unwrap();
        assert!(state.projects.is_empty());
        assert_eq!(
            state.active_project_setting_json.as_deref(),
            Some("\"project-a\"")
        );
        repo.get_db()
            .unwrap()
            .with_connection(|conn| {
                let assignment: Option<String> = conn
                    .query_row(
                        "SELECT tag_id FROM history_item_tags WHERE history_id = 'item'",
                        [],
                        |row| row.get(0),
                    )
                    .optional()?;
                assert_eq!(assignment, None);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn reorder_updates_only_supplied_ids_and_returns_complete_transaction_state() {
        let repo = repository();
        ProjectStore::replace_projects(
            &repo,
            vec![
                record("a", "A", 1),
                record("b", "B", 1),
                record("c", "C", 1),
            ],
        )
        .unwrap();

        let projects = ProjectStore::reorder_projects(&repo, vec!["c".to_string()]).unwrap();

        let mut ids = projects
            .iter()
            .map(|project| project.id.as_str())
            .collect::<Vec<_>>();
        ids.sort_unstable();
        assert_eq!(ids, vec!["a", "b", "c"]);
        assert_eq!(
            raw_sort_orders(&repo),
            vec![
                ("a".to_string(), 0),
                ("b".to_string(), 1),
                ("c".to_string(), 0)
            ]
        );
    }

    #[test]
    fn reorder_unknown_id_leaves_all_sort_orders_unchanged() {
        let repo = repository();
        ProjectStore::replace_projects(
            &repo,
            vec![
                record("a", "A", 1),
                record("b", "B", 1),
                record("c", "C", 1),
            ],
        )
        .unwrap();

        let projects = ProjectStore::reorder_projects(&repo, vec!["unknown".to_string()]).unwrap();

        assert_eq!(projects.len(), 3);
        assert_eq!(
            raw_sort_orders(&repo),
            vec![
                ("a".to_string(), 0),
                ("b".to_string(), 1),
                ("c".to_string(), 2)
            ]
        );
    }

    #[test]
    fn reorder_duplicate_id_applies_each_supplied_position_only() {
        let repo = repository();
        ProjectStore::replace_projects(
            &repo,
            vec![
                record("a", "A", 1),
                record("b", "B", 1),
                record("c", "C", 1),
            ],
        )
        .unwrap();

        let projects =
            ProjectStore::reorder_projects(&repo, vec!["c".to_string(), "c".to_string()]).unwrap();

        assert_eq!(projects.len(), 3);
        assert_eq!(
            raw_sort_orders(&repo),
            vec![
                ("a".to_string(), 0),
                ("b".to_string(), 1),
                ("c".to_string(), 1)
            ]
        );
    }

    struct FixedId;
    impl ProjectIdGenerator for FixedId {
        fn generate_id(&self) -> String {
            "fixed-id".to_string()
        }
    }
    struct FixedClock;
    impl UnixMillisClock for FixedClock {
        fn now_ms(&self) -> Result<u64, String> {
            Ok(777)
        }
    }

    #[test]
    fn project_adapter_composes_repository_ids_and_clock() {
        let adapter = SqliteProjectAdapter::new(
            Arc::new(Database::open_in_memory().unwrap()),
            Arc::new(FixedId),
            Arc::new(FixedClock),
        );

        let created = adapter
            .create_project(ProjectCreateInput {
                name: "Adapter project".to_string(),
                description: None,
                icon: None,
                defaults: ProjectDefaultsInput::default(),
            })
            .unwrap();

        assert_eq!(created.id, "fixed-id");
        assert_eq!(created.created_at, 777);
        assert_eq!(created.updated_at, 777);
        assert_eq!(adapter.load_state().unwrap().projects, vec![created]);

        let updated = adapter
            .update_project(
                "fixed-id",
                ProjectUpdateInput {
                    name: Some("Updated".to_string()),
                    ..Default::default()
                },
            )
            .unwrap()
            .unwrap();
        assert_eq!(updated.name, "Updated");
        assert_eq!(updated.updated_at, 777);

        let replacement = record("replacement", "Replacement", 900);
        adapter.replace_projects(vec![replacement.clone()]).unwrap();
        assert_eq!(adapter.load_state().unwrap().projects, vec![replacement]);
    }

    #[test]
    fn service_over_sqlite_preserves_create_update_and_replacement_defaults() {
        let repo = repository();
        let service = ProjectRepositoryService::new(&repo, &FixedId, &FixedClock);
        let created = service
            .create_project(ProjectCreateInput {
                name: "Created".to_string(),
                description: None,
                icon: None,
                defaults: ProjectDefaultsInput::default(),
            })
            .unwrap();
        assert_eq!(created.id, "fixed-id");
        assert_eq!(created.created_at, 777);
        assert_eq!(created.defaults.summary_template_id, "general");
        assert_eq!(created.defaults.translation_language, "zh");
        assert_eq!(created.defaults.polish_preset_id, "general");

        let updated = service.update_project_json("fixed-id", serde_json::json!({"name": "Updated", "defaults": {"enabledHotwordSetIds": ["hot"]}})).unwrap().unwrap();
        assert_eq!(updated.updated_at, 777);
        assert_eq!(updated.defaults.summary_template_id, "general");
        assert_eq!(updated.defaults.enabled_hotword_set_ids, vec!["hot"]);

        service.replace_projects_json(vec![serde_json::json!({"id": "replacement", "name": "Replacement", "createdAt": 5, "updatedAt": 6, "defaults": {}})]).unwrap();
        let replacement = service
            .list_projects(ProjectListOptions::default())
            .unwrap()
            .pop()
            .unwrap();
        assert_eq!(replacement.defaults.summary_template_id, "general");
        assert_eq!(replacement.defaults.translation_language, "zh");
        assert_eq!(replacement.defaults.polish_preset_id, "general");
    }
}
