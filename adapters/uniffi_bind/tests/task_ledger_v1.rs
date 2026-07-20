use sona_uniffi_bind::{
    FfiStringPatchV1, FfiTaskLedgerKindV1, FfiTaskLedgerPatchV1, FfiTaskLedgerRecordV1,
    FfiTaskLedgerStatusV1, clear_resolved_task_ledger_records_v1, load_task_ledger_snapshot_v1,
    patch_task_ledger_record_v1, remove_task_ledger_record_v1, upsert_task_ledger_record_v1,
};

fn record(id: &str, status: FfiTaskLedgerStatusV1) -> FfiTaskLedgerRecordV1 {
    FfiTaskLedgerRecordV1 {
        id: id.to_string(),
        kind: FfiTaskLedgerKindV1::LlmPolish,
        status,
        title: "  Polish transcript  ".to_string(),
        progress: 25.0,
        created_at: 0,
        updated_at: 0,
        retryable: false,
        cancelable: true,
        recoverable: false,
        stage: None,
        history_id: None,
        tag_ids: Vec::new(),
        file_path: None,
        automation_rule_id: None,
        tag_automation_rule_id: None,
        automation_profile_id: None,
        automation_profile_source: None,
        source_fingerprint: None,
        error_message: None,
        template_id: None,
        target_language: None,
    }
}

fn empty_patch() -> FfiTaskLedgerPatchV1 {
    FfiTaskLedgerPatchV1 {
        kind: None,
        status: None,
        title: None,
        progress: None,
        created_at: None,
        updated_at: None,
        retryable: None,
        cancelable: None,
        recoverable: None,
        stage: FfiStringPatchV1::Unchanged,
        history_id: FfiStringPatchV1::Unchanged,
        tag_ids: None,
        file_path: FfiStringPatchV1::Unchanged,
        automation_rule_id: FfiStringPatchV1::Unchanged,
        tag_automation_rule_id: FfiStringPatchV1::Unchanged,
        automation_profile_id: FfiStringPatchV1::Unchanged,
        automation_profile_source: FfiStringPatchV1::Unchanged,
        source_fingerprint: FfiStringPatchV1::Unchanged,
        error_message: FfiStringPatchV1::Unchanged,
        template_id: FfiStringPatchV1::Unchanged,
        target_language: FfiStringPatchV1::Unchanged,
    }
}

#[test]
fn task_ledger_v1_preserves_patch_tristate_without_json() {
    let dir = tempfile::tempdir().unwrap();
    let app_data_dir = dir.path().to_string_lossy().into_owned();

    let empty = load_task_ledger_snapshot_v1(app_data_dir.clone()).unwrap();
    assert!(empty.tasks.is_empty());

    let inserted = upsert_task_ledger_record_v1(
        app_data_dir.clone(),
        record("  task-1  ", FfiTaskLedgerStatusV1::Pending),
    )
    .unwrap();
    assert_eq!(inserted.tasks[0].id, "task-1");
    assert_eq!(inserted.tasks[0].title, "Polish transcript");
    assert!(inserted.tasks[0].created_at > 0);

    let with_stage = patch_task_ledger_record_v1(
        app_data_dir.clone(),
        "task-1".to_string(),
        FfiTaskLedgerPatchV1 {
            progress: Some(75.0),
            stage: FfiStringPatchV1::Set {
                value: "transcribing".to_string(),
            },
            ..empty_patch()
        },
    )
    .unwrap();
    assert_eq!(with_stage.tasks[0].progress, 75.0);
    assert_eq!(with_stage.tasks[0].stage.as_deref(), Some("transcribing"));

    let cleared = patch_task_ledger_record_v1(
        app_data_dir.clone(),
        "task-1".to_string(),
        FfiTaskLedgerPatchV1 {
            stage: FfiStringPatchV1::Clear,
            ..empty_patch()
        },
    )
    .unwrap();
    assert_eq!(cleared.tasks[0].stage, None);
    assert_eq!(cleared.tasks[0].progress, 75.0);

    let removed = remove_task_ledger_record_v1(app_data_dir.clone(), "task-1".to_string()).unwrap();
    assert!(removed.tasks.is_empty());

    upsert_task_ledger_record_v1(
        app_data_dir.clone(),
        record("done", FfiTaskLedgerStatusV1::Succeeded),
    )
    .unwrap();
    let cleared_resolved = clear_resolved_task_ledger_records_v1(app_data_dir).unwrap();
    assert!(cleared_resolved.tasks.is_empty());
}
