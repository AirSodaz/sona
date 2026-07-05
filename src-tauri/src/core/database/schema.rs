use super::{Database, DatabaseError};

const CURRENT_SCHEMA_VERSION: i64 = 1;
type MigrationFn = fn(&rusqlite::Transaction) -> Result<(), rusqlite::Error>;

const MIGRATIONS: &[(i64, &str, MigrationFn)] =
    &[(1, "Initial complete SQLite schema", migrate_v1)];

/// Runs pending schema migrations in version order.
///
/// Sona's public SQLite baseline starts after the v0.7.4 JSON storage era, so
/// historical in-development SQLite migrations are intentionally squashed into
/// v1. Future schema changes must be appended as v2, v3, etc.
pub fn run_migrations(db: &Database) -> Result<(), DatabaseError> {
    db.with_transaction(|tx| {
        bootstrap_schema_version(tx)?;
        let applied_version = current_applied_schema_version(tx)?;

        if applied_version > CURRENT_SCHEMA_VERSION {
            return Err(DatabaseError::UnsupportedSchemaVersion {
                found: applied_version,
                current: CURRENT_SCHEMA_VERSION,
            });
        }

        for &(version, _description, migration) in MIGRATIONS {
            if version > applied_version {
                migration(tx)?;
                tx.execute(
                    "INSERT INTO schema_version (version) VALUES (?1)",
                    [version],
                )?;
            }
        }
        Ok(())
    })
}

fn bootstrap_schema_version(tx: &rusqlite::Transaction) -> Result<(), rusqlite::Error> {
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )
}

fn current_applied_schema_version(tx: &rusqlite::Transaction) -> Result<i64, rusqlite::Error> {
    tx.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_version",
        [],
        |row| row.get(0),
    )
}

// ---------------------------------------------------------------------------
// v1: Current schema — all SQLite-backed data domains
// ---------------------------------------------------------------------------
fn migrate_v1(tx: &rusqlite::Transaction) -> Result<(), rusqlite::Error> {
    tx.execute_batch(
        "
        -- Projects (replaces projects/index.json)
        CREATE TABLE projects (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL DEFAULT '',
            description TEXT NOT NULL DEFAULT '',
            icon TEXT,
            color TEXT,
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            summary_template_id TEXT DEFAULT 'general',
            translation_language TEXT DEFAULT 'zh',
            polish_preset_id TEXT DEFAULT 'general',
            polish_scenario TEXT,
            polish_context TEXT,
            export_file_name_prefix TEXT NOT NULL DEFAULT ''
        );

        CREATE TABLE project_default_links (
            project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            kind TEXT NOT NULL,
            target_id TEXT NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (project_id, kind, target_id)
        );

        CREATE INDEX idx_project_default_links_kind_target
            ON project_default_links(kind, target_id);

        -- History Items (replaces history/index.json)
        CREATE TABLE history_items (
            id TEXT PRIMARY KEY,
            timestamp INTEGER NOT NULL,
            duration REAL NOT NULL DEFAULT 0.0,
            audio_path TEXT NOT NULL DEFAULT '',
            audio_status TEXT NOT NULL DEFAULT 'available',
            transcript_path TEXT NOT NULL DEFAULT '',
            title TEXT NOT NULL DEFAULT '',
            preview_text TEXT NOT NULL DEFAULT '',
            icon TEXT,
            kind TEXT NOT NULL DEFAULT 'recording',
            search_content TEXT NOT NULL DEFAULT '',
            project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
            status TEXT NOT NULL DEFAULT 'complete',
            draft_source TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE INDEX idx_history_items_timestamp ON history_items(timestamp);
        CREATE INDEX idx_history_items_project_id ON history_items(project_id);
        CREATE INDEX idx_history_items_project_timestamp ON history_items(project_id, timestamp DESC);
        CREATE INDEX idx_history_items_kind ON history_items(kind);
        CREATE INDEX idx_history_items_status ON history_items(status);

        -- Transcripts (replaces history/{id}.json files)
        CREATE TABLE history_transcripts (
            history_id TEXT PRIMARY KEY REFERENCES history_items(id) ON DELETE CASCADE,
            segments TEXT NOT NULL DEFAULT '[]'
        );

        -- Summaries (replaces history/{id}.summary.json files)
        CREATE TABLE history_summaries (
            history_id TEXT PRIMARY KEY REFERENCES history_items(id) ON DELETE CASCADE,
            payload TEXT NOT NULL DEFAULT '{}'
        );

        -- Transcript Snapshots (replaces history/versions/{id}/*.json)
        CREATE TABLE transcript_snapshots (
            id TEXT NOT NULL,
            history_id TEXT NOT NULL REFERENCES history_items(id) ON DELETE CASCADE,
            reason TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            segment_count INTEGER NOT NULL DEFAULT 0,
            segments TEXT NOT NULL DEFAULT '[]',
            PRIMARY KEY (history_id, id)
        );

        CREATE INDEX idx_snapshots_history_id ON transcript_snapshots(history_id);

        -- App Settings (replaces settings.json KV pairs except sona-config)
        CREATE TABLE app_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        -- App Config (replaces settings.json sona-config)
        CREATE TABLE app_config (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            config TEXT NOT NULL DEFAULT '{}',
            config_version INTEGER NOT NULL DEFAULT 7,
            updated_at INTEGER NOT NULL DEFAULT 0,
            http_server_enabled INTEGER NOT NULL DEFAULT 0,
            http_server_host TEXT NOT NULL DEFAULT '127.0.0.1',
            http_server_port INTEGER NOT NULL DEFAULT 14200,
            http_server_api_key TEXT NOT NULL DEFAULT '',
            http_server_max_concurrent INTEGER NOT NULL DEFAULT 2,
            http_server_max_queue_size INTEGER NOT NULL DEFAULT 100,
            http_server_max_upload_size_mb INTEGER NOT NULL DEFAULT 50,
            http_server_job_ttl_minutes INTEGER NOT NULL DEFAULT 60,
            http_server_max_streaming INTEGER NOT NULL DEFAULT 2,
            http_server_ip_whitelist TEXT NOT NULL DEFAULT 'localhost',
            gpu_acceleration TEXT NOT NULL DEFAULT 'auto'
        );

        CREATE TABLE summary_templates (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL DEFAULT '',
            instructions TEXT NOT NULL DEFAULT '',
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL DEFAULT 0,
            updated_at INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE polish_presets (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL DEFAULT '',
            context TEXT NOT NULL DEFAULT '',
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL DEFAULT 0,
            updated_at INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE vocabulary_sets (
            id TEXT NOT NULL,
            kind TEXT NOT NULL CHECK(kind IN ('text_replacement', 'hotword', 'polish_keyword')),
            name TEXT NOT NULL DEFAULT '',
            enabled INTEGER NOT NULL DEFAULT 1,
            ignore_case INTEGER NOT NULL DEFAULT 0,
            keywords TEXT NOT NULL DEFAULT '',
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL DEFAULT 0,
            updated_at INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (kind, id)
        );

        CREATE TABLE vocabulary_rules (
            id TEXT NOT NULL,
            set_kind TEXT NOT NULL,
            set_id TEXT NOT NULL,
            from_text TEXT NOT NULL DEFAULT '',
            to_text TEXT NOT NULL DEFAULT '',
            text TEXT NOT NULL DEFAULT '',
            sort_order INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (set_kind, set_id, id),
            FOREIGN KEY (set_kind, set_id)
                REFERENCES vocabulary_sets(kind, id)
                ON DELETE CASCADE
        );

        CREATE TABLE speaker_profiles (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL DEFAULT '',
            enabled INTEGER NOT NULL DEFAULT 1,
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL DEFAULT 0,
            updated_at INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE speaker_profile_samples (
            id TEXT NOT NULL,
            profile_id TEXT NOT NULL REFERENCES speaker_profiles(id) ON DELETE CASCADE,
            file_path TEXT NOT NULL DEFAULT '',
            source_name TEXT NOT NULL DEFAULT '',
            duration_seconds REAL NOT NULL DEFAULT 0.0,
            sort_order INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (profile_id, id)
        );

        -- Automation Rules (replaces automation/rules.json)
        CREATE TABLE automation_rules (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL DEFAULT '',
            project_id TEXT NOT NULL DEFAULT '',
            preset_id TEXT NOT NULL DEFAULT 'custom',
            watch_directory TEXT NOT NULL DEFAULT '',
            recursive INTEGER NOT NULL DEFAULT 0,
            enabled INTEGER NOT NULL DEFAULT 0,
            stage_auto_polish INTEGER NOT NULL DEFAULT 0,
            stage_polish_preset_id TEXT NOT NULL DEFAULT 'general',
            stage_auto_translate INTEGER NOT NULL DEFAULT 0,
            stage_translation_language TEXT NOT NULL DEFAULT 'en',
            stage_export_enabled INTEGER NOT NULL DEFAULT 0,
            export_directory TEXT NOT NULL DEFAULT '',
            export_format TEXT NOT NULL DEFAULT 'txt',
            export_mode TEXT NOT NULL DEFAULT 'original',
            export_prefix TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL DEFAULT 0,
            updated_at INTEGER NOT NULL DEFAULT 0
        );

        -- Automation Processed Entries (replaces automation/processed.json)
        CREATE TABLE automation_processed (
            id TEXT PRIMARY KEY,
            rule_id TEXT NOT NULL DEFAULT '',
            file_path TEXT NOT NULL DEFAULT '',
            source_fingerprint TEXT NOT NULL DEFAULT '',
            size INTEGER NOT NULL DEFAULT 0,
            mtime_ms INTEGER NOT NULL DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'complete',
            processed_at INTEGER NOT NULL DEFAULT 0,
            history_id TEXT,
            export_path TEXT,
            error_message TEXT
        );
        CREATE INDEX idx_automation_processed_rule_id ON automation_processed(rule_id);
        CREATE INDEX idx_automation_processed_file_path ON automation_processed(file_path);

        -- Task Ledger (replaces task-ledger/ledger.json)
        CREATE TABLE task_ledger (
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL DEFAULT '',
            title TEXT NOT NULL DEFAULT '',
            progress REAL NOT NULL DEFAULT 0.0,
            created_at INTEGER NOT NULL DEFAULT 0,
            updated_at INTEGER NOT NULL DEFAULT 0,
            retryable INTEGER NOT NULL DEFAULT 0,
            cancelable INTEGER NOT NULL DEFAULT 0,
            recoverable INTEGER NOT NULL DEFAULT 0,
            stage TEXT,
            history_id TEXT,
            project_id TEXT,
            file_path TEXT,
            automation_rule_id TEXT,
            source_fingerprint TEXT,
            error_message TEXT,
            template_id TEXT,
            target_language TEXT,
            version INTEGER NOT NULL DEFAULT 1
        );
        CREATE INDEX idx_task_ledger_status ON task_ledger(status);
        CREATE INDEX idx_task_ledger_updated_at ON task_ledger(updated_at);

        CREATE TABLE analytics.llm_usage (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            occurred_at TEXT NOT NULL,
            provider TEXT NOT NULL DEFAULT '',
            category TEXT NOT NULL DEFAULT '',
            prompt_tokens INTEGER NOT NULL DEFAULT 0,
            completion_tokens INTEGER NOT NULL DEFAULT 0,
            total_tokens INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX analytics.idx_llm_usage_occurred_at ON llm_usage(occurred_at);

        CREATE VIRTUAL TABLE history_items_fts USING fts5(
            title,
            preview_text,
            search_content,
            content='history_items',
            content_rowid='rowid',
            tokenize='trigram'
        );

        CREATE TRIGGER tbl_history_items_ai AFTER INSERT ON history_items BEGIN
          INSERT INTO history_items_fts(rowid, title, preview_text, search_content)
          VALUES (new.rowid, new.title, new.preview_text, new.search_content);
        END;

        CREATE TRIGGER tbl_history_items_ad AFTER DELETE ON history_items BEGIN
          INSERT INTO history_items_fts(history_items_fts, rowid, title, preview_text, search_content)
          VALUES ('delete', old.rowid, old.title, old.preview_text, old.search_content);
        END;

        CREATE TRIGGER tbl_history_items_au AFTER UPDATE OF title, preview_text, search_content ON history_items BEGIN
          INSERT INTO history_items_fts(history_items_fts, rowid, title, preview_text, search_content)
          VALUES ('delete', old.rowid, old.title, old.preview_text, old.search_content);
          INSERT INTO history_items_fts(rowid, title, preview_text, search_content)
          VALUES (new.rowid, new.title, new.preview_text, new.search_content);
        END;"
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::database::Database;

    fn schema_versions(db: &Database) -> Vec<i64> {
        db.with_connection(|conn| {
            let mut stmt = conn.prepare("SELECT version FROM schema_version ORDER BY version")?;
            stmt.query_map([], |row| row.get::<_, i64>(0))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(DatabaseError::QueryError)
        })
        .unwrap()
    }

    fn table_columns(conn: &rusqlite::Connection, table: &str) -> Vec<String> {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .unwrap();
        let rows = stmt.query_map([], |row| row.get::<_, String>(1)).unwrap();
        rows.collect::<Result<Vec<_>, _>>().unwrap()
    }

    fn has_composite_primary_key(
        conn: &rusqlite::Connection,
        table: &str,
        columns: &[&str],
    ) -> bool {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .unwrap();
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(1)?, row.get::<_, i64>(5)?))
            })
            .unwrap();
        let mut keyed_columns = rows
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .into_iter()
            .filter(|(_, pk_order)| *pk_order > 0)
            .collect::<Vec<_>>();
        keyed_columns.sort_by_key(|(_, pk_order)| *pk_order);
        keyed_columns
            .into_iter()
            .map(|(column, _)| column)
            .collect::<Vec<_>>()
            == columns
    }

    fn foreign_key_cascades_to(
        conn: &rusqlite::Connection,
        table: &str,
        referenced_table: &str,
    ) -> bool {
        let mut stmt = conn
            .prepare(&format!("PRAGMA foreign_key_list({table})"))
            .unwrap();
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(2)?, row.get::<_, String>(6)?))
            })
            .unwrap();
        rows.collect::<Result<Vec<_>, _>>()
            .unwrap()
            .into_iter()
            .any(|(table, on_delete)| table == referenced_table && on_delete == "CASCADE")
    }

    #[test]
    fn test_migrations_are_idempotent() {
        let db = Database::open_in_memory().unwrap();
        // Migrations already ran during open_in_memory. Running again should be a no-op.
        run_migrations(&db).unwrap();

        assert_eq!(schema_versions(&db), vec![1]);
    }

    #[test]
    fn test_future_schema_version_is_rejected() {
        let db = Database::open_in_memory().unwrap();
        db.with_connection(|conn| {
            conn.execute("INSERT INTO schema_version (version) VALUES (2)", [])?;
            Ok(())
        })
        .unwrap();

        let err = run_migrations(&db).unwrap_err();
        assert!(matches!(
            err,
            DatabaseError::UnsupportedSchemaVersion {
                found: 2,
                current: 1
            }
        ));
        assert_eq!(schema_versions(&db), vec![1, 2]);
    }

    #[test]
    fn test_lower_schema_version_records_are_preserved() {
        let db = Database::open_in_memory().unwrap();
        db.with_connection(|conn| {
            conn.execute("INSERT INTO schema_version (version) VALUES (0)", [])?;
            Ok(())
        })
        .unwrap();

        run_migrations(&db).unwrap();

        assert_eq!(schema_versions(&db), vec![0, 1]);
    }

    #[test]
    fn test_all_tables_created() {
        let db = Database::open_in_memory().unwrap();
        let expected_tables = [
            "schema_version",
            "history_items",
            "history_transcripts",
            "history_summaries",
            "transcript_snapshots",
            "projects",
            "project_default_links",
            "app_settings",
            "app_config",
            "summary_templates",
            "polish_presets",
            "vocabulary_sets",
            "vocabulary_rules",
            "speaker_profiles",
            "speaker_profile_samples",
            "automation_rules",
            "automation_processed",
            "task_ledger",
            "history_items_fts",
        ];

        db.with_connection(|conn| {
            for table in &expected_tables {
                let count: i64 =
                    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                        row.get(0)
                    })?;
                assert!(count >= 0, "Table {table} should exist");
            }
            // Verify analytics.llm_usage table exists in attached database
            let count: i64 =
                conn.query_row("SELECT COUNT(*) FROM analytics.llm_usage", [], |row| {
                    row.get(0)
                })?;
            assert_eq!(count, 0);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_structured_tables_do_not_keep_file_era_json_blobs() {
        let db = Database::open_in_memory().unwrap();
        db.with_connection(|conn| {
            assert_eq!(
                table_columns(conn, "projects"),
                vec![
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
                ]
            );
            assert_eq!(
                table_columns(conn, "project_default_links"),
                vec!["project_id", "kind", "target_id", "sort_order"]
            );
            assert_eq!(
                table_columns(conn, "automation_rules"),
                vec![
                    "id",
                    "name",
                    "project_id",
                    "preset_id",
                    "watch_directory",
                    "recursive",
                    "enabled",
                    "stage_auto_polish",
                    "stage_polish_preset_id",
                    "stage_auto_translate",
                    "stage_translation_language",
                    "stage_export_enabled",
                    "export_directory",
                    "export_format",
                    "export_mode",
                    "export_prefix",
                    "created_at",
                    "updated_at",
                ]
            );
            assert_eq!(
                table_columns(conn, "automation_processed"),
                vec![
                    "id",
                    "rule_id",
                    "file_path",
                    "source_fingerprint",
                    "size",
                    "mtime_ms",
                    "status",
                    "processed_at",
                    "history_id",
                    "export_path",
                    "error_message",
                ]
            );
            assert_eq!(
                table_columns(conn, "task_ledger"),
                vec![
                    "id",
                    "kind",
                    "status",
                    "title",
                    "progress",
                    "created_at",
                    "updated_at",
                    "retryable",
                    "cancelable",
                    "recoverable",
                    "stage",
                    "history_id",
                    "project_id",
                    "file_path",
                    "automation_rule_id",
                    "source_fingerprint",
                    "error_message",
                    "template_id",
                    "target_language",
                    "version",
                ]
            );

            for (table, removed_column) in [
                ("projects", "settings"),
                ("automation_rules", "data"),
                ("automation_processed", "data"),
                ("task_ledger", "data"),
            ] {
                assert!(
                    !table_columns(conn, table)
                        .iter()
                        .any(|column| column == removed_column),
                    "{table}.{removed_column} should not remain in the v1 baseline"
                );
            }

            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_app_config_projects_startup_fields_as_columns() {
        let db = Database::open_in_memory().unwrap();
        db.with_connection(|conn| {
            assert_eq!(
                table_columns(conn, "app_config"),
                vec![
                    "id",
                    "config",
                    "config_version",
                    "updated_at",
                    "http_server_enabled",
                    "http_server_host",
                    "http_server_port",
                    "http_server_api_key",
                    "http_server_max_concurrent",
                    "http_server_max_queue_size",
                    "http_server_max_upload_size_mb",
                    "http_server_job_ttl_minutes",
                    "http_server_max_streaming",
                    "http_server_ip_whitelist",
                    "gpu_acceleration",
                ]
            );
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_app_config_library_tables_have_structured_shape() {
        let db = Database::open_in_memory().unwrap();
        db.with_connection(|conn| {
            assert_eq!(
                table_columns(conn, "summary_templates"),
                vec![
                    "id",
                    "name",
                    "instructions",
                    "sort_order",
                    "created_at",
                    "updated_at",
                ]
            );
            assert_eq!(
                table_columns(conn, "polish_presets"),
                vec![
                    "id",
                    "name",
                    "context",
                    "sort_order",
                    "created_at",
                    "updated_at",
                ]
            );
            assert_eq!(
                table_columns(conn, "vocabulary_sets"),
                vec![
                    "id",
                    "kind",
                    "name",
                    "enabled",
                    "ignore_case",
                    "keywords",
                    "sort_order",
                    "created_at",
                    "updated_at",
                ]
            );
            assert_eq!(
                table_columns(conn, "vocabulary_rules"),
                vec![
                    "id",
                    "set_kind",
                    "set_id",
                    "from_text",
                    "to_text",
                    "text",
                    "sort_order",
                ]
            );
            assert_eq!(
                table_columns(conn, "speaker_profiles"),
                vec![
                    "id",
                    "name",
                    "enabled",
                    "sort_order",
                    "created_at",
                    "updated_at",
                ]
            );
            assert_eq!(
                table_columns(conn, "speaker_profile_samples"),
                vec![
                    "id",
                    "profile_id",
                    "file_path",
                    "source_name",
                    "duration_seconds",
                    "sort_order",
                ]
            );

            assert!(has_composite_primary_key(
                conn,
                "vocabulary_sets",
                &["kind", "id"]
            ));
            assert!(has_composite_primary_key(
                conn,
                "vocabulary_rules",
                &["set_kind", "set_id", "id"]
            ));
            assert!(has_composite_primary_key(
                conn,
                "speaker_profile_samples",
                &["profile_id", "id"]
            ));

            assert!(foreign_key_cascades_to(
                conn,
                "vocabulary_rules",
                "vocabulary_sets"
            ));
            assert!(foreign_key_cascades_to(
                conn,
                "speaker_profile_samples",
                "speaker_profiles"
            ));

            let invalid_kind = conn.execute(
                "INSERT INTO vocabulary_sets (kind, id) VALUES ('invalid', 'bad')",
                [],
            );
            assert!(invalid_kind.is_err());
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_trigram_support() {
        let db = Database::open_in_memory().unwrap();
        db.with_connection(|conn| {
            conn.execute_batch(
                "CREATE VIRTUAL TABLE test_fts USING fts5(content, tokenize='trigram');
                 INSERT INTO test_fts(content) VALUES ('中华人民共和国');
                 INSERT INTO test_fts(content) VALUES ('Hello World');",
            )?;
            let mut stmt =
                conn.prepare_cached("SELECT content FROM test_fts WHERE test_fts MATCH ?1")?;
            {
                let mut rows = stmt.query(["华人民"])?;
                assert!(rows.next()?.is_some());
            }
            {
                let mut rows2 = stmt.query(["hello"])?;
                assert!(rows2.next()?.is_some());
            }
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_fts5_triggers_sync_with_history_items() {
        let db = Database::open_in_memory().unwrap();
        db.with_connection(|conn| {
            // Verify trigger definition to prevent write amplification
            let trigger_sql: String = conn.query_row(
                "SELECT sql FROM sqlite_master WHERE type='trigger' AND name='tbl_history_items_au'",
                [],
                |row| row.get(0),
            )?;
            assert!(
                trigger_sql.contains("AFTER UPDATE OF title, preview_text, search_content ON history_items"),
                "Trigger tbl_history_items_au must restrict updates to title, preview_text, and search_content to prevent write amplification"
            );

            // 1. Test INSERT sync trigger
            conn.execute(
                "INSERT INTO history_items (id, timestamp, title, preview_text, search_content)
                 VALUES ('test-1', 12345, 'Testing Title One', 'Testing Preview One', 'Testing Search Content One')",
                [],
            )?;

            let count_fts: i64 = conn.query_row(
                "SELECT COUNT(*) FROM history_items_fts WHERE title MATCH 'Title'",
                [],
                |row| row.get(0),
            )?;
            assert_eq!(count_fts, 1);

            // 2. Test UPDATE sync trigger with non-FTS field (duration)
            conn.execute(
                "UPDATE history_items SET duration = 45.0 WHERE id = 'test-1'",
                [],
            )?;

            // Verify search still works and content is intact
            let count_fts_after_duration: i64 = conn.query_row(
                "SELECT COUNT(*) FROM history_items_fts WHERE title MATCH 'Title'",
                [],
                |row| row.get(0),
            )?;
            assert_eq!(count_fts_after_duration, 1);

            // 3. Test UPDATE sync trigger with FTS field (title)
            conn.execute(
                "UPDATE history_items SET title = 'Updated Title One' WHERE id = 'test-1'",
                [],
            )?;

            let count_fts_updated: i64 = conn.query_row(
                "SELECT COUNT(*) FROM history_items_fts WHERE title MATCH 'Updated'",
                [],
                |row| row.get(0),
            )?;
            assert_eq!(count_fts_updated, 1);

            // 4. Test DELETE sync trigger
            conn.execute(
                "DELETE FROM history_items WHERE id = 'test-1'",
                [],
            )?;

            let count_fts_deleted: i64 = conn.query_row(
                "SELECT COUNT(*) FROM history_items_fts",
                [],
                |row| row.get(0),
            )?;
            assert_eq!(count_fts_deleted, 0);

            Ok(())
        }).unwrap();
    }

    #[test]
    fn test_project_timestamp_index_exists() {
        let db = Database::open_in_memory().unwrap();
        db.with_connection(|conn| {
            let exists: bool = conn.query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master
                 WHERE type = 'index'
                   AND name = 'idx_history_items_project_timestamp'",
                [],
                |row| row.get(0),
            )?;
            assert!(exists);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_history_items_audio_status_column_defaults_available() {
        let db = Database::open_in_memory().unwrap();
        db.with_connection(|conn| {
            let mut stmt = conn.prepare("PRAGMA table_info(history_items)")?;
            let columns = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            })?;

            let mut found = None;
            for column in columns {
                let column = column?;
                if column.0 == "audio_status" {
                    found = Some(column);
                    break;
                }
            }

            assert_eq!(
                found,
                Some((
                    "audio_status".to_string(),
                    "TEXT".to_string(),
                    1,
                    Some("'available'".to_string()),
                ))
            );

            conn.execute(
                "INSERT INTO history_items (id, timestamp) VALUES ('audio-status-default', 1)",
                [],
            )?;
            let status: String = conn.query_row(
                "SELECT audio_status FROM history_items WHERE id = 'audio-status-default'",
                [],
                |row| row.get(0),
            )?;
            assert_eq!(status, "available");
            Ok(())
        })
        .unwrap();
    }
}
