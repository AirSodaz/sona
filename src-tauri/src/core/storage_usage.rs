use crate::core::database::{Database, DatabaseError};
use rusqlite::Connection;
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const HISTORY_DIR_NAME: &str = "history";
const SPEAKER_PROFILES_DIR_NAME: &str = "speaker-profiles";
const MODELS_DIR_NAME: &str = "models";
const API_TEMP_DIR_NAME: &str = "api_temp";
#[cfg(target_os = "windows")]
const WINDOWS_WEBVIEW_DIR_NAME: &str = "EBWebView";

const MAIN_DB_FILE_NAME: &str = "sona.db";
const MAIN_WAL_FILE_NAME: &str = "sona.db-wal";
const MAIN_SHM_FILE_NAME: &str = "sona.db-shm";
const ANALYTICS_DB_FILE_NAME: &str = "sona-analytics.db";
const ANALYTICS_WAL_FILE_NAME: &str = "sona-analytics.db-wal";
const ANALYTICS_SHM_FILE_NAME: &str = "sona-analytics.db-shm";

const DATABASE_FILE_NAMES: [&str; 6] = [
    MAIN_DB_FILE_NAME,
    MAIN_WAL_FILE_NAME,
    MAIN_SHM_FILE_NAME,
    ANALYTICS_DB_FILE_NAME,
    ANALYTICS_WAL_FILE_NAME,
    ANALYTICS_SHM_FILE_NAME,
];

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageUsageSnapshot {
    pub generated_at: String,
    pub total_bytes: u64,
    pub categories: StorageUsageCategories,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageUsageCategories {
    pub audio: AudioUsageCategory,
    pub database: DatabaseUsageCategory,
    pub models: FileUsageCategory,
    pub temporary: FileUsageCategory,
    pub webview_cache: WebviewCacheUsageCategory,
    pub other: FileUsageCategory,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioUsageCategory {
    pub bytes: u64,
    pub history_audio_bytes: u64,
    pub speaker_sample_bytes: u64,
    pub file_count: u64,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseUsageCategory {
    pub bytes: u64,
    pub sqlite: SQLiteUsageSummary,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileUsageCategory {
    pub bytes: u64,
    pub file_count: u64,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebviewCacheUsageCategory {
    pub bytes: Option<u64>,
    pub clear_supported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SQLiteUsageSummary {
    pub main_db_bytes: u64,
    pub main_wal_bytes: u64,
    pub main_shm_bytes: u64,
    pub analytics_db_bytes: u64,
    pub analytics_wal_bytes: u64,
    pub analytics_shm_bytes: u64,
    pub data_bytes: u64,
    pub index_bytes: u64,
    pub free_page_bytes: u64,
    pub index_entries: Vec<SQLiteIndexUsageEntry>,
    pub dbstat_available: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SQLiteIndexUsageEntry {
    pub schema: String,
    pub name: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebviewBrowsingDataClearResult {
    pub before_bytes: Option<u64>,
    pub after_bytes: Option<u64>,
    pub clear_requested: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct FileUsage {
    bytes: u64,
    file_count: u64,
}

impl From<FileUsage> for FileUsageCategory {
    fn from(value: FileUsage) -> Self {
        Self {
            bytes: value.bytes,
            file_count: value.file_count,
        }
    }
}

pub fn collect_storage_usage_snapshot(
    app_local_data_dir: &Path,
    db: &Database,
) -> Result<StorageUsageSnapshot, String> {
    collect_storage_usage_snapshot_with_webview_path(
        app_local_data_dir,
        db,
        default_webview_cache_path(app_local_data_dir),
    )
}

pub(crate) fn collect_storage_usage_snapshot_with_webview_path(
    app_local_data_dir: &Path,
    db: &Database,
    webview_cache_path: Option<PathBuf>,
) -> Result<StorageUsageSnapshot, String> {
    let history_dir = app_local_data_dir.join(HISTORY_DIR_NAME);
    let speaker_profiles_dir = app_local_data_dir.join(SPEAKER_PROFILES_DIR_NAME);
    let models_dir = app_local_data_dir.join(MODELS_DIR_NAME);
    let temporary_dir = app_local_data_dir.join(API_TEMP_DIR_NAME);

    let history_audio = scan_existing_path(&history_dir)?;
    let speaker_samples = scan_existing_path(&speaker_profiles_dir)?;
    let models = scan_existing_path(&models_dir)?;
    let temporary = scan_existing_path(&temporary_dir)?;
    let webview_cache = scan_optional_path(webview_cache_path.as_deref())?;

    let mut sqlite = collect_sqlite_usage_summary(db)?;
    sqlite.main_db_bytes = file_size(app_local_data_dir.join(MAIN_DB_FILE_NAME))?;
    sqlite.main_wal_bytes = file_size(app_local_data_dir.join(MAIN_WAL_FILE_NAME))?;
    sqlite.main_shm_bytes = file_size(app_local_data_dir.join(MAIN_SHM_FILE_NAME))?;
    sqlite.analytics_db_bytes = file_size(app_local_data_dir.join(ANALYTICS_DB_FILE_NAME))?;
    sqlite.analytics_wal_bytes = file_size(app_local_data_dir.join(ANALYTICS_WAL_FILE_NAME))?;
    sqlite.analytics_shm_bytes = file_size(app_local_data_dir.join(ANALYTICS_SHM_FILE_NAME))?;

    let database_bytes = sqlite.main_db_bytes
        + sqlite.main_wal_bytes
        + sqlite.main_shm_bytes
        + sqlite.analytics_db_bytes
        + sqlite.analytics_wal_bytes
        + sqlite.analytics_shm_bytes;

    let mut excluded_paths = vec![history_dir, speaker_profiles_dir, models_dir, temporary_dir];
    if let Some(path) = webview_cache_path.as_ref() {
        excluded_paths.push(path.clone());
    }
    for file_name in DATABASE_FILE_NAMES {
        excluded_paths.push(app_local_data_dir.join(file_name));
    }

    let other = scan_dir_excluding(app_local_data_dir, &excluded_paths)?;

    let audio = AudioUsageCategory {
        bytes: history_audio.bytes + speaker_samples.bytes,
        history_audio_bytes: history_audio.bytes,
        speaker_sample_bytes: speaker_samples.bytes,
        file_count: history_audio.file_count + speaker_samples.file_count,
    };
    let database = DatabaseUsageCategory {
        bytes: database_bytes,
        sqlite,
    };
    let webview_cache_bytes = webview_cache.map(|usage| usage.bytes);
    let webview_cache = WebviewCacheUsageCategory {
        bytes: webview_cache_bytes,
        clear_supported: true,
        path: webview_cache_path
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned()),
    };

    let categories = StorageUsageCategories {
        audio,
        database,
        models: models.into(),
        temporary: temporary.into(),
        webview_cache,
        other: other.into(),
    };

    let total_bytes = categories.audio.bytes
        + categories.database.bytes
        + categories.models.bytes
        + categories.temporary.bytes
        + categories.webview_cache.bytes.unwrap_or(0)
        + categories.other.bytes;

    Ok(StorageUsageSnapshot {
        generated_at: chrono::Utc::now().to_rfc3339(),
        total_bytes,
        categories,
    })
}

pub fn default_webview_cache_path(app_local_data_dir: &Path) -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        Some(app_local_data_dir.join(WINDOWS_WEBVIEW_DIR_NAME))
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = app_local_data_dir;
        None
    }
}

pub fn observable_webview_cache_bytes(app_local_data_dir: &Path) -> Result<Option<u64>, String> {
    let Some(path) = default_webview_cache_path(app_local_data_dir) else {
        return Ok(None);
    };
    Ok(Some(scan_existing_path(&path)?.bytes))
}

pub fn build_webview_clear_result(
    before_bytes: Option<u64>,
    after_bytes: Option<u64>,
) -> WebviewBrowsingDataClearResult {
    WebviewBrowsingDataClearResult {
        before_bytes,
        after_bytes,
        clear_requested: true,
    }
}

pub fn collect_sqlite_usage_summary(db: &Database) -> Result<SQLiteUsageSummary, String> {
    db.with_connection(|conn| collect_sqlite_usage_summary_inner(conn))
        .map_err(|error| error.to_string())
}

fn collect_sqlite_usage_summary_inner(
    conn: &Connection,
) -> Result<SQLiteUsageSummary, DatabaseError> {
    ensure_dbstat_available(conn)?;
    let index_like_objects = collect_index_like_object_names(conn)?;
    let mut index_entries_by_object: BTreeMap<(String, String), u64> = BTreeMap::new();
    let mut data_bytes = 0_u64;
    let mut index_bytes = 0_u64;

    let mut stmt = conn
        .prepare_cached(
            "SELECT schema, name, pagetype, COALESCE(SUM(pgsize), 0), COALESCE(SUM(unused), 0)
         FROM dbstat
         WHERE aggregate = false
         GROUP BY schema, name, pagetype",
        )
        .map_err(dbstat_unavailable)?;

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })
        .map_err(dbstat_unavailable)?;

    for row in rows {
        let (schema, name, _page_type, page_bytes, _unused_bytes) =
            row.map_err(dbstat_unavailable)?;
        let page_bytes = page_bytes.max(0) as u64;
        if index_like_objects.contains(&(schema.clone(), name.clone())) {
            index_bytes += page_bytes;
            *index_entries_by_object.entry((schema, name)).or_default() += page_bytes;
        } else {
            data_bytes += page_bytes;
        }
    }

    let index_entries = index_entries_by_object
        .into_iter()
        .map(|((schema, name), bytes)| SQLiteIndexUsageEntry {
            schema,
            name,
            bytes,
        })
        .collect();

    Ok(SQLiteUsageSummary {
        data_bytes,
        index_bytes,
        free_page_bytes: free_page_bytes(conn)?,
        index_entries,
        dbstat_available: true,
        ..Default::default()
    })
}

fn ensure_dbstat_available(conn: &Connection) -> Result<(), DatabaseError> {
    conn.query_row(
        "SELECT COUNT(*) FROM dbstat WHERE aggregate = false",
        [],
        |_row| Ok(()),
    )
    .map_err(dbstat_unavailable)
}

fn dbstat_unavailable(error: rusqlite::Error) -> DatabaseError {
    DatabaseError::Internal(format!("SQLite dbstat capability is unavailable: {error}"))
}

fn collect_index_like_object_names(
    conn: &Connection,
) -> Result<HashSet<(String, String)>, DatabaseError> {
    let mut objects = HashSet::new();
    for schema in ["main", "analytics"] {
        let schema_table = match schema {
            "main" => "sqlite_schema",
            "analytics" => "analytics.sqlite_schema",
            _ => unreachable!(),
        };
        let mut fts_roots = Vec::new();
        let mut stmt = conn
            .prepare(&format!(
                "SELECT type, name, COALESCE(sql, '') FROM {schema_table}
                 WHERE type IN ('index', 'table')"
            ))
            .map_err(DatabaseError::QueryError)?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(DatabaseError::QueryError)?;

        for row in rows {
            let (kind, name, sql) = row.map_err(DatabaseError::QueryError)?;
            if kind == "index" {
                objects.insert((schema.to_string(), name.clone()));
            }
            if kind == "table" && sql.to_ascii_lowercase().contains(" using fts5") {
                fts_roots.push(name.clone());
                objects.insert((schema.to_string(), name));
            }
        }

        for root in fts_roots {
            let prefix = format!("{root}_");
            let mut shadow_stmt = conn
                .prepare(&format!(
                    "SELECT name FROM {schema_table}
                     WHERE type = 'table'"
                ))
                .map_err(DatabaseError::QueryError)?;
            let shadow_rows = shadow_stmt
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(DatabaseError::QueryError)?;
            for shadow in shadow_rows {
                let shadow = shadow.map_err(DatabaseError::QueryError)?;
                if shadow.starts_with(&prefix) {
                    objects.insert((schema.to_string(), shadow));
                }
            }
        }
    }
    Ok(objects)
}

fn free_page_bytes(conn: &Connection) -> Result<u64, DatabaseError> {
    let mut total = 0_u64;
    for schema in ["main", "analytics"] {
        let page_size: i64 = conn
            .query_row(&format!("PRAGMA {schema}.page_size"), [], |row| row.get(0))
            .map_err(DatabaseError::QueryError)?;
        let freelist_count: i64 = conn
            .query_row(&format!("PRAGMA {schema}.freelist_count"), [], |row| {
                row.get(0)
            })
            .map_err(DatabaseError::QueryError)?;
        total += page_size.max(0) as u64 * freelist_count.max(0) as u64;
    }
    Ok(total)
}

fn file_size(path: PathBuf) -> Result<u64, String> {
    match fs::metadata(&path) {
        Ok(metadata) if metadata.is_file() => Ok(metadata.len()),
        Ok(_) => Ok(0),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(0),
        Err(error) => Err(format!("Failed to inspect {}: {error}", path.display())),
    }
}

fn scan_optional_path(path: Option<&Path>) -> Result<Option<FileUsage>, String> {
    path.map(scan_existing_path).transpose()
}

fn scan_existing_path(path: &Path) -> Result<FileUsage, String> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(FileUsage {
            bytes: metadata.len(),
            file_count: 1,
        }),
        Ok(metadata) if metadata.is_dir() => scan_dir(path),
        Ok(_) => Ok(FileUsage::default()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(FileUsage::default()),
        Err(error) => Err(format!("Failed to inspect {}: {error}", path.display())),
    }
}

fn scan_dir(path: &Path) -> Result<FileUsage, String> {
    let mut usage = FileUsage::default();
    for entry in WalkDir::new(path).into_iter().skip(1) {
        let entry = entry.map_err(|error| format!("Failed to scan {}: {error}", path.display()))?;
        let metadata = entry
            .metadata()
            .map_err(|error| format!("Failed to inspect {}: {error}", entry.path().display()))?;
        if metadata.is_file() {
            usage.bytes += metadata.len();
            usage.file_count += 1;
        }
    }
    Ok(usage)
}

fn scan_dir_excluding(root: &Path, excluded_paths: &[PathBuf]) -> Result<FileUsage, String> {
    if !root.exists() {
        return Ok(FileUsage::default());
    }

    let mut usage = FileUsage::default();
    let walker = WalkDir::new(root).into_iter().filter_entry(|entry| {
        entry.depth() == 0 || !is_excluded_path(entry.path(), excluded_paths)
    });
    for entry in walker.skip(1) {
        let entry = entry.map_err(|error| format!("Failed to scan {}: {error}", root.display()))?;
        if is_excluded_path(entry.path(), excluded_paths) {
            continue;
        }
        let metadata = entry
            .metadata()
            .map_err(|error| format!("Failed to inspect {}: {error}", entry.path().display()))?;
        if metadata.is_file() {
            usage.bytes += metadata.len();
            usage.file_count += 1;
        }
    }
    Ok(usage)
}

fn is_excluded_path(path: &Path, excluded_paths: &[PathBuf]) -> bool {
    excluded_paths
        .iter()
        .any(|excluded| path == excluded || path.starts_with(excluded))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::database::Database;
    use std::fs;
    use tempfile::tempdir;

    fn write_sized_file(path: &Path, size: usize) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, vec![7_u8; size]).unwrap();
    }

    #[test]
    fn bundled_sqlite_exposes_dbstat_compile_option() {
        let db = Database::open_in_memory().unwrap();
        db.with_connection(|conn| {
            let enabled: bool = conn.query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM pragma_compile_options
                    WHERE compile_options = 'ENABLE_DBSTAT_VTAB'
                 )",
                [],
                |row| row.get(0),
            )?;
            assert!(enabled, "bundled SQLite must expose ENABLE_DBSTAT_VTAB");
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn dbstat_reports_non_zero_index_bytes() {
        let db = Database::open_in_memory().unwrap();
        db.with_connection(|conn| {
            conn.execute_batch(
                "CREATE TABLE dbstat_index_probe (id INTEGER PRIMARY KEY, value TEXT NOT NULL);
                 CREATE INDEX idx_dbstat_index_probe_value ON dbstat_index_probe(value);",
            )?;
            let tx = conn.unchecked_transaction()?;
            for index in 0..500 {
                tx.execute(
                    "INSERT INTO dbstat_index_probe (value) VALUES (?1)",
                    [format!("value-{index:04}")],
                )?;
            }
            tx.commit()?;
            Ok(())
        })
        .unwrap();

        let summary = collect_sqlite_usage_summary(&db).unwrap();
        assert!(summary.dbstat_available);
        assert!(
            summary.index_bytes > 0,
            "dbstat should attribute bytes to SQLite indexes"
        );
        assert!(
            summary
                .index_entries
                .iter()
                .any(|entry| entry.name == "idx_dbstat_index_probe_value" && entry.bytes > 0)
        );
    }

    #[test]
    fn storage_snapshot_classifies_known_roots_and_excludes_webview_from_other() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let db = Database::open(root).unwrap();

        write_sized_file(&root.join("history").join("recording.wav"), 100);
        write_sized_file(
            &root
                .join("speaker-profiles")
                .join("speaker-1")
                .join("sample.wav"),
            50,
        );
        write_sized_file(&root.join("models").join("model.bin"), 30);
        write_sized_file(&root.join("api_temp").join("job.tmp"), 20);
        write_sized_file(&root.join("EBWebView").join("Cache").join("entry.bin"), 10);
        write_sized_file(&root.join("settings-sidecar.json"), 7);

        let snapshot = collect_storage_usage_snapshot_with_webview_path(
            root,
            &db,
            Some(root.join("EBWebView")),
        )
        .unwrap();

        assert_eq!(snapshot.categories.audio.history_audio_bytes, 100);
        assert_eq!(snapshot.categories.audio.speaker_sample_bytes, 50);
        assert_eq!(snapshot.categories.audio.bytes, 150);
        assert_eq!(snapshot.categories.audio.file_count, 2);
        assert_eq!(snapshot.categories.models.bytes, 30);
        assert_eq!(snapshot.categories.models.file_count, 1);
        assert_eq!(snapshot.categories.temporary.bytes, 20);
        assert_eq!(snapshot.categories.webview_cache.bytes, Some(10));
        assert_eq!(snapshot.categories.other.bytes, 7);
        assert_eq!(snapshot.categories.other.file_count, 1);
        assert!(snapshot.categories.database.bytes > 0);
        assert!(snapshot.total_bytes >= 150 + 30 + 20 + 10 + 7);
    }

    #[test]
    fn webview_clear_result_shape_sets_clear_requested() {
        let result = build_webview_clear_result(Some(128), Some(64));

        assert_eq!(result.before_bytes, Some(128));
        assert_eq!(result.after_bytes, Some(64));
        assert!(result.clear_requested);
    }
}
