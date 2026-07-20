use sona_uniffi_bind::{
    FfiHistoryCompleteLiveDraftRequestV1, FfiHistoryCreateLiveDraftRequestV1,
    FfiHistoryCreateTranscriptSnapshotRequestV1, FfiHistoryDeleteItemsRequestV1,
    FfiHistoryDraftSourcePatchV1, FfiHistoryDraftSourceV1, FfiHistoryItemMetaPatchV1,
    FfiHistoryItemStatusV1, FfiHistoryReplaceTagAssignmentsRequestV1,
    FfiHistorySaveImportedFileRequestV1, FfiHistorySaveRecordingRequestV1,
    FfiHistoryTrashItemsRequestV1, FfiHistoryUpdateItemMetaRequestV1,
    FfiHistoryUpdateTagAssignmentsRequestV1, FfiHistoryUpdateTranscriptRequestV1,
    FfiHistoryWorkspaceDateFilterV1, FfiHistoryWorkspaceFilterTypeV1,
    FfiHistoryWorkspaceQueryRequestV1, FfiHistoryWorkspaceScopeV1, FfiHistoryWorkspaceSortOrderV1,
    FfiSpeakerAttribution, FfiSpeakerCandidate, FfiSpeakerTag, FfiStringPatchV1,
    FfiTagCreateInputV1, FfiTranscriptSegment, FfiTranscriptSnapshotReasonV1, FfiTranscriptTiming,
    FfiTranscriptTimingLevel, FfiTranscriptTimingSource, FfiTranscriptTimingUnit,
    SonaCoreBindingError, complete_history_live_draft_v1, create_history_live_draft_v1,
    create_history_transcript_snapshot_v1, create_tag_v1, list_history_items_v1,
    list_history_transcript_snapshots_v1, load_history_transcript_snapshot_v1,
    load_history_transcript_v1, purge_history_items_v1, query_history_workspace_v1,
    replace_history_tag_assignments_v1, restore_history_items_v1, save_history_imported_file_v1,
    save_history_recording_v1, trash_history_items_v1, update_history_item_meta_v1,
    update_history_tag_assignments_v1, update_history_transcript_v1,
};

fn segment() -> FfiTranscriptSegment {
    FfiTranscriptSegment {
        id: "segment-1".to_string(),
        text: "Hello mobile history".to_string(),
        start: 0.0,
        end: 1.5,
        is_final: true,
        timing: Some(FfiTranscriptTiming {
            level: FfiTranscriptTimingLevel::Token,
            source: FfiTranscriptTimingSource::Model,
            units: vec![FfiTranscriptTimingUnit {
                text: "Hello".to_string(),
                start: 0.0,
                end: 0.5,
            }],
        }),
        tokens: Some(vec!["Hello".to_string(), "mobile".to_string()]),
        timestamps: Some(vec![0.0, 0.5]),
        durations: Some(vec![0.5, 0.5]),
        translation: Some("Hola".to_string()),
        speaker: Some(FfiSpeakerTag {
            id: "speaker-1".to_string(),
            label: "Speaker 1".to_string(),
            kind: "profile".to_string(),
            score: Some(0.95),
        }),
        speaker_attribution: Some(FfiSpeakerAttribution {
            group_id: "group-1".to_string(),
            anonymous_label: "Speaker 1".to_string(),
            state: "matched".to_string(),
            source: "embedding".to_string(),
            confidence: "high".to_string(),
            candidates: vec![FfiSpeakerCandidate {
                profile_id: "profile-1".to_string(),
                profile_name: "Alice".to_string(),
                score: 0.9,
                rank: 1,
            }],
        }),
    }
}

fn query(limit: u64) -> FfiHistoryWorkspaceQueryRequestV1 {
    FfiHistoryWorkspaceQueryRequestV1 {
        scope: FfiHistoryWorkspaceScopeV1::All,
        query: "mobile".to_string(),
        filter_type: FfiHistoryWorkspaceFilterTypeV1::Recording,
        date_filter: FfiHistoryWorkspaceDateFilterV1::All,
        sort_order: FfiHistoryWorkspaceSortOrderV1::Newest,
        limit,
        offset: 0,
    }
}

fn empty_meta_patch() -> FfiHistoryItemMetaPatchV1 {
    FfiHistoryItemMetaPatchV1 {
        timestamp: None,
        duration: None,
        audio_path: None,
        audio_status: None,
        transcript_path: None,
        title: None,
        preview_text: None,
        icon: FfiStringPatchV1::Unchanged,
        kind: None,
        search_content: None,
        status: None,
        draft_source: FfiHistoryDraftSourcePatchV1::Unchanged,
    }
}

fn tag_input(name: &str) -> FfiTagCreateInputV1 {
    FfiTagCreateInputV1 {
        name: name.to_string(),
        description: None,
        icon: None,
        color: None,
    }
}

#[tokio::test]
async fn history_v1_covers_the_android_recording_and_library_lifecycle() {
    let dir = tempfile::tempdir().unwrap();
    let app_data_dir = dir.path().to_string_lossy().into_owned();

    let draft = create_history_live_draft_v1(
        app_data_dir.clone(),
        FfiHistoryCreateLiveDraftRequestV1 {
            id: Some("recording-1".to_string()),
            audio_extension: "wav".to_string(),
            tag_ids: Vec::new(),
            icon: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(draft.item.id, "recording-1");
    assert_eq!(draft.item.status, FfiHistoryItemStatusV1::Draft);
    assert!(draft.audio_absolute_path.ends_with("recording-1.wav"));

    let updated = update_history_transcript_v1(
        app_data_dir.clone(),
        FfiHistoryUpdateTranscriptRequestV1 {
            history_id: draft.item.id.clone(),
            segments: vec![segment()],
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.preview_text, "Hello mobile history...");

    let completed = complete_history_live_draft_v1(
        app_data_dir.clone(),
        FfiHistoryCompleteLiveDraftRequestV1 {
            history_id: draft.item.id.clone(),
            segments: vec![segment()],
            duration: 1.5,
        },
    )
    .await
    .unwrap();
    assert_eq!(completed.status, FfiHistoryItemStatusV1::Complete);

    let workspace = query_history_workspace_v1(app_data_dir.clone(), query(20))
        .await
        .unwrap();
    assert_eq!(workspace.filtered_items.len(), 1);
    assert_eq!(workspace.filtered_items[0].id, draft.item.id);
    assert_eq!(workspace.filtered_item_count, 1);
    assert!(!workspace.has_more);
    assert_eq!(workspace.summary.recording_count, 1);

    let transcript = load_history_transcript_v1(app_data_dir.clone(), draft.item.id.clone())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(transcript.len(), 1);
    assert_eq!(transcript[0].id, "segment-1");
    assert_eq!(transcript[0].text, "Hello mobile history");
    assert_eq!(transcript[0].timing.as_ref().unwrap().units[0].end, 1.5);
    assert_eq!(transcript[0].speaker.as_ref().unwrap().id, "speaker-1");
    assert_eq!(
        transcript[0]
            .speaker_attribution
            .as_ref()
            .unwrap()
            .candidates[0]
            .profile_name,
        "Alice"
    );

    let discarded_draft = create_history_live_draft_v1(
        app_data_dir.clone(),
        FfiHistoryCreateLiveDraftRequestV1 {
            id: Some("draft-to-delete".to_string()),
            audio_extension: "wav".to_string(),
            tag_ids: Vec::new(),
            icon: None,
        },
    )
    .await
    .unwrap();
    purge_history_items_v1(
        app_data_dir.clone(),
        FfiHistoryDeleteItemsRequestV1 {
            ids: vec![discarded_draft.item.id.clone()],
        },
    )
    .await
    .unwrap();
    let missing_error = load_history_transcript_v1(app_data_dir.clone(), discarded_draft.item.id)
        .await
        .unwrap_err();
    assert!(matches!(
        missing_error,
        SonaCoreBindingError::HistoryQuery { .. }
    ));
    assert_eq!(
        query_history_workspace_v1(app_data_dir, query(20))
            .await
            .unwrap()
            .filtered_items
            .len(),
        1
    );
}

#[tokio::test]
async fn history_v1_validates_pagination_before_app_data_access() {
    let root = tempfile::tempdir().unwrap();
    let missing = root.path().join("missing");

    let error = query_history_workspace_v1(missing.to_string_lossy().into_owned(), query(u64::MAX))
        .await
        .unwrap_err();

    assert!(matches!(error, SonaCoreBindingError::InvalidInput { .. }));
    assert!(!missing.exists());
    assert!(!root.path().join("sona.db").exists());
}

#[tokio::test]
async fn history_v1_exposes_typed_lists_and_transcript_snapshots() {
    let dir = tempfile::tempdir().unwrap();
    let app_data_dir = dir.path().to_string_lossy().into_owned();

    let item = save_history_recording_v1(
        app_data_dir.clone(),
        FfiHistorySaveRecordingRequestV1 {
            segments: vec![segment()],
            duration: 1.5,
            tag_ids: Vec::new(),
            audio_bytes: Some(vec![1, 2, 3]),
            native_audio_path: None,
            audio_extension: Some("wav".to_string()),
        },
    )
    .await
    .unwrap();
    let snapshot = create_history_transcript_snapshot_v1(
        app_data_dir.clone(),
        FfiHistoryCreateTranscriptSnapshotRequestV1 {
            history_id: item.id.clone(),
            reason: FfiTranscriptSnapshotReasonV1::Polish,
            segments: vec![segment()],
        },
    )
    .await
    .unwrap();

    let items = list_history_items_v1(app_data_dir.clone(), Some(20), Some(0))
        .await
        .unwrap();
    let snapshots = list_history_transcript_snapshots_v1(app_data_dir.clone(), item.id.clone())
        .await
        .unwrap();
    let loaded = load_history_transcript_snapshot_v1(
        app_data_dir.clone(),
        item.id.clone(),
        snapshot.id.clone(),
    )
    .await
    .unwrap()
    .unwrap();
    let missing = load_history_transcript_snapshot_v1(
        app_data_dir,
        item.id.clone(),
        "missing-snapshot".to_string(),
    )
    .await
    .unwrap();

    assert_eq!(items[0].id, item.id);
    assert_eq!(snapshots, vec![snapshot.clone()]);
    assert_eq!(loaded.metadata, snapshot);
    assert_eq!(loaded.segments[0].text, "Hello mobile history");
    assert!(missing.is_none());
}

#[tokio::test]
async fn history_v1_covers_canonical_save_meta_tag_and_trash_mutations() {
    let dir = tempfile::tempdir().unwrap();
    let app_data_dir = dir.path().to_string_lossy().into_owned();
    let imported_source = dir.path().join("imported.wav");
    std::fs::write(&imported_source, [4, 5, 6]).unwrap();
    let tag_a = create_tag_v1(app_data_dir.clone(), tag_input("Tag A")).unwrap();
    let tag_b = create_tag_v1(app_data_dir.clone(), tag_input("Tag B")).unwrap();
    let tag_c = create_tag_v1(app_data_dir.clone(), tag_input("Tag C")).unwrap();

    let draft = create_history_live_draft_v1(
        app_data_dir.clone(),
        FfiHistoryCreateLiveDraftRequestV1 {
            id: Some("meta-draft".to_string()),
            audio_extension: "wav".to_string(),
            tag_ids: Vec::new(),
            icon: None,
        },
    )
    .await
    .unwrap();
    let imported = save_history_imported_file_v1(
        app_data_dir.clone(),
        FfiHistorySaveImportedFileRequestV1 {
            id: Some("imported-1".to_string()),
            source_path: imported_source.to_string_lossy().into_owned(),
            segments: vec![segment()],
            duration: 1.5,
            tag_ids: Vec::new(),
            converted_source_path: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(imported.id, "imported-1");

    let mut set_meta = empty_meta_patch();
    set_meta.title = Some("Typed title".to_string());
    set_meta.icon = FfiStringPatchV1::Set {
        value: "microphone".to_string(),
    };
    set_meta.draft_source = FfiHistoryDraftSourcePatchV1::Clear;
    update_history_item_meta_v1(
        app_data_dir.clone(),
        FfiHistoryUpdateItemMetaRequestV1 {
            history_id: draft.item.id.clone(),
            updates: set_meta,
        },
    )
    .await
    .unwrap();
    update_history_tag_assignments_v1(
        app_data_dir.clone(),
        FfiHistoryUpdateTagAssignmentsRequestV1 {
            ids: vec![draft.item.id.clone()],
            add_tag_ids: vec![tag_a.id, tag_b.id],
            remove_tag_ids: Vec::new(),
        },
    )
    .await
    .unwrap();
    replace_history_tag_assignments_v1(
        app_data_dir.clone(),
        FfiHistoryReplaceTagAssignmentsRequestV1 {
            ids: vec![draft.item.id.clone()],
            tag_ids: vec![tag_c.id.clone()],
        },
    )
    .await
    .unwrap();

    let updated = list_history_items_v1(app_data_dir.clone(), None, None)
        .await
        .unwrap()
        .into_iter()
        .find(|item| item.id == draft.item.id)
        .unwrap();
    assert_eq!(updated.title, "Typed title");
    assert_eq!(updated.icon.as_deref(), Some("microphone"));
    assert_eq!(updated.draft_source, None);
    assert_eq!(updated.tag_ids, vec![tag_c.id]);

    let mut clear_meta = empty_meta_patch();
    clear_meta.icon = FfiStringPatchV1::Clear;
    clear_meta.draft_source = FfiHistoryDraftSourcePatchV1::Set {
        value: FfiHistoryDraftSourceV1::LiveRecord,
    };
    update_history_item_meta_v1(
        app_data_dir.clone(),
        FfiHistoryUpdateItemMetaRequestV1 {
            history_id: draft.item.id.clone(),
            updates: clear_meta,
        },
    )
    .await
    .unwrap();

    trash_history_items_v1(
        app_data_dir.clone(),
        FfiHistoryTrashItemsRequestV1 {
            ids: vec![draft.item.id.clone()],
            deleted_at: 1_725_000_000_000,
        },
    )
    .await
    .unwrap();
    let mut trash_query = query(20);
    trash_query.scope = FfiHistoryWorkspaceScopeV1::Trash;
    trash_query.query.clear();
    assert_eq!(
        query_history_workspace_v1(app_data_dir.clone(), trash_query)
            .await
            .unwrap()
            .filtered_items[0]
            .id,
        draft.item.id
    );
    restore_history_items_v1(
        app_data_dir.clone(),
        FfiHistoryDeleteItemsRequestV1 {
            ids: vec![draft.item.id.clone()],
        },
    )
    .await
    .unwrap();

    let restored = list_history_items_v1(app_data_dir, None, None)
        .await
        .unwrap()
        .into_iter()
        .find(|item| item.id == draft.item.id)
        .unwrap();
    assert_eq!(restored.icon, None);
    assert_eq!(
        restored.draft_source,
        Some(FfiHistoryDraftSourceV1::LiveRecord)
    );
    assert_eq!(restored.deleted_at, None);
}

#[tokio::test]
async fn history_v1_rejects_invalid_mutations_without_database_or_partial_writes() {
    let pristine = tempfile::tempdir().unwrap();
    let pristine_path = pristine.path().to_string_lossy().into_owned();
    let error = save_history_recording_v1(
        pristine_path,
        FfiHistorySaveRecordingRequestV1 {
            segments: vec![segment()],
            duration: f64::NAN,
            tag_ids: Vec::new(),
            audio_bytes: Some(vec![1]),
            native_audio_path: None,
            audio_extension: Some("wav".to_string()),
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(error, SonaCoreBindingError::InvalidInput { .. }));
    assert!(!pristine.path().join("sona.db").exists());

    let dir = tempfile::tempdir().unwrap();
    let app_data_dir = dir.path().to_string_lossy().into_owned();
    let draft = create_history_live_draft_v1(
        app_data_dir.clone(),
        FfiHistoryCreateLiveDraftRequestV1 {
            id: Some("no-partial-meta".to_string()),
            audio_extension: "wav".to_string(),
            tag_ids: Vec::new(),
            icon: None,
        },
    )
    .await
    .unwrap();
    let before = draft.item.title;
    let mut invalid_patch = empty_meta_patch();
    invalid_patch.title = Some("must not persist".to_string());
    invalid_patch.audio_path = Some("../outside.wav".to_string());
    let error = update_history_item_meta_v1(
        app_data_dir.clone(),
        FfiHistoryUpdateItemMetaRequestV1 {
            history_id: draft.item.id.clone(),
            updates: invalid_patch,
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(error, SonaCoreBindingError::InvalidInput { .. }));

    let after = list_history_items_v1(app_data_dir, None, None)
        .await
        .unwrap()
        .into_iter()
        .find(|item| item.id == draft.item.id)
        .unwrap();
    assert_eq!(after.title, before);
}
