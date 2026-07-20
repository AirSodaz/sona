use crate::DatabaseError;
use crate::ports::Database as DatabasePort;
use rusqlite::OptionalExtension;
use rusqlite::types::Type;
use serde_json::Value;
use sona_core::dashboard::error::DashboardServiceError;
use sona_core::dashboard::ports::TagRepository;
use sona_core::ports::time::UnixMillisClock;
use sona_core::sync::SyncEntityKind;
use sona_core::tag::{
    ACTIVE_TAG_SETTINGS_KEY, ActiveTagSelection, TagCreateInput, TagError, TagIdGenerator,
    TagListOptions, TagPatch, TagRecord, TagRepositoryService, TagRepositorySnapshot, TagStore,
    TagStoredState, TagUpdateInput,
};
use std::sync::Arc;

use crate::legacy_change_time;
use crate::sync_repository::{
    record_local_delete_in_transaction, record_local_field_change_in_transaction,
};

#[derive(Clone)]
pub struct SqliteTagRepository<D = crate::Database>
where
    D: DatabasePort,
{
    db: Arc<D>,
}

crate::impl_db_repository!(SqliteTagRepository);

pub struct SqliteTagAdapter<D = crate::Database>
where
    D: DatabasePort,
{
    repository: SqliteTagRepository<D>,
    ids: Arc<dyn TagIdGenerator>,
    clock: Arc<dyn UnixMillisClock>,
}

impl<D> SqliteTagAdapter<D>
where
    D: DatabasePort,
{
    pub fn new(db: Arc<D>, ids: Arc<dyn TagIdGenerator>, clock: Arc<dyn UnixMillisClock>) -> Self {
        Self {
            repository: SqliteTagRepository::new(db),
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

    pub fn reorder_tags(&self, tag_ids: Vec<String>) -> Result<Vec<TagRecord>, TagError> {
        self.service().reorder_tags(tag_ids)
    }

    pub fn get_active_tag_selection(&self) -> Result<ActiveTagSelection, TagError> {
        self.service().get_active_tag_selection()
    }

    pub fn set_active_tag_id(&self, tag_id: Option<String>) -> Result<(), TagError> {
        self.service().set_active_tag_id(tag_id)
    }

    fn service(&self) -> TagRepositoryService<'_> {
        TagRepositoryService::new(&self.repository, self.ids.as_ref(), self.clock.as_ref())
    }
}

const TAG_COLUMNS: [&str; 8] = [
    "id",
    "name",
    "description",
    "icon",
    "color",
    "sort_order",
    "created_at",
    "updated_at",
];

const TAG_UPDATE_COLUMNS: [&str; 5] = ["name", "icon", "color", "description", "updated_at"];

fn tag_column_list(columns: &[&str]) -> String {
    columns.join(", ")
}

fn tag_named_param_list(columns: &[&str]) -> String {
    columns
        .iter()
        .map(|column| format!(":{column}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn tag_select_columns() -> String {
    tag_column_list(&TAG_COLUMNS)
}

fn tag_insert_sql() -> String {
    format!(
        "INSERT INTO tags ({}) VALUES ({})",
        tag_column_list(&TAG_COLUMNS),
        tag_named_param_list(&TAG_COLUMNS)
    )
}

fn tag_upsert_sql() -> String {
    let update_assignments = TAG_COLUMNS
        .iter()
        .filter(|&&col| col != "id")
        .map(|col| format!("{col} = excluded.{col}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "INSERT INTO tags ({}) VALUES ({}) ON CONFLICT(id) DO UPDATE SET {}",
        tag_column_list(&TAG_COLUMNS),
        tag_named_param_list(&TAG_COLUMNS),
        update_assignments
    )
}

fn tag_update_sql() -> String {
    let assignments = TAG_UPDATE_COLUMNS
        .iter()
        .map(|column| format!("{column} = :{column}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("UPDATE tags SET {assignments} WHERE id = :id")
}

fn load_tags(conn: &rusqlite::Connection) -> Result<Vec<TagRecord>, DatabaseError> {
    let columns = tag_select_columns();
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {columns} FROM tags ORDER BY sort_order, id"
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
    load_tags(tx)
}

pub(crate) fn insert_tags_in_transaction(
    tx: &rusqlite::Transaction<'_>,
    tags: &[TagRecord],
) -> Result<(), DatabaseError> {
    let sql = tag_insert_sql();
    for tag in tags {
        write_tag_row(tx, &sql, tag)?;
    }
    Ok(())
}

fn upsert_tags_in_transaction(
    tx: &rusqlite::Transaction<'_>,
    tags: &[TagRecord],
) -> Result<(), DatabaseError> {
    let sql = tag_upsert_sql();
    for tag in tags {
        write_tag_row(tx, &sql, tag)?;
    }
    Ok(())
}

pub(crate) fn delete_tags_in_transaction(
    tx: &rusqlite::Transaction<'_>,
) -> Result<(), DatabaseError> {
    tx.execute("DELETE FROM tags", [])?;
    Ok(())
}

pub(crate) fn replace_tags_in_transaction(
    tx: &rusqlite::Transaction<'_>,
    tags: &[TagRecord],
) -> Result<(), DatabaseError> {
    tx.execute("CREATE TEMPORARY TABLE keep_tags (id TEXT PRIMARY KEY)", [])?;
    {
        let mut keep = tx.prepare_cached("INSERT INTO keep_tags (id) VALUES (?1)")?;
        for tag in tags.iter().filter(|tag| !tag.id.is_empty()) {
            keep.execute([tag.id.as_str()])?;
        }
    }
    tx.execute(
        "DELETE FROM tags WHERE id NOT IN (SELECT id FROM keep_tags)",
        [],
    )?;
    tx.execute("DROP TABLE keep_tags", [])?;
    upsert_tags_in_transaction(tx, tags)
}

impl<D> SqliteTagRepository<D>
where
    D: DatabasePort,
{
    fn list_tags(conn: &rusqlite::Connection) -> Result<Vec<TagRecord>, DatabaseError> {
        load_tags(conn)
    }

    fn get_by_id_from_conn(
        conn: &rusqlite::Connection,
        tag_id: &str,
    ) -> Result<Option<TagRecord>, DatabaseError> {
        let columns = tag_select_columns();
        let mut stmt = conn.prepare_cached(&format!("SELECT {columns} FROM tags WHERE id = ?1"))?;
        let mut rows = stmt.query([tag_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(map_row_to_tag(row)?))
        } else {
            Ok(None)
        }
    }
}

impl<D> TagStore for SqliteTagRepository<D>
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
                            [ACTIVE_TAG_SETTINGS_KEY],
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
                    write_tag_row(tx, &tag_insert_sql(), &tag)?;
                    record_tag_sync_fields(tx, &tag, None)?;
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
                    record_tag_sync_fields(tx, &tag, None)?;
                    Ok(Some(tag))
                })
            })
            .map_err(|error| TagError::Repository(error.to_string()))
    }

    fn delete_tag(&self, tag_id: &str) -> Result<(), TagError> {
        self.get_db()
            .and_then(|db| {
                db.with_transaction(|tx| {
                    tx.execute("DELETE FROM tags WHERE id = ?1", [tag_id])?;
                    record_local_delete_in_transaction(
                        tx,
                        SyncEntityKind::Tag,
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
                    let existing_ids = load_tags(tx)?
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
                            SyncEntityKind::Tag,
                            existing_id,
                            now_ms,
                        )?;
                    }
                    for tag in &tags {
                        record_tag_sync_fields(tx, tag, Some(tag.sort_order))?;
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
                        tx.prepare_cached("UPDATE tags SET sort_order = ?1 WHERE id = ?2")?;
                    for (sort_order, id) in tag_ids.iter().enumerate() {
                        stmt.execute(rusqlite::params![sort_order as i64, id])?;
                    }
                    drop(stmt);
                    let tags = Self::list_tags(tx)?;
                    let now_ms = legacy_change_time::now_ms();
                    for tag in &tags {
                        record_local_field_change_in_transaction(
                            tx,
                            SyncEntityKind::Tag,
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
                        rusqlite::params![ACTIVE_TAG_SETTINGS_KEY, setting_json],
                    )?;
                    Ok(())
                })
            })
            .map_err(|error| TagError::Repository(error.to_string()))
    }
}

pub(crate) fn record_tag_sync_fields(
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
            SyncEntityKind::Tag,
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
        &tag_update_sql(),
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
    let icon: String = row.get("icon")?;
    let color: String = row.get("color")?;
    let sort_order: i64 = row.get("sort_order")?;
    let created_at: i64 = row.get("created_at")?;
    let updated_at: i64 = row.get("updated_at")?;
    Ok(TagRecord {
        id,
        name,
        description,
        icon,
        color,
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
impl<D> TagRepository for SqliteTagRepository<D>
where
    D: DatabasePort,
{
    async fn count_tags(&self) -> Result<u64, DashboardServiceError> {
        let state = TagStore::load_state(self)
            .map_err(|error| DashboardServiceError::TagRepository(error.to_string()))?;
        Ok(state.tags.len() as u64)
    }
}
