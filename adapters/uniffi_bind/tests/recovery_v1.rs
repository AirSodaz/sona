use sona_uniffi_bind::{
    FfiRecoveredTranscriptSegmentV1, FfiRecoveredTranscriptTimingUnitV1,
    FfiRecoveredTranscriptTimingV1, FfiRecoveryFileStatV1, FfiRecoveryItemInputV1,
    FfiRecoveryItemStageV1, FfiRecoveryQueueStatusV1, FfiRecoveryResolutionV1, FfiRecoverySourceV1,
    FfiSpeakerAttribution, FfiSpeakerCandidate, FfiSpeakerTag, FfiTranscriptTimingLevel,
    FfiTranscriptTimingSource, SonaCoreBindingError, load_recovery_snapshot_v1,
    persist_recovery_queue_snapshot_v1, save_recovery_snapshot_v1,
};

fn segment() -> FfiRecoveredTranscriptSegmentV1 {
    FfiRecoveredTranscriptSegmentV1 {
        id: "segment-1".to_string(),
        text: "hello".to_string(),
        start: 1.0,
        end: 2.0,
        is_final: true,
        timing: Some(FfiRecoveredTranscriptTimingV1 {
            level: FfiTranscriptTimingLevel::Token,
            source: FfiTranscriptTimingSource::Model,
            units: vec![FfiRecoveredTranscriptTimingUnitV1 {
                text: "hello".to_string(),
                start: 1.0,
                end: 2.0,
            }],
        }),
        tokens: Some(vec!["hello".to_string()]),
        timestamps: Some(vec![1.0]),
        durations: Some(vec![1.0]),
        translation: Some("hola".to_string()),
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

fn saved_item(id: &str) -> FfiRecoveryItemInputV1 {
    FfiRecoveryItemInputV1 {
        id: Some(id.to_string()),
        recovery_id: None,
        filename: Some(format!("{id}.wav")),
        file_path: Some(String::new()),
        source: Some(FfiRecoverySourceV1::BatchImport),
        origin: None,
        resolution: Some(FfiRecoveryResolutionV1::Pending),
        status: None,
        progress: Some(25.0),
        segments: vec![segment()],
        tag_ids: vec!["tag-1".to_string()],
        project_id: None,
        history_id: Some("history-1".to_string()),
        history_title: Some("History".to_string()),
        last_known_stage: Some(FfiRecoveryItemStageV1::Transcribing),
        updated_at: None,
        has_source_file: Some(true),
        can_resume: Some(true),
        automation_rule_id: None,
        automation_rule_name: None,
        resolved_config_snapshot_json: Some(r#"{"z":1,"a":2}"#.to_string()),
        export_config_json: Some(r#"{"format":"txt"}"#.to_string()),
        stage_config_json: Some(r#"{"chunk":1}"#.to_string()),
        source_fingerprint: Some("fingerprint".to_string()),
        file_stat: Some(FfiRecoveryFileStatV1 {
            size: 1024,
            mtime_ms: 2_000,
        }),
        export_file_name_prefix: Some("meeting".to_string()),
    }
}

#[test]
fn recovery_v1_round_trips_typed_snapshot_and_canonical_dynamic_json_leaves() {
    let dir = tempfile::tempdir().unwrap();
    let app_data_dir = dir.path().to_string_lossy().into_owned();

    let saved =
        save_recovery_snapshot_v1(app_data_dir.clone(), vec![saved_item("recovery-1")]).unwrap();

    assert_eq!(saved.items.len(), 1);
    let item = &saved.items[0];
    assert_eq!(item.id, "recovery-1");
    assert_eq!(item.source, FfiRecoverySourceV1::BatchImport);
    assert_eq!(item.resolution, FfiRecoveryResolutionV1::Pending);
    assert_eq!(item.last_known_stage, FfiRecoveryItemStageV1::Transcribing);
    assert_eq!(
        item.resolved_config_snapshot_json.as_deref(),
        Some(r#"{"a":2,"z":1}"#)
    );
    assert_eq!(item.export_config_json, r#"{"format":"txt"}"#);
    assert_eq!(item.stage_config_json, r#"{"chunk":1}"#);
    assert_eq!(item.file_stat.as_ref().unwrap().mtime_ms, 2_000);
    assert_eq!(
        item.segments[0].timing.as_ref().unwrap().units[0].text,
        "hello"
    );
    assert_eq!(item.segments[0].speaker.as_ref().unwrap().id, "speaker-1");
    assert_eq!(
        item.segments[0]
            .speaker_attribution
            .as_ref()
            .unwrap()
            .candidates[0]
            .profile_name,
        "Alice"
    );

    let loaded = load_recovery_snapshot_v1(app_data_dir).unwrap();
    assert_eq!(loaded, saved);
}

#[test]
fn recovery_v1_persists_typed_queue_items() {
    let dir = tempfile::tempdir().unwrap();
    let app_data_dir = dir.path().to_string_lossy().into_owned();
    let mut item = saved_item("queue-1");
    item.recovery_id = Some("recovery-alias".to_string());
    item.source = None;
    item.origin = Some(FfiRecoverySourceV1::Automation);
    item.resolution = None;
    item.status = Some(FfiRecoveryQueueStatusV1::Processing);

    let snapshot =
        persist_recovery_queue_snapshot_v1(app_data_dir, vec![item], Vec::new()).unwrap();

    assert_eq!(snapshot.items[0].id, "recovery-alias");
    assert_eq!(snapshot.items[0].source, FfiRecoverySourceV1::Automation);
    assert_eq!(
        snapshot.items[0].resolution,
        FfiRecoveryResolutionV1::Pending
    );
}

#[test]
fn recovery_v1_rejects_malformed_dynamic_json_before_writing() {
    let dir = tempfile::tempdir().unwrap();
    let app_data_dir = dir.path().to_string_lossy().into_owned();
    save_recovery_snapshot_v1(app_data_dir.clone(), vec![saved_item("retained")]).unwrap();

    let mut invalid = saved_item("replacement");
    invalid.stage_config_json = Some("{".to_string());
    let error = save_recovery_snapshot_v1(app_data_dir.clone(), vec![invalid]).unwrap_err();

    assert!(matches!(error, SonaCoreBindingError::InvalidInput { .. }));
    assert!(error.to_string().contains("stage_config_json"));
    let loaded = load_recovery_snapshot_v1(app_data_dir).unwrap();
    assert_eq!(
        loaded
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["retained"]
    );
}
