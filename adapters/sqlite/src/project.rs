use crate::DatabaseError;
use crate::ports::Database as DatabasePort;
use rusqlite::OptionalExtension;
use rusqlite::types::Type;
use serde_json::Value;
use sona_application::tag::TagRepositoryService;
use sona_core::dashboard::error::DashboardError;
use sona_core::dashboard::ports::TagRepository;
use sona_core::ports::time::ClockPort;
use sona_core::project::ports::ProjectRepository;
use sona_core::project::{
    ProjectCreateInput, ProjectError, ProjectPipelineConfig, ProjectRecord, ProjectUpdateInput,
};
use sona_core::sync::SyncEntityKind;
use sona_core::tag::{
    ActiveTagSelection, TagCreateInput, TagError, TagIdGenerator, TagListOptions, TagPatch,
    TagRecord, TagRepositorySnapshot, TagStore, TagStoredState, TagUpdateInput,
};
use std::sync::Arc;

use crate::legacy_change_time;
use crate::sync_repository::{
    record_local_delete_in_transaction, record_local_field_change_in_transaction,
};

pub const ACTIVE_PROJECT_SETTINGS_KEY: &str = "sona-active-project-id";

#[derive(Clone)]
pub struct SqliteProjectRepository<D = crate::Database>
where
    D: DatabasePort,
{
    db: Arc<D>,
}

crate::impl_db_repository!(SqliteProjectRepository);

pub type SqliteTagRepository<D = crate::Database> = SqliteProjectRepository<D>;

pub struct SqliteProjectAdapter<D = crate::Database>
where
    D: DatabasePort,
{
    repository: SqliteProjectRepository<D>,
    ids: Arc<dyn TagIdGenerator>,
    clock: Arc<dyn ClockPort>,
}

pub type SqliteTagAdapter<D = crate::Database> = SqliteProjectAdapter<D>;

impl<D> SqliteProjectAdapter<D>
where
    D: DatabasePort,
{
    pub fn new(db: Arc<D>, ids: Arc<dyn TagIdGenerator>, clock: Arc<dyn ClockPort>) -> Self {
        Self {
            repository: SqliteProjectRepository::new(db),
            ids,
            clock,
        }
    }

    pub fn load_state(&self) -> Result<TagRepositorySnapshot, TagError> {
        self.service().load_state()
    }

    pub fn list_tags(&self, options: TagListOptions) -> Result<Vec<TagRecord>, TagError> {
        self.service().list_tags(options)
    }

    pub fn replace_tags_json(&self, tags: Vec<Value>) -> Result<(), TagError> {
        self.service().replace_tags_json(tags)
    }

    pub fn replace_tags(&self, tags: Vec<TagRecord>) -> Result<(), TagError> {
        self.service().replace_tags(tags)
    }

    pub fn create_tag(&self, input: TagCreateInput) -> Result<TagRecord, TagError> {
        self.service().create_tag(input)
    }

    pub fn update_tag_json(
        &self,
        tag_id: &str,
        updates: Value,
    ) -> Result<Option<TagRecord>, TagError> {
        self.service().update_tag_json(tag_id, updates)
    }

    pub fn update_tag(
        &self,
        tag_id: &str,
        updates: TagUpdateInput,
    ) -> Result<Option<TagRecord>, TagError> {
        self.service().update_tag(tag_id, updates)
    }

    pub fn delete_tag(&self, tag_id: &str) -> Result<(), TagError> {
        self.service().delete_tag(tag_id)
    }

    pub fn delete_project_with_cascade(
        &self,
        project_id: &str,
        cascade_action: &str,
    ) -> Result<(), TagError> {
        self.repository
            .get_db()
            .and_then(|db| {
                db.with_transaction(|tx| {
                    if cascade_action == "deleteItems" {
                        tx.execute(
                            "UPDATE history_items SET project_id = NULL, deleted_at = COALESCE(deleted_at, ?1) WHERE project_id = ?2",
                            rusqlite::params![legacy_change_time::now_ms() as i64, project_id],
                        )?;
                    } else {
                        tx.execute(
                            "UPDATE history_items SET project_id = NULL WHERE project_id = ?1",
                            [project_id],
                        )?;
                    }
                    tx.execute(
                        "DELETE FROM project_pipelines WHERE project_id = ?1",
                        [project_id],
                    )?;
                    tx.execute(
                        "DELETE FROM app_settings WHERE key = ?1 AND (value = ?2 OR json_extract(value, '$') = ?2)",
                        rusqlite::params![ACTIVE_PROJECT_SETTINGS_KEY, project_id],
                    )?;
                    tx.execute("DELETE FROM projects WHERE id = ?1", [project_id])?;
                    record_local_delete_in_transaction(
                        tx,
                        SyncEntityKind::Project,
                        project_id,
                        legacy_change_time::now_ms() as i64 as u64,
                    )?;
                    Ok(())
                })
            })
            .map_err(|error| TagError::Repository(error.to_string()))
    }

    pub fn reorder_tags(&self, tag_ids: Vec<String>) -> Result<Vec<TagRecord>, TagError> {
        self.service().reorder_tags(tag_ids)
    }

    pub fn get_active_tag_selection(&self) -> Result<ActiveTagSelection, TagError> {
        self.service().get_active_tag_selection()
    }

    pub fn set_active_tag_id(&self, tag_id: Option<String>) -> Result<(), TagError> {
        self.service().set_active_tag_id(tag_id)
    }

    /// Loads the deterministic pipeline attached to a project. Pipelines are
    /// stored separately so project rows remain schema-compatible.
    pub fn get_project_pipeline(
        &self,
        project_id: &str,
    ) -> Result<Option<ProjectPipelineConfig>, TagError> {
        self.repository
            .get_db()
            .and_then(|db| {
                db.with_read_connection(|conn| {
                    let json: Option<String> = conn
                        .query_row(
                            "SELECT pipeline_json FROM project_pipelines WHERE project_id = ?1",
                            [project_id],
                            |row| row.get(0),
                        )
                        .optional()?;
                    json.map(|value| {
                        serde_json::from_str(&value).map_err(|error| {
                            DatabaseError::QueryError(rusqlite::Error::ToSqlConversionFailure(
                                Box::new(error),
                            ))
                        })
                    })
                    .transpose()
                })
            })
            .map_err(|error| TagError::Repository(error.to_string()))
    }

    /// Persists (or replaces) a project's pipeline snapshot.
    pub fn set_project_pipeline(
        &self,
        project_id: &str,
        pipeline: &ProjectPipelineConfig,
    ) -> Result<(), TagError> {
        let payload = serde_json::to_string(pipeline)
            .map_err(|error| TagError::Repository(error.to_string()))?;
        let now_ms = legacy_change_time::now_ms() as i64;
        self.repository
            .get_db()
            .and_then(|db| {
                db.with_transaction(|tx| {
                    tx.execute(
                        "INSERT INTO project_pipelines (project_id, pipeline_json, updated_at) VALUES (?1, ?2, ?3) ON CONFLICT(project_id) DO UPDATE SET pipeline_json = excluded.pipeline_json, updated_at = excluded.updated_at",
                        rusqlite::params![project_id, payload, now_ms],
                    )?;
                    let val = serde_json::to_value(pipeline)
                        .map_err(|e| DatabaseError::QueryError(rusqlite::Error::ToSqlConversionFailure(Box::new(e))))?;
                    record_local_field_change_in_transaction(
                        tx,
                        SyncEntityKind::Project,
                        project_id,
                        "pipeline",
                        val,
                        now_ms as u64,
                    )?;
                    Ok(())
                })
            })
            .map_err(|error| TagError::Repository(error.to_string()))
    }

    fn service(&self) -> TagRepositoryService<'_> {
        TagRepositoryService::new(&self.repository, self.ids.as_ref(), self.clock.as_ref())
    }
}

const PROJECT_COLUMNS: [&str; 8] = [
    "id",
    "name",
    "description",
    "icon",
    "color",
    "sort_order",
    "created_at",
    "updated_at",
];

const PROJECT_UPDATE_COLUMNS: [&str; 5] = ["name", "icon", "color", "description", "updated_at"];

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

fn project_upsert_sql() -> String {
    let update_assignments = PROJECT_COLUMNS
        .iter()
        .filter(|&&col| col != "id")
        .map(|col| format!("{col} = excluded.{col}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "INSERT INTO projects ({}) VALUES ({}) ON CONFLICT(id) DO UPDATE SET {}",
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
    format!("UPDATE projects SET {assignments} WHERE id = :id")
}

fn load_projects(conn: &rusqlite::Connection) -> Result<Vec<TagRecord>, DatabaseError> {
    let columns = project_select_columns();
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {columns} FROM projects ORDER BY sort_order, id"
    ))?;
    let rows = stmt.query_map([], map_row_to_tag)?;
    let mut tags = Vec::new();
    for row in rows {
        tags.push(row?);
    }
    Ok(tags)
}

pub(crate) fn load_tags_in_transaction(
    tx: &rusqlite::Transaction<'_>,
) -> Result<Vec<TagRecord>, DatabaseError> {
    load_projects(tx)
}

pub(crate) fn insert_tags_in_transaction(
    tx: &rusqlite::Transaction<'_>,
    tags: &[TagRecord],
) -> Result<(), DatabaseError> {
    let sql = project_insert_sql();
    for tag in tags {
        write_tag_row(tx, &sql, tag)?;
    }
    Ok(())
}

fn upsert_tags_in_transaction(
    tx: &rusqlite::Transaction<'_>,
    tags: &[TagRecord],
) -> Result<(), DatabaseError> {
    let sql = project_upsert_sql();
    for tag in tags {
        write_tag_row(tx, &sql, tag)?;
    }
    Ok(())
}

pub(crate) fn delete_tags_in_transaction(
    tx: &rusqlite::Transaction<'_>,
) -> Result<(), DatabaseError> {
    tx.execute("DELETE FROM projects", [])?;
    Ok(())
}

pub(crate) fn replace_tags_in_transaction(
    tx: &rusqlite::Transaction<'_>,
    tags: &[TagRecord],
) -> Result<(), DatabaseError> {
    tx.execute(
        "CREATE TEMPORARY TABLE keep_projects (id TEXT PRIMARY KEY)",
        [],
    )?;
    {
        let mut keep = tx.prepare_cached("INSERT INTO keep_projects (id) VALUES (?1)")?;
        for tag in tags.iter().filter(|tag| !tag.id.is_empty()) {
            keep.execute([tag.id.as_str()])?;
        }
    }
    tx.execute(
        "DELETE FROM projects WHERE id NOT IN (SELECT id FROM keep_projects)",
        [],
    )?;
    tx.execute("DROP TABLE keep_projects", [])?;
    upsert_tags_in_transaction(tx, tags)
}

impl<D> SqliteProjectRepository<D>
where
    D: DatabasePort,
{
    fn list_tags(conn: &rusqlite::Connection) -> Result<Vec<TagRecord>, DatabaseError> {
        load_projects(conn)
    }

    fn get_by_id_from_conn(
        conn: &rusqlite::Connection,
        tag_id: &str,
    ) -> Result<Option<TagRecord>, DatabaseError> {
        let columns = project_select_columns();
        let mut stmt =
            conn.prepare_cached(&format!("SELECT {columns} FROM projects WHERE id = ?1"))?;
        let mut rows = stmt.query([tag_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(map_row_to_tag(row)?))
        } else {
            Ok(None)
        }
    }
}

impl<D> TagStore for SqliteProjectRepository<D>
where
    D: DatabasePort,
{
    fn load_state(&self) -> Result<TagStoredState, TagError> {
        self.get_db()
            .and_then(|db| {
                db.with_read_connection(|conn| {
                    let tx = conn
                        .unchecked_transaction()
                        .map_err(DatabaseError::QueryError)?;
                    let tags = Self::list_tags(&tx)?;
                    let active_tag_setting_json = tx
                        .query_row(
                            "SELECT value FROM app_settings WHERE key = ?1",
                            [ACTIVE_PROJECT_SETTINGS_KEY],
                            |row| row.get(0),
                        )
                        .optional()?;
                    tx.commit().map_err(DatabaseError::QueryError)?;
                    Ok(TagStoredState {
                        tags,
                        active_tag_setting_json,
                    })
                })
            })
            .map_err(|error| TagError::Repository(error.to_string()))
    }

    fn insert_tag(&self, tag: TagRecord) -> Result<TagRecord, TagError> {
        self.get_db()
            .and_then(|db| {
                db.with_transaction(|tx| {
                    write_tag_row(tx, &project_insert_sql(), &tag)?;
                    record_project_sync_fields(tx, &tag, None)?;
                    Ok(())
                })
            })
            .map_err(|error| TagError::Repository(error.to_string()))?;
        Ok(tag)
    }

    fn update_tag(
        &self,
        tag_id: &str,
        patch: TagPatch,
        updated_at: u64,
    ) -> Result<Option<TagRecord>, TagError> {
        self.get_db()
            .and_then(|db| {
                db.with_rw_transaction(|tx| {
                    let Some(mut tag) = Self::get_by_id_from_conn(tx, tag_id)? else {
                        return Ok(None);
                    };
                    apply_patch(&mut tag, patch);
                    tag.updated_at = updated_at;
                    update_tag_row(tx, &tag)?;
                    record_project_sync_fields(tx, &tag, None)?;
                    Ok(Some(tag))
                })
            })
            .map_err(|error| TagError::Repository(error.to_string()))
    }

    fn delete_tag(&self, tag_id: &str) -> Result<(), TagError> {
        self.get_db()
            .and_then(|db| {
                db.with_transaction(|tx| {
                    tx.execute("DELETE FROM projects WHERE id = ?1", [tag_id])?;
                    record_local_delete_in_transaction(
                        tx,
                        SyncEntityKind::Project,
                        tag_id,
                        legacy_change_time::now_ms(),
                    )?;
                    Ok(())
                })
            })
            .map_err(|error| TagError::Repository(error.to_string()))
    }

    fn replace_tags(&self, tags: Vec<TagRecord>) -> Result<(), TagError> {
        self.get_db()
            .and_then(|db| {
                db.with_transaction(|tx| {
                    let existing_ids = load_projects(tx)?
                        .into_iter()
                        .map(|tag| tag.id)
                        .collect::<Vec<_>>();
                    replace_tags_in_transaction(tx, &tags)?;
                    let now_ms = legacy_change_time::now_ms();
                    for existing_id in existing_ids
                        .iter()
                        .filter(|existing_id| !tags.iter().any(|tag| tag.id == **existing_id))
                    {
                        record_local_delete_in_transaction(
                            tx,
                            SyncEntityKind::Project,
                            existing_id,
                            now_ms,
                        )?;
                    }
                    for tag in &tags {
                        record_project_sync_fields(tx, tag, Some(tag.sort_order))?;
                    }
                    Ok(())
                })
            })
            .map_err(|error| TagError::Repository(error.to_string()))
    }

    fn reorder_tags(&self, tag_ids: Vec<String>) -> Result<Vec<TagRecord>, TagError> {
        self.get_db()
            .and_then(|db| {
                db.with_rw_transaction(|tx| {
                    let mut stmt =
                        tx.prepare_cached("UPDATE projects SET sort_order = ?1 WHERE id = ?2")?;
                    for (sort_order, id) in tag_ids.iter().enumerate() {
                        stmt.execute(rusqlite::params![sort_order as i64, id])?;
                    }
                    drop(stmt);
                    let tags = Self::list_tags(tx)?;
                    let now_ms = legacy_change_time::now_ms();
                    for tag in &tags {
                        record_local_field_change_in_transaction(
                            tx,
                            SyncEntityKind::Project,
                            &tag.id,
                            "sortOrder",
                            serde_json::json!(tag.sort_order),
                            now_ms,
                        )?;
                    }
                    Ok(tags)
                })
            })
            .map_err(|error| TagError::Repository(error.to_string()))
    }

    fn set_active_tag_setting_json(&self, setting_json: String) -> Result<(), TagError> {
        self.get_db()
            .and_then(|db| {
                db.with_transaction(|tx| {
                    tx.execute(
                        "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
                         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                        rusqlite::params![ACTIVE_PROJECT_SETTINGS_KEY, setting_json],
                    )?;
                    Ok(())
                })
            })
            .map_err(|error| TagError::Repository(error.to_string()))
    }
}

pub(crate) fn record_project_sync_fields(
    tx: &rusqlite::Transaction<'_>,
    tag: &TagRecord,
    sort_order: Option<usize>,
) -> Result<(), DatabaseError> {
    let now_ms = tag.updated_at.max(tag.created_at);
    let mut fields = vec![
        ("name", serde_json::json!(tag.name)),
        ("description", serde_json::json!(tag.description)),
        ("icon", serde_json::json!(tag.icon)),
        ("color", serde_json::json!(tag.color)),
        ("createdAt", serde_json::json!(tag.created_at)),
        ("updatedAt", serde_json::json!(tag.updated_at)),
    ];
    if let Some(sort_order) = sort_order {
        fields.push(("sortOrder", serde_json::json!(sort_order)));
    }
    for (field, value) in fields {
        record_local_field_change_in_transaction(
            tx,
            SyncEntityKind::Project,
            &tag.id,
            field,
            value,
            now_ms,
        )?;
    }
    Ok(())
}

fn write_tag_row(
    conn: &rusqlite::Connection,
    sql: &str,
    tag: &TagRecord,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        sql,
        rusqlite::named_params! {
            ":id": &tag.id,
            ":name": &tag.name,
            ":description": &tag.description,
            ":icon": &tag.icon,
            ":color": &tag.color,
            ":sort_order": tag.sort_order as i64,
            ":created_at": tag.created_at as i64,
            ":updated_at": tag.updated_at as i64,
        },
    )?;
    Ok(())
}

fn update_tag_row(conn: &rusqlite::Connection, tag: &TagRecord) -> Result<(), rusqlite::Error> {
    conn.execute(
        &project_update_sql(),
        rusqlite::named_params! {
            ":name": &tag.name,
            ":icon": &tag.icon,
            ":color": &tag.color,
            ":description": &tag.description,
            ":updated_at": tag.updated_at as i64,
            ":id": &tag.id,
        },
    )?;
    Ok(())
}

fn apply_patch(tag: &mut TagRecord, patch: TagPatch) {
    if let Some(value) = patch.name {
        tag.name = value;
    }
    if let Some(value) = patch.icon {
        tag.icon = value;
    }
    if let Some(value) = patch.color {
        tag.color = value;
    }
    if let Some(value) = patch.description {
        tag.description = value;
    }
}

fn map_row_to_tag(row: &rusqlite::Row) -> rusqlite::Result<TagRecord> {
    let id: String = row.get("id")?;
    let name: String = row.get("name")?;
    let description: String = row.get("description")?;
    let icon: Option<String> = row.get("icon")?;
    let color: Option<String> = row.get("color")?;
    let sort_order: i64 = row.get("sort_order")?;
    let created_at: i64 = row.get("created_at")?;
    let updated_at: i64 = row.get("updated_at")?;
    Ok(TagRecord {
        id,
        name,
        description,
        icon: icon.unwrap_or_default(),
        color: color.unwrap_or_default(),
        sort_order: checked_usize_column(row, "sort_order", sort_order)?,
        created_at: checked_u64_column(row, "created_at", created_at)?,
        updated_at: checked_u64_column(row, "updated_at", updated_at)?,
    })
}

fn checked_u64_column(row: &rusqlite::Row<'_>, column: &str, value: i64) -> rusqlite::Result<u64> {
    let column_index = row.as_ref().column_index(column)?;
    u64::try_from(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(column_index, Type::Integer, Box::new(error))
    })
}

fn checked_usize_column(
    row: &rusqlite::Row<'_>,
    column: &str,
    value: i64,
) -> rusqlite::Result<usize> {
    let column_index = row.as_ref().column_index(column)?;
    usize::try_from(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(column_index, Type::Integer, Box::new(error))
    })
}

#[async_trait::async_trait]
impl<D> TagRepository for SqliteProjectRepository<D>
where
    D: DatabasePort,
{
    async fn count_tags(&self) -> Result<u64, DashboardError> {
        let count: i64 = self
            .get_db()
            .and_then(|db| {
                db.with_read_connection(|conn| {
                    conn.query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))
                        .map_err(DatabaseError::QueryError)
                })
            })
            .map_err(|error| DashboardError::TagRepository(error.to_string()))?;
        Ok(count as u64)
    }
}

impl<D> ProjectRepository for SqliteProjectAdapter<D>
where
    D: DatabasePort + 'static,
{
    fn list_projects(&self) -> Result<Vec<ProjectRecord>, ProjectError> {
        self.repository
            .get_db()
            .and_then(|db| {
                db.with_read_connection(|conn| {
                    let mut stmt = conn.prepare_cached(
                        "SELECT p.id, p.name, p.description, p.icon, p.color, p.sort_order, p.created_at, p.updated_at, pp.pipeline_json
                         FROM projects p
                         LEFT JOIN project_pipelines pp ON pp.project_id = p.id
                         ORDER BY p.sort_order, p.id",
                    )?;
                    let rows = stmt.query_map([], |row| {
                        let id: String = row.get(0)?;
                        let name: String = row.get(1)?;
                        let description: String = row.get(2)?;
                        let icon: Option<String> = row.get(3)?;
                        let color: Option<String> = row.get(4)?;
                        let sort_order: i64 = row.get(5)?;
                        let created_at: i64 = row.get(6)?;
                        let updated_at: i64 = row.get(7)?;
                        let pipeline_json: Option<String> = row.get(8)?;

                        let pipeline = pipeline_json
                            .map(|s| serde_json::from_str::<ProjectPipelineConfig>(&s))
                            .transpose()
                            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

                        let sort_order = usize::try_from(sort_order).map_err(|e| {
                            rusqlite::Error::FromSqlConversionFailure(5, Type::Integer, Box::new(e))
                        })?;
                        let created_at = u64::try_from(created_at).map_err(|e| {
                            rusqlite::Error::FromSqlConversionFailure(6, Type::Integer, Box::new(e))
                        })?;
                        let updated_at = u64::try_from(updated_at).map_err(|e| {
                            rusqlite::Error::FromSqlConversionFailure(7, Type::Integer, Box::new(e))
                        })?;

                        Ok(ProjectRecord {
                            id,
                            name,
                            description,
                            icon: icon.filter(|s| !s.is_empty()),
                            color: color.filter(|s| !s.is_empty()),
                            sort_order,
                            created_at,
                            updated_at,
                            pipeline,
                        })
                    })?;
                    let mut projects = Vec::new();
                    for row in rows {
                        projects.push(row?);
                    }
                    Ok(projects)
                })
            })
            .map_err(|e| ProjectError::Repository(e.to_string()))
    }

    fn create_project(&self, input: ProjectCreateInput) -> Result<ProjectRecord, ProjectError> {
        let pipeline = input.pipeline;
        let tag = self
            .create_tag(TagCreateInput {
                name: input.name,
                description: input.description,
                icon: input.icon,
                color: input.color,
            })
            .map_err(|e| ProjectError::Repository(e.to_string()))?;
        if let Some(value) = pipeline.as_ref() {
            self.set_project_pipeline(&tag.id, value)
                .map_err(|e| ProjectError::Repository(e.to_string()))?;
        }
        Ok(ProjectRecord::from_tag(tag, pipeline))
    }

    fn update_project(
        &self,
        project_id: &str,
        updates: ProjectUpdateInput,
    ) -> Result<Option<ProjectRecord>, ProjectError> {
        let pipeline_update = updates.pipeline;
        let tag = self
            .update_tag(
                project_id,
                TagUpdateInput {
                    name: updates.name,
                    description: updates.description,
                    icon: updates.icon,
                    color: updates.color,
                },
            )
            .map_err(|e| ProjectError::Repository(e.to_string()))?;
        let Some(tag) = tag else {
            return Ok(None);
        };
        if let Some(value) = pipeline_update.as_ref() {
            self.set_project_pipeline(project_id, value)
                .map_err(|e| ProjectError::Repository(e.to_string()))?;
        }
        let pipeline = if pipeline_update.is_some() {
            pipeline_update
        } else {
            self.get_project_pipeline(project_id)
                .map_err(|e| ProjectError::Repository(e.to_string()))?
        };
        Ok(Some(ProjectRecord::from_tag(tag, pipeline)))
    }

    fn delete_project(&self, project_id: &str) -> Result<(), ProjectError> {
        self.delete_tag(project_id)
            .map_err(|e| ProjectError::Repository(e.to_string()))
    }

    fn delete_project_with_cascade(
        &self,
        project_id: &str,
        cascade_action: &str,
    ) -> Result<(), ProjectError> {
        self.delete_project_with_cascade(project_id, cascade_action)
            .map_err(|e| ProjectError::Repository(e.to_string()))
    }

    fn reorder_projects(
        &self,
        project_ids: Vec<String>,
    ) -> Result<Vec<ProjectRecord>, ProjectError> {
        self.reorder_tags(project_ids)
            .map_err(|e| ProjectError::Repository(e.to_string()))?;
        self.list_projects()
    }

    fn get_active_project_id(&self) -> Result<Option<String>, ProjectError> {
        Ok(self
            .get_active_tag_selection()
            .map_err(|e| ProjectError::Repository(e.to_string()))?
            .tag_id)
    }

    fn set_active_project_id(&self, project_id: Option<String>) -> Result<(), ProjectError> {
        self.set_active_tag_id(project_id)
            .map_err(|e| ProjectError::Repository(e.to_string()))
    }
}
