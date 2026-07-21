    use super::*;
    use super::{HISTORY_ITEM_ROW_COLUMNS, history_insert_sql};
    use crate::Database;
    use serde_json::json;
    use sona_core::history::mutation_repository::{
        HistoryCompleteLiveDraftRequest, HistoryCreateTranscriptSnapshotRequest,
        HistoryItemMetaPatch, HistoryMutationError, HistoryMutationRepository,
        HistoryPurgeItemsRequest, HistoryReplaceTagAssignmentsRequest,
        HistoryRestoreItemsRequest, HistoryTrashItemsRequest, HistoryUpdateItemMetaRequest,
        HistoryUpdateTagAssignmentsRequest, HistoryUpdateTranscriptRequest,
    };
    use sona_core::history::query_repository::HistoryQueryRepository;
    use sona_core::history::{
        HistoryAudioCleanupRequest, HistoryAudioStatus, HistoryCreateLiveDraftRequest,
        HistoryDraftSource, HistoryItemKind, HistoryItemRecord, HistoryItemStatus,
        HistoryListOptions, HistorySaveImportedFileRequest, HistorySaveRecordingRequest,
        HistorySummaryPayload, HistoryWorkspaceDateFilter, HistoryWorkspaceFilterType,
        HistoryWorkspaceQueryRequest, HistoryWorkspaceScope, HistoryWorkspaceSortOrder,
        TranscriptSnapshotReason,
    };
    use sona_core::history_store::{HistoryStore, HistoryStoreError};
    use sona_core::ports::fs::{FileSystemError, FileSystemOperation};
    use sona_core::transcription::transcript::TranscriptSegment;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tempfile::tempdir;

    fn segment_value(id: &str, text: &str, start: f64, end: f64) -> TranscriptSegment {
        serde_json::from_value(json!({
            "id": id,
            "text": text,
            "start": start,
            "end": end,
            "isFinal": true
        }))
        .unwrap()
    }

    fn set_history_timestamp(store: &SqliteHistoryStore, history_id: &str, timestamp: u64) {
        store
            .get_db()
            .unwrap()
            .with_connection(|conn| {
                conn.execute(
                    "UPDATE history_items SET timestamp = ?1 WHERE id = ?2",
                    rusqlite::params![timestamp as i64, history_id],
                )?;
                Ok(())
            })
            .unwrap();
    }

    fn insert_workspace_item(
        store: &SqliteHistoryStore,
        id: &str,
        timestamp: i64,
        duration: f64,
        title: &str,
        kind: &str,
    ) {
        store
            .get_db()
            .unwrap()
            .with_connection(|conn| {
                conn.execute(
                    "INSERT INTO history_items (
                        id, timestamp, duration, title, preview_text, search_content, kind
                     ) VALUES (?1, ?2, ?3, ?4, ?4, ?4, ?5)",
                    rusqlite::params![id, timestamp, duration, title, kind],
                )?;
                Ok(())
            })
            .unwrap();
    }

    fn saved_audio_path(root: &tempfile::TempDir, item: &HistoryItemRecord) -> PathBuf {
        root.path().join("history").join(&item.audio_path)
    }

    fn table_columns(conn: &rusqlite::Connection, table: &str) -> Vec<String> {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .unwrap();
        let rows = stmt.query_map([], |row| row.get::<_, String>(1)).unwrap();

        rows.collect::<Result<Vec<_>, _>>().unwrap()
    }

    fn assert_query_not_found<T>(result: Result<T, HistoryStoreError>, expected_id: &str) {
        match result {
            Err(HistoryStoreError::Database(message)) => {
                assert!(message.contains(expected_id));
            }
            Ok(_) => panic!("expected NotFoundError for missing history item"),
            Err(error) => panic!("expected NotFoundError, got {error:?}"),
        }
    }

    fn assert_mutation_not_found<T>(result: Result<T, HistoryMutationError>, expected_id: &str) {
        match result {
            Err(HistoryMutationError::NotFound(message)) => {
                assert!(message.contains(expected_id));
            }
            Ok(_) => panic!("expected NotFoundError for missing history item"),
            Err(error) => panic!("expected NotFoundError, got {error:?}"),
        }
    }

    #[test]
    fn history_column_shape_matches_schema() {
        let db = Database::open_in_memory().unwrap();
        db.with_connection(|conn| {
            let mut expected: Vec<String> = HISTORY_ITEM_ROW_COLUMNS
                .iter()
                .map(|column| (*column).to_string())
                .collect();
            expected.push("created_at".to_string());
            expected.sort();
            let mut actual = table_columns(conn, "history_items");
            actual.sort();

            assert_eq!(actual, expected);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn history_row_mapper_reads_columns_by_name() {
        let db = Database::open_in_memory().unwrap();
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO tags (id, name, icon, color, sort_order, created_at, updated_at)
                 VALUES ('project-name-map', 'Mapped Tag', 'folder', '', 0, 1000, 1000)",
                [],
            )?;
            conn.execute(
                "INSERT INTO history_items (
                    id, timestamp, duration, audio_path, audio_status, transcript_path,
                    title, preview_text, icon, kind, search_content, deleted_at, status, draft_source
                )
                VALUES (
                    'history-name-map', 1234, 42.5, 'audio.wav', 'removed', 'history-name-map.json',
                    'Mapped title', 'Mapped preview', 'sparkles', 'batch', 'Mapped search',
                    NULL, 'draft', 'live_record'
                )",
                [],
            )?;
            conn.execute(
                "INSERT INTO history_item_tags (history_id, tag_id) VALUES ('history-name-map', 'project-name-map')",
                [],
            )?;

            let mut stmt = conn.prepare(
                "SELECT
                    draft_source AS draft_source,
                    status AS status,
                    deleted_at AS deleted_at,
                    '[\"project-name-map\"]' AS tag_ids,
                    search_content AS search_content,
                    kind AS kind,
                    icon AS icon,
                    preview_text AS preview_text,
                    title AS title,
                    transcript_path AS transcript_path,
                    audio_status AS audio_status,
                    audio_path AS audio_path,
                    duration AS duration,
                    timestamp AS timestamp,
                    id AS id
                 FROM history_items
                 WHERE id = 'history-name-map'",
            )?;
            let item = stmt.query_row([], map_row_to_item)?;

            assert_eq!(item.id, "history-name-map");
            assert_eq!(item.timestamp, 1234);
            assert_eq!(item.duration, 42.5);
            assert_eq!(item.audio_path, "audio.wav");
            assert_eq!(item.audio_status, HistoryAudioStatus::Removed);
            assert_eq!(item.transcript_path, "history-name-map.json");
            assert_eq!(item.title, "Mapped title");
            assert_eq!(item.preview_text, "Mapped preview");
            assert_eq!(item.icon.as_deref(), Some("sparkles"));
            assert_eq!(item.kind, HistoryItemKind::Batch);
            assert_eq!(item.search_content, "Mapped search");
            assert_eq!(item.tag_ids, vec!["project-name-map"]);
            assert_eq!(item.status, HistoryItemStatus::Draft);
            assert_eq!(item.draft_source, Some(HistoryDraftSource::LiveRecord));
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn history_insert_sql_uses_named_params_for_all_columns() {
        let sql = history_insert_sql();

        assert!(!sql.contains('?'));
        for column in HISTORY_ITEM_ROW_COLUMNS {
            assert!(
                sql.contains(&format!(":{column}")),
                "missing named param for {column} in {sql}"
            );
        }
        assert_eq!(sql.matches(':').count(), HISTORY_ITEM_ROW_COLUMNS.len());
    }

    #[test]
    fn audio_cleanup_removes_only_eligible_audio_and_preserves_text() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        let old_item = store
            .save_recording(HistorySaveRecordingRequest {
                segments: vec![segment_value(
                    "seg-old",
                    "Keep the old transcript",
                    0.0,
                    1.0,
                )],
                duration: 1.0,
                tag_ids: Vec::new(),
                audio_bytes: Some(vec![1, 2, 3, 4]),
                native_audio_path: None,
                audio_extension: Some("wav".to_string()),
            })
            .unwrap();
        let recent_item = store
            .save_recording(HistorySaveRecordingRequest {
                segments: vec![segment_value("seg-new", "Keep recent audio", 0.0, 1.0)],
                duration: 1.0,
                tag_ids: Vec::new(),
                audio_bytes: Some(vec![5, 6]),
                native_audio_path: None,
                audio_extension: Some("wav".to_string()),
            })
            .unwrap();

        set_history_timestamp(&store, &old_item.id, 1);

        let preview = store
            .preview_audio_cleanup(HistoryAudioCleanupRequest {
                retention_days: Some(7),
                exclude_history_id: None,
            })
            .unwrap();
        assert_eq!(preview.eligible_count, 1);
        assert_eq!(preview.removed_count, 1);
        assert_eq!(preview.removed_bytes, 4);
        assert_eq!(preview.missing_marked_count, 0);
        assert_eq!(preview.failed_count, 0);
        assert_eq!(preview.skipped_active_count, 0);
        assert!(saved_audio_path(&root, &old_item).exists());

        let report = store
            .cleanup_audio(HistoryAudioCleanupRequest {
                retention_days: Some(7),
                exclude_history_id: None,
            })
            .unwrap();
        assert_eq!(report, preview);
        assert!(!saved_audio_path(&root, &old_item).exists());
        assert!(saved_audio_path(&root, &recent_item).exists());

        let items = store.list_items().unwrap();
        let cleaned = items.iter().find(|item| item.id == old_item.id).unwrap();
        let kept = items.iter().find(|item| item.id == recent_item.id).unwrap();
        assert_eq!(cleaned.audio_status, HistoryAudioStatus::Removed);
        assert_eq!(kept.audio_status, HistoryAudioStatus::Available);

        let transcript = store.load_transcript(&old_item.id).unwrap().unwrap();
        assert_eq!(transcript[0].text, "Keep the old transcript");
    }

    #[test]
    fn audio_cleanup_skips_active_history_and_drafts() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        let active = store
            .save_recording(HistorySaveRecordingRequest {
                segments: vec![segment_value("seg-active", "Active transcript", 0.0, 1.0)],
                duration: 1.0,
                tag_ids: Vec::new(),
                audio_bytes: Some(vec![1]),
                native_audio_path: None,
                audio_extension: Some("wav".to_string()),
            })
            .unwrap();
        let draft = store
            .create_live_draft(HistoryCreateLiveDraftRequest {
                id: None,
                audio_extension: "wav".to_string(),
                tag_ids: Vec::new(),
                icon: None,
            })
            .unwrap()
            .item;
        std::fs::write(saved_audio_path(&root, &draft), [9, 9, 9]).unwrap();
        set_history_timestamp(&store, &active.id, 1);
        set_history_timestamp(&store, &draft.id, 1);

        let report = store
            .cleanup_audio(HistoryAudioCleanupRequest {
                retention_days: Some(0),
                exclude_history_id: Some(active.id.clone()),
            })
            .unwrap();

        assert_eq!(report.eligible_count, 0);
        assert_eq!(report.removed_count, 0);
        assert_eq!(report.skipped_active_count, 1);
        assert!(saved_audio_path(&root, &active).exists());
        assert!(saved_audio_path(&root, &draft).exists());

        let items = store.list_items().unwrap();
        assert_eq!(
            items
                .iter()
                .find(|item| item.id == active.id)
                .unwrap()
                .audio_status,
            HistoryAudioStatus::Available
        );
        assert_eq!(
            items
                .iter()
                .find(|item| item.id == draft.id)
                .unwrap()
                .audio_status,
            HistoryAudioStatus::Available
        );
    }

    #[test]
    fn audio_cleanup_marks_missing_audio_without_deleting_history() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        let missing = store
            .save_recording(HistorySaveRecordingRequest {
                segments: vec![segment_value(
                    "seg-missing",
                    "Text survives missing audio",
                    0.0,
                    1.0,
                )],
                duration: 1.0,
                tag_ids: Vec::new(),
                audio_bytes: Some(vec![1]),
                native_audio_path: None,
                audio_extension: Some("wav".to_string()),
            })
            .unwrap();
        set_history_timestamp(&store, &missing.id, 1);
        std::fs::remove_file(saved_audio_path(&root, &missing)).unwrap();

        let report = store
            .cleanup_audio(HistoryAudioCleanupRequest {
                retention_days: Some(0),
                exclude_history_id: None,
            })
            .unwrap();

        assert_eq!(report.eligible_count, 1);
        assert_eq!(report.removed_count, 0);
        assert_eq!(report.removed_bytes, 0);
        assert_eq!(report.missing_marked_count, 1);
        assert_eq!(report.failed_count, 0);

        let item = store
            .list_items()
            .unwrap()
            .into_iter()
            .find(|item| item.id == missing.id)
            .unwrap();
        assert_eq!(item.audio_status, HistoryAudioStatus::Missing);
        let transcript = store.load_transcript(&missing.id).unwrap().unwrap();
        assert_eq!(transcript[0].text, "Text survives missing audio");
    }

    #[cfg(windows)]
    #[test]
    fn audio_cleanup_keeps_available_when_file_deletion_fails() {
        use std::os::windows::fs::OpenOptionsExt;

        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        let item = store
            .save_recording(HistorySaveRecordingRequest {
                segments: vec![segment_value(
                    "seg-fail",
                    "Text survives delete failure",
                    0.0,
                    1.0,
                )],
                duration: 1.0,
                tag_ids: Vec::new(),
                audio_bytes: Some(vec![1, 2, 3]),
                native_audio_path: None,
                audio_extension: Some("wav".to_string()),
            })
            .unwrap();
        set_history_timestamp(&store, &item.id, 1);

        let audio_path = saved_audio_path(&root, &item);
        let locked_file = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&audio_path)
            .unwrap();

        let report = store
            .cleanup_audio(HistoryAudioCleanupRequest {
                retention_days: Some(0),
                exclude_history_id: None,
            })
            .unwrap();

        assert_eq!(report.eligible_count, 1);
        assert_eq!(report.removed_count, 0);
        assert_eq!(report.removed_bytes, 0);
        assert_eq!(report.missing_marked_count, 0);
        assert_eq!(report.failed_count, 1);
        assert!(audio_path.exists());

        let item = store
            .list_items()
            .unwrap()
            .into_iter()
            .find(|candidate| candidate.id == item.id)
            .unwrap();
        assert_eq!(item.audio_status, HistoryAudioStatus::Available);

        drop(locked_file);
    }

    #[test]
    fn audio_cleanup_disabled_when_retention_is_none() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        let item = store
            .save_recording(HistorySaveRecordingRequest {
                segments: vec![segment_value("seg-keep", "Keep forever", 0.0, 1.0)],
                duration: 1.0,
                tag_ids: Vec::new(),
                audio_bytes: Some(vec![1, 2]),
                native_audio_path: None,
                audio_extension: Some("wav".to_string()),
            })
            .unwrap();
        set_history_timestamp(&store, &item.id, 1);

        let report = store
            .cleanup_audio(HistoryAudioCleanupRequest {
                retention_days: None,
                exclude_history_id: None,
            })
            .unwrap();

        assert_eq!(report, HistoryAudioCleanupReport::default());
        assert!(saved_audio_path(&root, &item).exists());
        assert_eq!(
            store.list_items().unwrap()[0].audio_status,
            HistoryAudioStatus::Available
        );
    }

    #[test]
    fn save_imported_file_rolls_back_db_and_cleans_staging_when_promote_fails() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        let source_path = root.path().join("source.wav");
        std::fs::write(&source_path, [1, 2, 3]).unwrap();
        let target_path = root.path().join("history").join("promote-fail.wav");
        std::fs::write(&target_path, [9, 9, 9]).unwrap();

        let result = store.save_imported_file(HistorySaveImportedFileRequest {
            id: Some("promote-fail".to_string()),
            source_path: source_path.to_string_lossy().to_string(),
            converted_source_path: None,
            segments: vec![segment_value("seg-promote", "Promote failure", 0.0, 1.0)],
            duration: 1.0,
            tag_ids: Vec::new(),
        });

        assert!(
            matches!(result, Err(HistoryMutationError::Internal(message)) if message.contains("History audio target already exists"))
        );
        assert_query_not_found(store.load_transcript("promote-fail"), "promote-fail");
        assert_eq!(std::fs::read(&target_path).unwrap(), vec![9, 9, 9]);
        let staged_entries = std::fs::read_dir(root.path().join("history"))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .contains(STAGED_AUDIO_MARKER)
            })
            .count();
        assert_eq!(staged_entries, 0);
    }

    #[test]
    fn mutation_readiness_filesystem_failures_preserve_operation_and_path() {
        let root = tempdir().unwrap();
        let blocked_app_data = root.path().join("app-data-file");
        std::fs::write(&blocked_app_data, [1]).unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(blocked_app_data, db);

        let result = store.create_live_draft(HistoryCreateLiveDraftRequest {
            id: Some("draft-1".to_string()),
            audio_extension: "wav".to_string(),
            tag_ids: Vec::new(),
            icon: None,
        });

        assert!(matches!(
            result,
            Err(HistoryMutationError::FileSystem(FileSystemError {
                operation: FileSystemOperation::CreateDirectory,
                path,
                ..
            })) if path == root.path().join("app-data-file").join(HISTORY_DIR_NAME)
        ));
    }

    #[test]
    fn history_file_lock_serializes_staging_cleanup_across_store_handles() {
        use std::sync::mpsc;
        use std::thread;
        use std::time::{Duration, Instant};

        let root = tempdir().unwrap();
        let first = SqliteHistoryStore::with_db(
            root.path().to_path_buf(),
            Database::open_in_memory().unwrap(),
        );
        let second = SqliteHistoryStore::with_db(
            root.path().to_path_buf(),
            Database::open_in_memory().unwrap(),
        );
        let history_dir = root.path().join(HISTORY_DIR_NAME);
        let staging_path = history_dir.join(format!("active{STAGED_AUDIO_MARKER}test"));
        let staging_path_for_first = staging_path.clone();
        let (staged_tx, staged_rx) = mpsc::channel();

        thread::scope(|scope| {
            let first_handle = scope.spawn(move || {
                first
                    .with_history_file_lock(|| {
                        std::fs::create_dir_all(&history_dir)
                            .map_err(|error| HistoryMutationError::Internal(error.to_string()))?;
                        std::fs::write(&staging_path_for_first, [1, 2, 3])
                            .map_err(|error| HistoryMutationError::Internal(error.to_string()))?;
                        staged_tx.send(()).unwrap();
                        thread::sleep(Duration::from_millis(150));
                        assert!(staging_path_for_first.exists());
                        Ok(())
                    })
                    .unwrap();
            });
            let second_handle = scope.spawn(move || {
                staged_rx.recv().unwrap();
                let started = Instant::now();
                second
                    .with_history_file_lock(|| {
                        second
                            .cleanup_stale_staged_audio_files()
                            .map_err(HistoryMutationError::from)
                    })
                    .unwrap();
                assert!(started.elapsed() >= Duration::from_millis(100));
            });
            first_handle.join().unwrap();
            second_handle.join().unwrap();
        });

        assert!(!staging_path.exists());
    }

    #[test]
    fn test_sqlite_store_crud() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        // Insert referenced project first to satisfy foreign key constraint
        store
            .get_db()
            .unwrap()
            .with_connection(|conn| {
                conn.execute(
                    "INSERT INTO tags (id, name, icon, color, sort_order, created_at, updated_at)
                     VALUES ('project-1', 'Project One', 'folder', '', 0, 1000, 1000)",
                    [],
                )?;
                Ok(())
            })
            .unwrap();

        // 1. Save recording
        let recording = store
            .save_recording(HistorySaveRecordingRequest {
                segments: vec![segment_value("seg-1", "Hello world", 0.0, 2.0)],
                duration: 2.0,
                tag_ids: vec!["project-1".to_string()],
                audio_bytes: Some(vec![1, 2, 3]),
                native_audio_path: None,
                audio_extension: Some("wav".to_string()),
            })
            .unwrap();

        assert_eq!(recording.preview_text, "Hello world...");
        assert_eq!(recording.search_content, "Hello world");
        assert_eq!(recording.tag_ids, vec!["project-1"]);
        assert_eq!(recording.audio_status, HistoryAudioStatus::Available);

        // 2. Load transcript
        let transcript = store.load_transcript(&recording.id).unwrap().unwrap();
        assert_eq!(transcript.len(), 1);
        assert_eq!(transcript[0].text, "Hello world");

        // 3. Update transcript
        let updated = store
            .update_transcript(
                &recording.id,
                vec![segment_value("seg-1", "Hello updated", 0.0, 3.0)],
            )
            .unwrap();
        assert_eq!(updated.preview_text, "Hello updated...");

        // 4. Update item metadata
        store
            .update_item_meta(
                &recording.id,
                HistoryItemMetaPatch {
                    title: Some("New Title".to_string()),
                    ..HistoryItemMetaPatch::default()
                },
            )
            .unwrap();
        store
            .replace_tag_assignments(std::slice::from_ref(&recording.id), &[])
            .unwrap();
        let items = store.list_items().unwrap();
        assert_eq!(items[0].title, "New Title");
        assert!(items[0].tag_ids.is_empty());

        // 5. Load summary & save summary
        assert_eq!(store.load_summary(&recording.id).unwrap(), None);
        store
            .save_summary(
                &recording.id,
                HistorySummaryPayload {
                    active_template_id: "summary-1".to_string(),
                    record: None,
                },
            )
            .unwrap();
        assert_eq!(
            store.load_summary(&recording.id).unwrap(),
            Some(HistorySummaryPayload {
                active_template_id: "summary-1".to_string(),
                record: None,
            })
        );

        // 6. Delete item
        store
            .trash_items(std::slice::from_ref(&recording.id), 10_000)
            .unwrap();
        let items = store.list_items().unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn update_transcript_missing_history_returns_not_found() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        assert_mutation_not_found(
            store.update_transcript(
                "missing-history",
                vec![segment_value("seg-1", "Missing", 0.0, 1.0)],
            ),
            "missing-history",
        );
    }

    #[test]
    fn complete_live_draft_missing_history_returns_not_found() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        assert_mutation_not_found(
            store.complete_live_draft(
                "missing-history",
                vec![segment_value("seg-1", "Missing", 0.0, 1.0)],
                1.0,
            ),
            "missing-history",
        );
    }

    #[test]
    fn update_item_meta_missing_history_returns_not_found() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        assert_mutation_not_found(
            store.update_item_meta(
                "missing-history",
                HistoryItemMetaPatch {
                    title: Some("Missing".to_string()),
                    ..HistoryItemMetaPatch::default()
                },
            ),
            "missing-history",
        );
    }

    #[test]
    fn create_transcript_snapshot_missing_history_returns_not_found() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        assert_mutation_not_found(
            store.create_transcript_snapshot(
                "missing-history",
                TranscriptSnapshotReason::Polish,
                vec![segment_value("seg-1", "Missing", 0.0, 1.0)],
            ),
            "missing-history",
        );
    }

    #[test]
    fn save_imported_file_duplicate_id_does_not_overwrite_existing_audio() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        let first_source = root.path().join("first.wav");
        let second_source = root.path().join("second.wav");
        std::fs::write(&first_source, [1, 2, 3]).unwrap();
        std::fs::write(&second_source, [9, 8, 7]).unwrap();

        let item = store
            .save_imported_file(HistorySaveImportedFileRequest {
                id: Some("import-1".to_string()),
                source_path: first_source.to_string_lossy().to_string(),
                segments: vec![segment_value("seg-1", "Original import", 0.0, 1.0)],
                duration: 1.0,
                tag_ids: Vec::new(),
                converted_source_path: None,
            })
            .unwrap();
        let audio_path = root.path().join("history").join(&item.audio_path);
        assert_eq!(std::fs::read(&audio_path).unwrap(), vec![1, 2, 3]);

        let duplicate = store.save_imported_file(HistorySaveImportedFileRequest {
            id: Some("import-1".to_string()),
            source_path: second_source.to_string_lossy().to_string(),
            segments: vec![segment_value("seg-2", "Duplicate import", 0.0, 1.0)],
            duration: 1.0,
            tag_ids: Vec::new(),
            converted_source_path: None,
        });

        assert!(duplicate.is_err());
        assert_eq!(std::fs::read(&audio_path).unwrap(), vec![1, 2, 3]);
        assert_eq!(store.list_items().unwrap().len(), 1);
    }

    #[test]
    fn resolve_audio_path_marks_available_item_missing_without_deleting_text() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        let recording = store
            .save_recording(HistorySaveRecordingRequest {
                segments: vec![segment_value("seg-1", "Keep text", 0.0, 1.0)],
                duration: 1.0,
                tag_ids: Vec::new(),
                audio_bytes: Some(vec![1]),
                native_audio_path: None,
                audio_extension: Some("wav".to_string()),
            })
            .unwrap();

        std::fs::remove_file(root.path().join("history").join(&recording.audio_path)).unwrap();

        assert_eq!(store.resolve_audio_path(&recording.id).unwrap(), None);
        let item = store
            .list_items()
            .unwrap()
            .into_iter()
            .find(|item| item.id == recording.id)
            .unwrap();
        assert_eq!(item.audio_status, HistoryAudioStatus::Missing);
        let transcript = store.load_transcript(&recording.id).unwrap().unwrap();
        assert_eq!(transcript[0].text, "Keep text");
    }

    #[test]
    fn resolve_audio_path_preserves_removed_status_when_file_is_missing() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        let recording = store
            .save_recording(HistorySaveRecordingRequest {
                segments: vec![segment_value("seg-1", "Removed audio text", 0.0, 1.0)],
                duration: 1.0,
                tag_ids: Vec::new(),
                audio_bytes: Some(vec![1]),
                native_audio_path: None,
                audio_extension: Some("wav".to_string()),
            })
            .unwrap();
        store
            .update_item_meta(
                &recording.id,
                HistoryItemMetaPatch {
                    audio_status: Some(HistoryAudioStatus::Removed),
                    ..HistoryItemMetaPatch::default()
                },
            )
            .unwrap();
        std::fs::remove_file(root.path().join("history").join(&recording.audio_path)).unwrap();

        assert_eq!(store.resolve_audio_path(&recording.id).unwrap(), None);
        let item = store
            .list_items()
            .unwrap()
            .into_iter()
            .find(|item| item.id == recording.id)
            .unwrap();
        assert_eq!(item.audio_status, HistoryAudioStatus::Removed);
    }

    #[test]
    fn ensure_ready_removes_stale_staging_files_without_removing_final_audio() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);

        let history_dir = root.path().join("history");
        std::fs::create_dir_all(&history_dir).unwrap();
        let final_audio = history_dir.join("orphan.wav");
        let staged_audio = history_dir.join("orphan.wav.sona-staging-old");
        std::fs::write(&final_audio, [1]).unwrap();
        std::fs::write(&staged_audio, [2]).unwrap();

        store.ensure_ready().unwrap();

        assert!(final_audio.exists());
        assert!(!staged_audio.exists());
    }

    #[test]
    fn test_sqlite_store_cascades() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        let recording = store
            .save_recording(HistorySaveRecordingRequest {
                segments: vec![segment_value("seg-1", "Hello", 0.0, 1.0)],
                duration: 1.0,
                tag_ids: Vec::new(),
                audio_bytes: Some(vec![1]),
                native_audio_path: None,
                audio_extension: Some("wav".to_string()),
            })
            .unwrap();

        // Save summary
        store
            .save_summary(
                &recording.id,
                HistorySummaryPayload {
                    active_template_id: "general".to_string(),
                    record: None,
                },
            )
            .unwrap();
        // Create transcript snapshot
        store
            .create_transcript_snapshot(&recording.id, TranscriptSnapshotReason::Polish, Vec::new())
            .unwrap();

        // Verify they exist in DB
        store
            .get_db()
            .unwrap()
            .with_connection(|conn| {
                let t_count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM history_transcripts WHERE history_id = ?1",
                    [&recording.id],
                    |r| r.get(0),
                )?;
                let s_count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM history_summaries WHERE history_id = ?1",
                    [&recording.id],
                    |r| r.get(0),
                )?;
                let snap_count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM transcript_snapshots WHERE history_id = ?1",
                    [&recording.id],
                    |r| r.get(0),
                )?;
                assert_eq!(t_count, 1);
                assert_eq!(s_count, 1);
                assert_eq!(snap_count, 1);
                Ok(())
            })
            .unwrap();

        // Delete parent item
        store
            .trash_items(std::slice::from_ref(&recording.id), 10_000)
            .unwrap();

        // Soft deletion keeps child records until the item is explicitly purged.
        store
            .get_db()
            .unwrap()
            .with_connection(|conn| {
                let child_count: i64 = conn.query_row(
                    "SELECT (SELECT COUNT(*) FROM history_transcripts WHERE history_id = ?1)
                          + (SELECT COUNT(*) FROM history_summaries WHERE history_id = ?1)
                          + (SELECT COUNT(*) FROM transcript_snapshots WHERE history_id = ?1)",
                    [&recording.id],
                    |row| row.get(0),
                )?;
                assert_eq!(child_count, 3);
                Ok(())
            })
            .unwrap();
        store
            .purge_items(std::slice::from_ref(&recording.id))
            .unwrap();

        // Verify child tables are automatically pruned by ON DELETE CASCADE
        store
            .get_db()
            .unwrap()
            .with_connection(|conn| {
                let t_count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM history_transcripts WHERE history_id = ?1",
                    [&recording.id],
                    |r| r.get(0),
                )?;
                let s_count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM history_summaries WHERE history_id = ?1",
                    [&recording.id],
                    |r| r.get(0),
                )?;
                let snap_count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM transcript_snapshots WHERE history_id = ?1",
                    [&recording.id],
                    |r| r.get(0),
                )?;
                assert_eq!(t_count, 0);
                assert_eq!(s_count, 0);
                assert_eq!(snap_count, 0);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn test_sqlite_store_workspace_query() {
        use sona_core::history::{
            HistoryWorkspaceDateFilter, HistoryWorkspaceFilterType, HistoryWorkspaceScope,
            HistoryWorkspaceSortOrder,
        };

        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        // Insert referenced project first to satisfy foreign key constraint
        store
            .get_db()
            .unwrap()
            .with_connection(|conn| {
                conn.execute(
                    "INSERT INTO tags (id, name, icon, color, sort_order, created_at, updated_at)
                     VALUES ('project-1', 'Project One', 'folder', '', 0, 1000, 1000)",
                    [],
                )?;
                Ok(())
            })
            .unwrap();

        // Create alpha item
        let _alpha = store
            .save_recording(HistorySaveRecordingRequest {
                segments: vec![segment_value(
                    "seg-1",
                    "Alpha roadmap discussion",
                    0.0,
                    10.0,
                )],
                duration: 10.0,
                tag_ids: vec!["project-1".to_string()],
                audio_bytes: Some(vec![1]),
                native_audio_path: None,
                audio_extension: Some("wav".to_string()),
            })
            .unwrap();

        // Create batch item
        let source_file = root.path().join("import.wav");
        std::fs::write(&source_file, [1, 2, 3]).unwrap();
        let _beta = store
            .save_imported_file(HistorySaveImportedFileRequest {
                id: None,
                source_path: source_file.to_string_lossy().to_string(),
                segments: vec![segment_value("seg-2", "Beta notes", 0.0, 20.0)],
                duration: 20.0,
                tag_ids: vec!["project-1".to_string()],
                converted_source_path: None,
            })
            .unwrap();

        // Query workspace
        let result = store
            .query_workspace(HistoryWorkspaceQueryRequest {
                scope: HistoryWorkspaceScope::Tag {
                    tag_id: "project-1".to_string(),
                },
                query: "roadmap".to_string(),
                filter_type: HistoryWorkspaceFilterType::Recording,
                date_filter: HistoryWorkspaceDateFilter::All,
                sort_order: HistoryWorkspaceSortOrder::TitleAsc,
                limit: 100,
                offset: 0,
            })
            .unwrap();

        assert_eq!(result.filtered_items.len(), 1);
        assert_eq!(
            result.filtered_items[0].preview_text,
            "Alpha roadmap discussion..."
        );
    }

    #[test]
    fn workspace_query_applies_each_sort_before_pagination() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        insert_workspace_item(&store, "a", 100, 5.0, "Zulu", "recording");
        insert_workspace_item(&store, "b", 300, 2.0, "alpha", "recording");
        insert_workspace_item(&store, "c", 200, 2.0, "Beta", "recording");
        insert_workspace_item(&store, "d", 300, 7.0, "alpha", "recording");
        insert_workspace_item(&store, "batch", 400, 1.0, "Ignored", "batch");

        let cases = [
            (HistoryWorkspaceSortOrder::Newest, ["d", "c"]),
            (HistoryWorkspaceSortOrder::Oldest, ["c", "b"]),
            (HistoryWorkspaceSortOrder::DurationDesc, ["a", "b"]),
            (HistoryWorkspaceSortOrder::DurationAsc, ["c", "a"]),
            (HistoryWorkspaceSortOrder::TitleAsc, ["d", "c"]),
        ];

        for (sort_order, expected_ids) in cases {
            let result = store
                .query_workspace(HistoryWorkspaceQueryRequest {
                    scope: HistoryWorkspaceScope::All,
                    query: String::new(),
                    filter_type: HistoryWorkspaceFilterType::Recording,
                    date_filter: HistoryWorkspaceDateFilter::All,
                    sort_order,
                    limit: 2,
                    offset: 1,
                })
                .unwrap();

            assert_eq!(
                result
                    .filtered_items
                    .iter()
                    .map(|item| item.id.as_str())
                    .collect::<Vec<_>>(),
                expected_ids
            );
            assert_eq!(result.filtered_item_count, 4);
            assert!(result.has_more);
            assert_eq!(result.summary.total_items, 5);
        }
    }

    #[test]
    fn workspace_query_rejects_invalid_pagination() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        for (limit, offset) in [(0, 0), (201, 0), (1, usize::MAX)] {
            let result = store.query_workspace(HistoryWorkspaceQueryRequest {
                scope: HistoryWorkspaceScope::All,
                query: String::new(),
                filter_type: HistoryWorkspaceFilterType::All,
                date_filter: HistoryWorkspaceDateFilter::All,
                sort_order: HistoryWorkspaceSortOrder::Newest,
                limit,
                offset,
            });

            assert!(matches!(result, Err(HistoryStoreError::InvalidRequest(_))));
        }
    }

    #[test]
    fn workspace_query_paginates_exact_search_matches() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        insert_workspace_item(
            &store,
            "newest-miss",
            500,
            1.0,
            "Status update",
            "recording",
        );
        insert_workspace_item(
            &store,
            "first-match",
            400,
            1.0,
            "Roadmap alpha",
            "recording",
        );
        insert_workspace_item(
            &store,
            "middle-miss",
            300,
            1.0,
            "Release notes",
            "recording",
        );
        insert_workspace_item(
            &store,
            "second-match",
            200,
            1.0,
            "Roadmap beta",
            "recording",
        );

        let result = store
            .query_workspace(HistoryWorkspaceQueryRequest {
                scope: HistoryWorkspaceScope::All,
                query: "roadmap".to_string(),
                filter_type: HistoryWorkspaceFilterType::All,
                date_filter: HistoryWorkspaceDateFilter::All,
                sort_order: HistoryWorkspaceSortOrder::Newest,
                limit: 1,
                offset: 1,
            })
            .unwrap();

        assert_eq!(result.filtered_item_count, 2);
        assert!(!result.has_more);
        assert_eq!(result.filtered_items[0].id, "second-match");
        assert_eq!(
            result
                .search_match_by_item_id
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["second-match".to_string()]
        );
    }

    #[test]
    fn workspace_query_search_matches_nfkc_equivalent_text() {
        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        insert_workspace_item(
            &store,
            "full-width",
            100,
            1.0,
            "\u{ff21}\u{ff22}\u{ff23} planning",
            "recording",
        );

        let result = store
            .query_workspace(HistoryWorkspaceQueryRequest {
                scope: HistoryWorkspaceScope::All,
                query: "abc".to_string(),
                filter_type: HistoryWorkspaceFilterType::All,
                date_filter: HistoryWorkspaceDateFilter::All,
                sort_order: HistoryWorkspaceSortOrder::Newest,
                limit: 100,
                offset: 0,
            })
            .unwrap();

        assert_eq!(result.filtered_item_count, 1);
        assert_eq!(result.filtered_items[0].id, "full-width");
        assert!(result.search_match_by_item_id["full-width"].is_some());
    }

    #[test]
    fn workspace_query_builds_metadata_and_page_from_one_read_snapshot() {
        let root = tempdir().unwrap();
        let store = SqliteHistoryStore::with_db(
            root.path().to_path_buf(),
            Database::open(root.path()).unwrap(),
        );
        store.ensure_ready().unwrap();
        insert_workspace_item(
            &store,
            "existing",
            100,
            1.0,
            "snapshotmarker existing",
            "recording",
        );

        let (start_write_tx, start_write_rx) = std::sync::mpsc::channel();
        let (write_done_tx, write_done_rx) = std::sync::mpsc::channel();
        let db_path = root.path().join("sona.db");
        let writer = std::thread::spawn(move || {
            start_write_rx.recv().unwrap();
            let conn = rusqlite::Connection::open(db_path).unwrap();
            conn.execute_batch("PRAGMA busy_timeout=5000;").unwrap();
            conn.execute(
                "INSERT INTO history_items (
                    id, timestamp, duration, title, preview_text, search_content, kind
                 ) VALUES ('concurrent', 200, 1.0, 'snapshotmarker concurrent',
                           'snapshotmarker concurrent', 'snapshotmarker concurrent', 'recording')",
                [],
            )
            .unwrap();
            write_done_tx.send(()).unwrap();
        });

        crate::set_workspace_match_test_hook(
            "snapshotmarker",
            Box::new(move || {
                start_write_tx.send(()).unwrap();
                write_done_rx.recv().unwrap();
            }),
        );

        let result = store
            .query_workspace(HistoryWorkspaceQueryRequest {
                scope: HistoryWorkspaceScope::All,
                query: "snapshotmarker".to_string(),
                filter_type: HistoryWorkspaceFilterType::All,
                date_filter: HistoryWorkspaceDateFilter::All,
                sort_order: HistoryWorkspaceSortOrder::Newest,
                limit: 100,
                offset: 0,
            })
            .unwrap();
        writer.join().unwrap();

        assert_eq!(result.filtered_item_count, 1);
        assert_eq!(result.filtered_items.len(), 1);
        assert_eq!(result.filtered_items[0].id, "existing");
        assert_eq!(result.summary.total_items, 1);
        assert_eq!(result.item_counts.untagged, 1);
    }

    #[test]
    fn test_workspace_query_with_reconciliation() {
        use sona_core::history::{
            HistoryWorkspaceDateFilter, HistoryWorkspaceFilterType, HistoryWorkspaceScope,
            HistoryWorkspaceSortOrder,
        };

        let root = tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let store = SqliteHistoryStore::with_db(root.path().to_path_buf(), db);
        store.ensure_ready().unwrap();

        // Insert referenced project first to satisfy foreign key constraint
        store
            .get_db()
            .unwrap()
            .with_connection(|conn| {
                conn.execute(
                    "INSERT INTO tags (id, name, icon, color, sort_order, created_at, updated_at)
                     VALUES ('project-1', 'Project One', 'folder', '', 0, 1000, 1000)",
                    [],
                )?;
                Ok(())
            })
            .unwrap();

        // 1. Create a live draft item
        let draft_res = store
            .create_live_draft(HistoryCreateLiveDraftRequest {
                id: None,
                audio_extension: "wav".to_string(),
                tag_ids: vec!["project-1".to_string()],
                icon: None,
            })
            .unwrap();

        // At this point, the draft is in the database with status = 'draft' and draft_source = 'live_record'.
        // But since there is no audio file yet (or segments in transcripts), reconcile_live_drafts should skip it.
        let result = store
            .query_workspace(HistoryWorkspaceQueryRequest {
                scope: HistoryWorkspaceScope::All,
                query: "".to_string(),
                filter_type: HistoryWorkspaceFilterType::All,
                date_filter: HistoryWorkspaceDateFilter::All,
                sort_order: HistoryWorkspaceSortOrder::Newest,
                limit: 100,
                offset: 0,
            })
            .unwrap();
        assert_eq!(result.filtered_items.len(), 1);
        assert_eq!(result.filtered_items[0].status, HistoryItemStatus::Draft);

        // 2. Write the audio file so reconcile_live_drafts will process it
        let audio_path = root.path().join("history").join(&draft_res.item.audio_path);
        if let Some(parent) = audio_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&audio_path, [1, 2, 3]).unwrap();

        // 3. Insert some dummy transcript segments into history_transcripts
        let segments_str = serde_json::to_string(&json!([
            {
                "id": "seg-1",
                "text": "Hello world from reconciled draft",
                "start": 0.0,
                "end": 5.0
            }
        ]))
        .unwrap();
        store
            .get_db()
            .unwrap()
            .with_transaction(|tx| {
                tx.execute(
                "INSERT OR REPLACE INTO history_transcripts (history_id, segments) VALUES (?1, ?2)",
                rusqlite::params![draft_res.item.id, segments_str]
            )?;
                Ok(())
            })
            .unwrap();

        // 4. Query workspace again, which should trigger reconciliation
        let result = store
            .query_workspace(HistoryWorkspaceQueryRequest {
                scope: HistoryWorkspaceScope::All,
                query: "reconciled".to_string(),
                filter_type: HistoryWorkspaceFilterType::All,
                date_filter: HistoryWorkspaceDateFilter::All,
                sort_order: HistoryWorkspaceSortOrder::Newest,
                limit: 100,
                offset: 0,
            })
            .unwrap();

        assert_eq!(result.filtered_items.len(), 1);
        assert_eq!(result.filtered_items[0].status, HistoryItemStatus::Complete);
        assert_eq!(
            result.filtered_items[0].preview_text,
            "Hello world from reconciled draft..."
        );
    }

    #[test]
    fn test_sqlite_store_fts_workspace_query() {
        use sona_core::history::{
            HistoryWorkspaceDateFilter, HistoryWorkspaceFilterType, HistoryWorkspaceScope,
            HistoryWorkspaceSortOrder,
        };

        let root = tempfile::TempDir::new().unwrap();
        let store = SqliteHistoryStore::with_db(
            root.path().to_path_buf(),
            Database::open_in_memory().unwrap(),
        );
        store.ensure_ready().unwrap();

        // Save test item with Chinese and English text
        let item = store
            .save_recording(HistorySaveRecordingRequest {
                segments: vec![
                    segment_value("seg-1", "你好世界，这是一个测试", 0.0, 2.0),
                    segment_value("seg-2", "Fuzzy matching should be fast", 2.0, 4.0),
                ],
                duration: 4.0,
                tag_ids: Vec::new(),
                audio_bytes: Some(vec![1, 2, 3]),
                native_audio_path: None,
                audio_extension: Some("wav".to_string()),
            })
            .unwrap();

        // Save second test item with full-width Chinese punctuation for punctuation matching
        let item_punc = store
            .save_recording(HistorySaveRecordingRequest {
                segments: vec![segment_value("seg-3", "你好，世界，这是一个测试", 0.0, 2.0)],
                duration: 2.0,
                tag_ids: Vec::new(),
                audio_bytes: Some(vec![1, 2, 3]),
                native_audio_path: None,
                audio_extension: Some("wav".to_string()),
            })
            .unwrap();

        // 1. Test Chinese search match
        let req_zh = HistoryWorkspaceQueryRequest {
            scope: HistoryWorkspaceScope::All,
            query: "你好世界".to_string(),
            filter_type: HistoryWorkspaceFilterType::All,
            date_filter: HistoryWorkspaceDateFilter::All,
            sort_order: HistoryWorkspaceSortOrder::Newest,
            limit: 100,
            offset: 0,
        };
        let res_zh = store.query_workspace(req_zh).unwrap();
        assert_eq!(res_zh.filtered_items.len(), 1);
        assert_eq!(res_zh.filtered_items[0].id, item.id);
        // New assertions verifying optimized behavior:
        assert_eq!(res_zh.filtered_item_count, 1);
        assert_eq!(res_zh.summary.total_items, 2); // Summary correctly counts all items in the scope

        // 2. Test fuzzy/substring match
        let req_fuzzy = HistoryWorkspaceQueryRequest {
            scope: HistoryWorkspaceScope::All,
            query: "fuzzy".to_string(),
            filter_type: HistoryWorkspaceFilterType::All,
            date_filter: HistoryWorkspaceDateFilter::All,
            sort_order: HistoryWorkspaceSortOrder::Newest,
            limit: 100,
            offset: 0,
        };
        let res_fuzzy = store.query_workspace(req_fuzzy).unwrap();
        assert_eq!(res_fuzzy.filtered_items.len(), 1);
        assert_eq!(res_fuzzy.filtered_items[0].id, item.id);

        // 3. Test non-matching query
        let req_none = HistoryWorkspaceQueryRequest {
            scope: HistoryWorkspaceScope::All,
            query: "unrelated".to_string(),
            filter_type: HistoryWorkspaceFilterType::All,
            date_filter: HistoryWorkspaceDateFilter::All,
            sort_order: HistoryWorkspaceSortOrder::Newest,
            limit: 100,
            offset: 0,
        };
        let res_none = store.query_workspace(req_none).unwrap();
        assert_eq!(res_none.filtered_items.len(), 0);

        // 4. Test queries with full-width Chinese punctuation successfully matching
        let req_punc1 = HistoryWorkspaceQueryRequest {
            scope: HistoryWorkspaceScope::All,
            query: "你好，世界".to_string(),
            filter_type: HistoryWorkspaceFilterType::All,
            date_filter: HistoryWorkspaceDateFilter::All,
            sort_order: HistoryWorkspaceSortOrder::Newest,
            limit: 100,
            offset: 0,
        };
        let res_punc1 = store.query_workspace(req_punc1).unwrap();
        assert_eq!(res_punc1.filtered_items.len(), 1);
        assert_eq!(res_punc1.filtered_items[0].id, item_punc.id);

        let req_punc2 = HistoryWorkspaceQueryRequest {
            scope: HistoryWorkspaceScope::All,
            query: "你好、世界".to_string(),
            filter_type: HistoryWorkspaceFilterType::All,
            date_filter: HistoryWorkspaceDateFilter::All,
            sort_order: HistoryWorkspaceSortOrder::Newest,
            limit: 100,
            offset: 0,
        };
        let res_punc2 = store.query_workspace(req_punc2).unwrap();
        assert_eq!(res_punc2.filtered_items.len(), 1);
        assert_eq!(res_punc2.filtered_items[0].id, item_punc.id);

        // 5. Test short Chinese query (6 bytes / 2 chars): valid for
        //    byte-level trigram — FTS should be used for both items
        let req_short_zh = HistoryWorkspaceQueryRequest {
            scope: HistoryWorkspaceScope::All,
            query: "你好".to_string(), // 2 chars, 6 bytes
            filter_type: HistoryWorkspaceFilterType::All,
            date_filter: HistoryWorkspaceDateFilter::All,
            sort_order: HistoryWorkspaceSortOrder::Newest,
            limit: 100,
            offset: 0,
        };
        let res_short_zh = store.query_workspace(req_short_zh).unwrap();
        let short_zh_ids: std::collections::HashSet<&str> = res_short_zh
            .filtered_items
            .iter()
            .map(|i| i.id.as_str())
            .collect();
        assert!(short_zh_ids.contains(item.id.as_str()));
        assert!(short_zh_ids.contains(item_punc.id.as_str()));

        // 6. Test short ASCII query (< 3 bytes): must fall back to in-memory
        //    matching on search_content loaded from DB
        let req_short_en = HistoryWorkspaceQueryRequest {
            scope: HistoryWorkspaceScope::All,
            query: "fa".to_string(), // 2 bytes — too short for trigram
            filter_type: HistoryWorkspaceFilterType::All,
            date_filter: HistoryWorkspaceDateFilter::All,
            sort_order: HistoryWorkspaceSortOrder::Newest,
            limit: 100,
            offset: 0,
        };
        let res_short_en = store.query_workspace(req_short_en).unwrap();
        assert_eq!(res_short_en.filtered_items.len(), 1);
        assert_eq!(res_short_en.filtered_items[0].id, item.id);
    }
