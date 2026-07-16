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
    ACTIVE_TAG_SETTINGS_KEY, ActiveTagSelection, TagCreateInput, TagDefaults, TagIdGenerator,
    TagListOptions, TagPatch, TagRecord, TagRepositoryService, TagRepositorySnapshot, TagStore,
    TagStoredState, TagUpdateInput,
};
use std::sync::Arc;

use crate::sync_repository::{
    record_local_delete_in_transaction, record_local_field_change_in_transaction, sync_now_ms,
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

    pub fn load_state(&self) -> Result<TagRepositorySnapshot, String> {
        self.service().load_state()
    }

    pub fn list_tags(&self, options: TagListOptions) -> Result<Vec<TagRecord>, String> {
        self.service().list_tags(options)
    }

    pub fn replace_tags_json(&self, tags: Vec<Value>) -> Result<(), String> {
        self.service().replace_tags_json(tags)
    }

    pub fn replace_tags(&self, tags: Vec<TagRecord>) -> Result<(), String> {
        self.service().replace_tags(tags)
    }

    pub fn create_tag(&self, input: TagCreateInput) -> Result<TagRecord, String> {
        self.service().create_tag(input)
    }

    pub fn update_tag_json(
        &self,
        tag_id: &str,
        updates: Value,
    ) -> Result<Option<TagRecord>, String> {
        self.service().update_tag_json(tag_id, updates)
    }

    pub fn update_tag(
        &self,
        tag_id: &str,
        updates: TagUpdateInput,
    ) -> Result<Option<TagRecord>, String> {
        self.service().update_tag(tag_id, updates)
    }

    pub fn delete_tag(&self, tag_id: &str) -> Result<(), String> {
        self.service().delete_tag(tag_id)
    }

    pub fn reorder_tags(&self, tag_ids: Vec<String>) -> Result<Vec<TagRecord>, String> {
        self.service().reorder_tags(tag_ids)
    }

    pub fn get_active_tag_selection(&self) -> Result<ActiveTagSelection, String> {
        self.service().get_active_tag_selection()
    }

    pub fn set_active_tag_id(&self, tag_id: Option<String>) -> Result<(), String> {
        self.service().set_active_tag_id(tag_id)
    }

    fn service(&self) -> TagRepositoryService<'_> {
        TagRepositoryService::new(&self.repository, self.ids.as_ref(), self.clock.as_ref())
    }
}

const TAG_COLUMNS: [&str; 14] = [
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

const TAG_UPDATE_COLUMNS: [&str; 11] = [
    "name",
    "icon",
    "color",
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
        let mut tag = row?;
        hydrate_tag_default_links(conn, &mut tag)?;
        tags.push(tag);
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
        replace_tag_default_links(tx, &tag.id, &tag.defaults)?;
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
        replace_tag_default_links(tx, &tag.id, &tag.defaults)?;
    }
    Ok(())
}

pub(crate) fn delete_tags_in_transaction(
    tx: &rusqlite::Transaction<'_>,
) -> Result<(), DatabaseError> {
    tx.execute("DELETE FROM tag_default_links", [])?;
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
            let mut tag = map_row_to_tag(row)?;
            hydrate_tag_default_links(conn, &mut tag)?;
            Ok(Some(tag))
        } else {
            Ok(None)
        }
    }
}

impl<D> TagStore for SqliteTagRepository<D>
where
    D: DatabasePort,
{
    fn load_state(&self) -> Result<TagStoredState, String> {
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
            .map_err(|error| error.to_string())
    }

    fn insert_tag(&self, tag: TagRecord) -> Result<TagRecord, String> {
        self.get_db()
            .and_then(|db| {
                db.with_transaction(|tx| {
                    write_tag_row(tx, &tag_insert_sql(), &tag)?;
                    replace_tag_default_links(tx, &tag.id, &tag.defaults)?;
                    record_tag_sync_fields(tx, &tag, None)?;
                    Ok(())
                })
            })
            .map_err(|error| error.to_string())?;
        Ok(tag)
    }

    fn update_tag(
        &self,
        tag_id: &str,
        patch: TagPatch,
        updated_at: u64,
    ) -> Result<Option<TagRecord>, String> {
        self.get_db()
            .and_then(|db| {
                db.with_rw_transaction(|tx| {
                    let Some(mut tag) = Self::get_by_id_from_conn(tx, tag_id)? else {
                        return Ok(None);
                    };
                    let replace_text_links =
                        patch.defaults.enabled_text_replacement_set_ids.is_some();
                    let replace_hotword_links = patch.defaults.enabled_hotword_set_ids.is_some();
                    let replace_keyword_links =
                        patch.defaults.enabled_polish_keyword_set_ids.is_some();
                    let replace_speaker_links =
                        patch.defaults.enabled_speaker_profile_ids.is_some();
                    apply_patch(&mut tag, patch);
                    tag.updated_at = updated_at;
                    update_tag_row(tx, &tag)?;
                    if replace_text_links {
                        replace_tag_links_for_kind(
                            tx,
                            tag_id,
                            LINK_KIND_TEXT_REPLACEMENT,
                            &tag.defaults.enabled_text_replacement_set_ids,
                        )?;
                    }
                    if replace_hotword_links {
                        replace_tag_links_for_kind(
                            tx,
                            tag_id,
                            LINK_KIND_HOTWORD,
                            &tag.defaults.enabled_hotword_set_ids,
                        )?;
                    }
                    if replace_keyword_links {
                        replace_tag_links_for_kind(
                            tx,
                            tag_id,
                            LINK_KIND_POLISH_KEYWORD,
                            &tag.defaults.enabled_polish_keyword_set_ids,
                        )?;
                    }
                    if replace_speaker_links {
                        replace_tag_links_for_kind(
                            tx,
                            tag_id,
                            LINK_KIND_SPEAKER_PROFILE,
                            &tag.defaults.enabled_speaker_profile_ids,
                        )?;
                    }
                    record_tag_sync_fields(tx, &tag, None)?;
                    Ok(Some(tag))
                })
            })
            .map_err(|error| error.to_string())
    }

    fn delete_tag(&self, tag_id: &str) -> Result<(), String> {
        self.get_db()
            .and_then(|db| {
                db.with_transaction(|tx| {
                    tx.execute("DELETE FROM tags WHERE id = ?1", [tag_id])?;
                    record_local_delete_in_transaction(
                        tx,
                        SyncEntityKind::Tag,
                        tag_id,
                        sync_now_ms(),
                    )?;
                    Ok(())
                })
            })
            .map_err(|error| error.to_string())
    }

    fn replace_tags(&self, tags: Vec<TagRecord>) -> Result<(), String> {
        self.get_db()
            .and_then(|db| {
                db.with_transaction(|tx| {
                    let existing_ids = load_tags(tx)?
                        .into_iter()
                        .map(|tag| tag.id)
                        .collect::<Vec<_>>();
                    replace_tags_in_transaction(tx, &tags)?;
                    let now_ms = sync_now_ms();
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
            .map_err(|error| error.to_string())
    }

    fn reorder_tags(&self, tag_ids: Vec<String>) -> Result<Vec<TagRecord>, String> {
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
                    let now_ms = sync_now_ms();
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
            .map_err(|error| error.to_string())
    }

    fn set_active_tag_setting_json(&self, setting_json: String) -> Result<(), String> {
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
        (
            "summaryTemplateId",
            serde_json::json!(tag.defaults.summary_template_id),
        ),
        (
            "translationLanguage",
            serde_json::json!(tag.defaults.translation_language),
        ),
        (
            "polishPresetId",
            serde_json::json!(tag.defaults.polish_preset_id),
        ),
        (
            "polishScenario",
            serde_json::json!(tag.defaults.polish_scenario),
        ),
        (
            "polishContext",
            serde_json::json!(tag.defaults.polish_context),
        ),
        (
            "exportFileNamePrefix",
            serde_json::json!(tag.defaults.export_file_name_prefix),
        ),
        (
            "enabledTextReplacementSetIds",
            serde_json::json!(tag.defaults.enabled_text_replacement_set_ids),
        ),
        (
            "enabledHotwordSetIds",
            serde_json::json!(tag.defaults.enabled_hotword_set_ids),
        ),
        (
            "enabledPolishKeywordSetIds",
            serde_json::json!(tag.defaults.enabled_polish_keyword_set_ids),
        ),
        (
            "enabledSpeakerProfileIds",
            serde_json::json!(tag.defaults.enabled_speaker_profile_ids),
        ),
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
            ":summary_template_id": &tag.defaults.summary_template_id,
            ":translation_language": &tag.defaults.translation_language,
            ":polish_preset_id": &tag.defaults.polish_preset_id,
            ":polish_scenario": &tag.defaults.polish_scenario,
            ":polish_context": &tag.defaults.polish_context,
            ":export_file_name_prefix": &tag.defaults.export_file_name_prefix,
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
            ":summary_template_id": &tag.defaults.summary_template_id,
            ":translation_language": &tag.defaults.translation_language,
            ":polish_preset_id": &tag.defaults.polish_preset_id,
            ":polish_scenario": &tag.defaults.polish_scenario,
            ":polish_context": &tag.defaults.polish_context,
            ":export_file_name_prefix": &tag.defaults.export_file_name_prefix,
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
    if let Some(value) = patch.defaults.summary_template_id {
        tag.defaults.summary_template_id = value;
    }
    if let Some(value) = patch.defaults.translation_language {
        tag.defaults.translation_language = value;
    }
    if let Some(value) = patch.defaults.polish_preset_id {
        tag.defaults.polish_preset_id = value;
    }
    if let Some(value) = patch.defaults.polish_scenario {
        tag.defaults.polish_scenario = Some(value);
    }
    if let Some(value) = patch.defaults.polish_context {
        tag.defaults.polish_context = Some(value);
    }
    if let Some(value) = patch.defaults.export_file_name_prefix {
        tag.defaults.export_file_name_prefix = value;
    }
    if let Some(value) = patch.defaults.enabled_text_replacement_set_ids {
        tag.defaults.enabled_text_replacement_set_ids = value;
    }
    if let Some(value) = patch.defaults.enabled_hotword_set_ids {
        tag.defaults.enabled_hotword_set_ids = value;
    }
    if let Some(value) = patch.defaults.enabled_polish_keyword_set_ids {
        tag.defaults.enabled_polish_keyword_set_ids = value;
    }
    if let Some(value) = patch.defaults.enabled_speaker_profile_ids {
        tag.defaults.enabled_speaker_profile_ids = value;
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
    let summary_template_id: String = row.get("summary_template_id")?;
    let translation_language: String = row.get("translation_language")?;
    let polish_preset_id: String = row.get("polish_preset_id")?;
    let polish_scenario: Option<String> = row.get("polish_scenario")?;
    let polish_context: Option<String> = row.get("polish_context")?;
    let export_file_name_prefix: String = row.get("export_file_name_prefix")?;

    let defaults = TagDefaults {
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

    Ok(TagRecord {
        id,
        name,
        description,
        icon,
        color,
        sort_order: checked_usize_column(row, "sort_order", sort_order)?,
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

fn replace_tag_default_links(
    tx: &rusqlite::Transaction,
    tag_id: &str,
    defaults: &TagDefaults,
) -> Result<(), rusqlite::Error> {
    tx.execute("DELETE FROM tag_default_links WHERE tag_id = ?1", [tag_id])?;
    insert_tag_links_for_kind(
        tx,
        tag_id,
        LINK_KIND_TEXT_REPLACEMENT,
        &defaults.enabled_text_replacement_set_ids,
    )?;
    insert_tag_links_for_kind(
        tx,
        tag_id,
        LINK_KIND_HOTWORD,
        &defaults.enabled_hotword_set_ids,
    )?;
    insert_tag_links_for_kind(
        tx,
        tag_id,
        LINK_KIND_POLISH_KEYWORD,
        &defaults.enabled_polish_keyword_set_ids,
    )?;
    insert_tag_links_for_kind(
        tx,
        tag_id,
        LINK_KIND_SPEAKER_PROFILE,
        &defaults.enabled_speaker_profile_ids,
    )
}

fn replace_tag_links_for_kind(
    tx: &rusqlite::Transaction,
    tag_id: &str,
    kind: &str,
    target_ids: &[String],
) -> Result<(), rusqlite::Error> {
    tx.execute(
        "DELETE FROM tag_default_links WHERE tag_id = ?1 AND kind = ?2",
        rusqlite::params![tag_id, kind],
    )?;
    insert_tag_links_for_kind(tx, tag_id, kind, target_ids)
}

fn insert_tag_links_for_kind(
    tx: &rusqlite::Transaction,
    tag_id: &str,
    kind: &str,
    target_ids: &[String],
) -> Result<(), rusqlite::Error> {
    let mut stmt = tx.prepare_cached(
        "INSERT OR REPLACE INTO tag_default_links (tag_id, kind, target_id, sort_order)
         VALUES (?1, ?2, ?3, ?4)",
    )?;
    for (sort_order, target_id) in target_ids.iter().enumerate() {
        stmt.execute(rusqlite::params![
            tag_id,
            kind,
            target_id,
            sort_order as i64
        ])?;
    }
    Ok(())
}

fn hydrate_tag_default_links(
    conn: &rusqlite::Connection,
    tag: &mut TagRecord,
) -> Result<(), DatabaseError> {
    let mut stmt = conn.prepare_cached(
        "SELECT kind, target_id FROM tag_default_links
         WHERE tag_id = ?1
         ORDER BY kind, sort_order, target_id",
    )?;
    let mut rows = stmt.query([tag.id.as_str()])?;
    while let Some(row) = rows.next()? {
        let kind: String = row.get(0)?;
        let target_id: String = row.get(1)?;
        match kind.as_str() {
            LINK_KIND_TEXT_REPLACEMENT => tag
                .defaults
                .enabled_text_replacement_set_ids
                .push(target_id),
            LINK_KIND_HOTWORD => tag.defaults.enabled_hotword_set_ids.push(target_id),
            LINK_KIND_POLISH_KEYWORD => tag.defaults.enabled_polish_keyword_set_ids.push(target_id),
            LINK_KIND_SPEAKER_PROFILE => tag.defaults.enabled_speaker_profile_ids.push(target_id),
            _ => {}
        }
    }
    Ok(())
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
