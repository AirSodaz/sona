use serde_json::json;
use sona_core::sync::{
    HybridLogicalClock, SyncCausalContext, SyncConflictDetail, SyncConflictKind,
    SyncConflictResolution, SyncConflictSummary, SyncEntityKey, SyncEntityKind, SyncJoinPreview,
    SyncLifecycleState, SyncOperation, SyncOperationKind, SyncPresetV1, SyncProviderDescriptor,
    SyncRunResult, SyncStatusSnapshot, SyncVersion,
};

#[test]
fn status_contract_uses_stable_camel_case_json() {
    let status = SyncStatusSnapshot {
        state: SyncLifecycleState::Idle,
        provider_id: Some("webdav".to_string()),
        vault_id: Some("vault-1".to_string()),
        preset: Some(SyncPresetV1::Standard),
        last_success_at_ms: Some(123),
        pending_operation_count: 4,
        conflict_count: 2,
        next_retry_at_ms: None,
        last_error: None,
    };

    assert_eq!(
        serde_json::to_value(status).unwrap(),
        json!({
            "state": "idle",
            "providerId": "webdav",
            "vaultId": "vault-1",
            "preset": "standard",
            "lastSuccessAtMs": 123,
            "pendingOperationCount": 4,
            "conflictCount": 2,
            "nextRetryAtMs": null,
            "lastError": null
        })
    );
}

#[test]
fn run_and_join_contracts_report_domain_counts_without_payloads() {
    let run = SyncRunResult {
        pulled_segment_count: 2,
        pulled_checkpoint_count: 1,
        pushed_segment_count: 1,
        applied_operation_count: 7,
        published_operation_count: 3,
        conflict_count: 1,
        checkpoint_published: false,
    };
    let preview = SyncJoinPreview {
        local_operation_count: 3,
        remote_operation_count: 7,
        projected_conflict_count: 1,
    };

    assert_eq!(serde_json::to_value(run).unwrap()["conflictCount"], 1);
    assert_eq!(
        serde_json::to_value(preview).unwrap()["projectedConflictCount"],
        1
    );
}

#[test]
fn conflict_detail_contract_preserves_both_versions() {
    let operation = |id: &str, value: &str| SyncOperation {
        operation_id: id.to_string(),
        source_device_id: "device-a".to_string(),
        source_sequence: 1,
        causal_context: SyncCausalContext::default(),
        version: SyncVersion {
            clock: HybridLogicalClock {
                physical_ms: 123,
                logical: 0,
            },
            device_id: "device-a".to_string(),
            operation_id: id.to_string(),
        },
        entity: SyncEntityKey {
            kind: SyncEntityKind::Project,
            id: "project-1".to_string(),
        },
        kind: SyncOperationKind::SetField {
            field: "name".to_string(),
            value: json!(value),
        },
    };
    let detail = SyncConflictDetail {
        summary: SyncConflictSummary {
            conflict_id: "conflict-1".to_string(),
            kind: SyncConflictKind::ConcurrentWrite,
            entity: SyncEntityKey {
                kind: SyncEntityKind::Project,
                id: "project-1".to_string(),
            },
            field: Some("name".to_string()),
            created_at_ms: 123,
        },
        current: operation("winner", "current"),
        conflicting: operation("loser", "conflicting"),
    };

    let json = serde_json::to_value(detail).unwrap();
    assert_eq!(json["summary"]["conflictId"], "conflict-1");
    assert_eq!(json["current"]["kind"]["value"], "current");
    assert_eq!(json["conflicting"]["kind"]["value"], "conflicting");
}

#[test]
fn provider_and_conflict_contracts_are_provider_neutral() {
    let provider = SyncProviderDescriptor {
        id: "webdav".to_string(),
        display_name: "WebDAV".to_string(),
    };

    assert_eq!(serde_json::to_value(provider).unwrap()["id"], "webdav");
    assert_eq!(
        serde_json::to_value(SyncConflictResolution::UseConflicting).unwrap(),
        "use_conflicting"
    );
}
