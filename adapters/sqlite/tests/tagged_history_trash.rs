use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use sona_core::history::mutation_repository::{
    HistoryCreateTranscriptSnapshotRequest, HistoryDeleteItemsRequest, HistoryMutationRepository,
    HistoryTrashItemsRequest,
};
use sona_core::history::query_repository::HistoryQueryRepository;
use sona_core::history::{
    HistoryCreateLiveDraftRequest, HistoryIdGenerator, HistorySaveRecordingRequest,
    HistoryWorkspaceDateFilter, HistoryWorkspaceFilterType, HistoryWorkspaceQueryRequest,
    HistoryWorkspaceQueryResult, HistoryWorkspaceScope, HistoryWorkspaceSortOrder,
    TranscriptSnapshotReason,
};
use sona_core::ports::time::{ClockError, UnixMillisClock};
use sona_core::sync::{SyncLocalRepository, SyncPresetV1};
use sona_core::tag::{TagDefaults, TagRecord, TagStore};
use sona_core::transcription::transcript::TranscriptSegment;
use sona_sqlite::{Database, SqliteHistoryStore, SqliteSyncRepository, SqliteTagRepository};

struct TestClock;

impl UnixMillisClock for TestClock {
    fn now_ms(&self) -> Result<u64, ClockError> {
        Ok(1_700_000_000_000)
    }
}

struct TestIds;

impl HistoryIdGenerator for TestIds {
    fn generate_id(&self) -> String {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        format!("history-test-{}", NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

fn history_store(root: &std::path::Path, db: Arc<Database>) -> SqliteHistoryStore {
    SqliteHistoryStore::with_environment(
        root.to_path_buf(),
        db,
        Arc::new(TestClock),
        Arc::new(TestIds),
    )
}

fn tag(id: &str, sort_order: usize) -> TagRecord {
    TagRecord {
        id: id.to_string(),
        name: id.to_string(),
        description: String::new(),
        icon: "Tag".to_string(),
        color: "#2563EB".to_string(),
        sort_order,
        created_at: 1,
        updated_at: 1,
        defaults: TagDefaults::default(),
    }
}

fn segment(text: &str) -> TranscriptSegment {
    TranscriptSegment {
        id: format!("segment-{text}"),
        text: text.to_string(),
        start: 0.0,
        end: 1.0,
        is_final: true,
        timing: None,
        tokens: None,
        timestamps: None,
        durations: None,
        translation: None,
        speaker: None,
        speaker_attribution: None,
    }
}

fn query(store: &SqliteHistoryStore, scope: HistoryWorkspaceScope) -> HistoryWorkspaceQueryResult {
    HistoryQueryRepository::query_workspace(
        store,
        HistoryWorkspaceQueryRequest {
            scope,
            query: String::new(),
            filter_type: HistoryWorkspaceFilterType::All,
            date_filter: HistoryWorkspaceDateFilter::All,
            sort_order: HistoryWorkspaceSortOrder::Newest,
            limit: 200,
            offset: 0,
        },
    )
    .unwrap()
}

#[test]
fn multi_tag_scopes_and_trash_lifecycle_preserve_then_purge_children_and_audio() {
    let root = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(root.path()).unwrap());
    SqliteTagRepository::new(Arc::clone(&db))
        .replace_tags(vec![tag("tag-priority", 0), tag("tag-secondary", 1)])
        .unwrap();
    let store = history_store(root.path(), Arc::clone(&db));

    let tagged = HistoryMutationRepository::save_recording(
        &store,
        HistorySaveRecordingRequest {
            segments: vec![segment("tagged")],
            duration: 1.0,
            tag_ids: vec!["tag-secondary".to_string(), "tag-priority".to_string()],
            audio_bytes: Some(vec![1, 2, 3, 4]),
            native_audio_path: None,
            audio_extension: Some("wav".to_string()),
        },
    )
    .unwrap();
    HistoryMutationRepository::create_transcript_snapshot(
        &store,
        HistoryCreateTranscriptSnapshotRequest {
            history_id: tagged.id.clone(),
            reason: TranscriptSnapshotReason::Polish,
            segments: vec![segment("snapshot")],
        },
    )
    .unwrap();
    HistoryMutationRepository::save_recording(
        &store,
        HistorySaveRecordingRequest {
            segments: vec![segment("untagged")],
            duration: 2.0,
            tag_ids: Vec::new(),
            audio_bytes: Some(vec![5, 6]),
            native_audio_path: None,
            audio_extension: Some("wav".to_string()),
        },
    )
    .unwrap();

    let all = query(&store, HistoryWorkspaceScope::All);
    assert_eq!(all.filtered_item_count, 2);
    assert_eq!(all.item_counts.untagged, 1);
    assert_eq!(all.item_counts.trash, 0);
    assert_eq!(all.item_counts.by_tag_id["tag-priority"], 1);
    assert_eq!(all.item_counts.by_tag_id["tag-secondary"], 1);
    let reloaded = all
        .filtered_items
        .iter()
        .find(|item| item.id == tagged.id)
        .unwrap();
    assert_eq!(reloaded.tag_ids, ["tag-priority", "tag-secondary"]);
    assert_eq!(
        query(&store, HistoryWorkspaceScope::Untagged).filtered_item_count,
        1
    );
    assert_eq!(
        query(
            &store,
            HistoryWorkspaceScope::Tag {
                tag_id: "tag-secondary".to_string(),
            },
        )
        .filtered_item_count,
        1
    );

    let audio_path = root.path().join("history").join(&tagged.audio_path);
    assert!(audio_path.is_file());
    HistoryMutationRepository::trash_items(
        &store,
        HistoryTrashItemsRequest {
            ids: vec![tagged.id.clone()],
            deleted_at: 900,
        },
    )
    .unwrap();

    assert_eq!(
        query(&store, HistoryWorkspaceScope::All).filtered_item_count,
        1
    );
    let trash = query(&store, HistoryWorkspaceScope::Trash);
    assert_eq!(trash.filtered_item_count, 1);
    assert_eq!(trash.filtered_items[0].deleted_at, Some(900));
    assert_eq!(
        trash.filtered_items[0].tag_ids,
        ["tag-priority", "tag-secondary"]
    );
    assert!(audio_path.is_file());
    assert_eq!(
        HistoryQueryRepository::load_transcript(&store, &tagged.id)
            .unwrap()
            .unwrap()[0]
            .text,
        "tagged"
    );
    assert_eq!(
        HistoryQueryRepository::list_transcript_snapshots(&store, &tagged.id)
            .unwrap()
            .len(),
        1
    );

    HistoryMutationRepository::restore_items(
        &store,
        HistoryDeleteItemsRequest {
            ids: vec![tagged.id.clone()],
        },
    )
    .unwrap();
    let restored = query(
        &store,
        HistoryWorkspaceScope::Tag {
            tag_id: "tag-priority".to_string(),
        },
    );
    assert_eq!(restored.filtered_item_count, 1);
    assert_eq!(restored.filtered_items[0].deleted_at, None);

    HistoryMutationRepository::trash_items(
        &store,
        HistoryTrashItemsRequest {
            ids: vec![tagged.id.clone()],
            deleted_at: 901,
        },
    )
    .unwrap();
    HistoryMutationRepository::purge_items(
        &store,
        HistoryDeleteItemsRequest {
            ids: vec![tagged.id.clone()],
        },
    )
    .unwrap();

    assert_eq!(
        query(&store, HistoryWorkspaceScope::Trash).filtered_item_count,
        0
    );
    assert!(!audio_path.exists());
    let (transcript_count, snapshot_count) = db
        .with_read_connection(|connection| {
            let transcript_count = connection.query_row(
                "SELECT COUNT(*) FROM history_transcripts WHERE history_id = ?1",
                [&tagged.id],
                |row| row.get::<_, i64>(0),
            )?;
            let snapshot_count = connection.query_row(
                "SELECT COUNT(*) FROM transcript_snapshots WHERE history_id = ?1",
                [&tagged.id],
                |row| row.get::<_, i64>(0),
            )?;
            Ok((transcript_count, snapshot_count))
        })
        .unwrap();
    assert_eq!(transcript_count, 0);
    assert_eq!(snapshot_count, 0);
}

#[test]
fn purging_a_live_draft_removes_its_audio_without_purging_active_history() {
    let root = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(root.path()).unwrap());
    let store = history_store(root.path(), db);
    let draft = HistoryMutationRepository::create_live_draft(
        &store,
        HistoryCreateLiveDraftRequest {
            id: Some("live-draft".to_string()),
            audio_extension: "wav".to_string(),
            tag_ids: Vec::new(),
            icon: None,
        },
    )
    .unwrap();
    std::fs::write(&draft.audio_absolute_path, [1, 2, 3]).unwrap();
    let active = HistoryMutationRepository::save_recording(
        &store,
        HistorySaveRecordingRequest {
            segments: vec![segment("active")],
            duration: 1.0,
            tag_ids: Vec::new(),
            audio_bytes: Some(vec![4, 5, 6]),
            native_audio_path: None,
            audio_extension: Some("wav".to_string()),
        },
    )
    .unwrap();

    HistoryMutationRepository::purge_items(
        &store,
        HistoryDeleteItemsRequest {
            ids: vec![draft.item.id.clone(), active.id.clone()],
        },
    )
    .unwrap();

    assert!(!std::path::Path::new(&draft.audio_absolute_path).exists());
    let all = query(&store, HistoryWorkspaceScope::All);
    assert_eq!(all.filtered_item_count, 1);
    assert_eq!(all.filtered_items[0].id, active.id);
}

#[test]
fn trashing_unknown_or_already_trashed_items_does_not_enqueue_sync() {
    let root = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(root.path()).unwrap());
    let sync = SqliteSyncRepository::initialize(
        Arc::clone(&db),
        Arc::new(TestClock),
        "vault-a",
        "device-a",
        SyncPresetV1::Standard,
    )
    .unwrap();
    let store = history_store(root.path(), db);
    let item = HistoryMutationRepository::save_recording(
        &store,
        HistorySaveRecordingRequest {
            segments: vec![segment("sync")],
            duration: 1.0,
            tag_ids: Vec::new(),
            audio_bytes: Some(vec![1]),
            native_audio_path: None,
            audio_extension: Some("wav".to_string()),
        },
    )
    .unwrap();
    let pending_count = || {
        sync.load_pending_operations(SyncPresetV1::Standard, 256, usize::MAX)
            .unwrap()
            .len()
    };

    let before_unknown = pending_count();
    HistoryMutationRepository::trash_items(
        &store,
        HistoryTrashItemsRequest {
            ids: vec!["missing".to_string()],
            deleted_at: 500,
        },
    )
    .unwrap();
    assert_eq!(pending_count(), before_unknown);

    HistoryMutationRepository::trash_items(
        &store,
        HistoryTrashItemsRequest {
            ids: vec![item.id.clone()],
            deleted_at: 501,
        },
    )
    .unwrap();
    let after_first_trash = pending_count();
    HistoryMutationRepository::trash_items(
        &store,
        HistoryTrashItemsRequest {
            ids: vec![item.id],
            deleted_at: 502,
        },
    )
    .unwrap();
    assert_eq!(pending_count(), after_first_trash);
}

#[test]
fn restoring_unknown_or_active_items_does_not_enqueue_sync() {
    let root = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(root.path()).unwrap());
    let sync = SqliteSyncRepository::initialize(
        Arc::clone(&db),
        Arc::new(TestClock),
        "vault-a",
        "device-a",
        SyncPresetV1::Standard,
    )
    .unwrap();
    let store = history_store(root.path(), db);
    let item = HistoryMutationRepository::save_recording(
        &store,
        HistorySaveRecordingRequest {
            segments: vec![segment("active-sync")],
            duration: 1.0,
            tag_ids: Vec::new(),
            audio_bytes: Some(vec![1]),
            native_audio_path: None,
            audio_extension: Some("wav".to_string()),
        },
    )
    .unwrap();
    let pending_count = || {
        sync.load_pending_operations(SyncPresetV1::Standard, 256, usize::MAX)
            .unwrap()
            .len()
    };

    let before_restore = pending_count();
    HistoryMutationRepository::restore_items(
        &store,
        HistoryDeleteItemsRequest {
            ids: vec!["missing".to_string(), item.id],
        },
    )
    .unwrap();

    assert_eq!(pending_count(), before_restore);
}

#[test]
fn deleting_tags_keeps_history_and_moves_items_without_tags_to_untagged() {
    let root = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(root.path()).unwrap());
    let tags = SqliteTagRepository::new(Arc::clone(&db));
    tags.replace_tags(vec![tag("tag-a", 0), tag("tag-b", 1)])
        .unwrap();
    let store = history_store(root.path(), db);
    let item = HistoryMutationRepository::save_recording(
        &store,
        HistorySaveRecordingRequest {
            segments: vec![segment("kept")],
            duration: 1.0,
            tag_ids: vec!["tag-a".to_string(), "tag-b".to_string()],
            audio_bytes: Some(vec![1]),
            native_audio_path: None,
            audio_extension: Some("wav".to_string()),
        },
    )
    .unwrap();

    tags.delete_tag("tag-a").unwrap();
    let after_first_delete = query(&store, HistoryWorkspaceScope::All);
    assert_eq!(after_first_delete.filtered_item_count, 1);
    assert_eq!(after_first_delete.filtered_items[0].id, item.id);
    assert_eq!(after_first_delete.filtered_items[0].tag_ids, ["tag-b"]);

    tags.delete_tag("tag-b").unwrap();
    let untagged = query(&store, HistoryWorkspaceScope::Untagged);
    assert_eq!(untagged.filtered_item_count, 1);
    assert_eq!(untagged.filtered_items[0].id, item.id);
    assert!(untagged.filtered_items[0].tag_ids.is_empty());
}
