use std::collections::BTreeMap;

use serde_json::json;
use sona_core::sync::{
    HybridLogicalClock, SyncCausalContext, SyncConflictKind, SyncEntityKey, SyncEntityKind,
    SyncMergeOutcome, SyncOperation, SyncOperationKind, SyncVersion, merge_operations,
};

fn context(entries: &[(&str, u64)]) -> SyncCausalContext {
    SyncCausalContext {
        observed_sequences: entries
            .iter()
            .map(|(device_id, sequence)| ((*device_id).to_string(), *sequence))
            .collect::<BTreeMap<_, _>>(),
    }
}

fn set_operation(
    operation_id: &str,
    device_id: &str,
    sequence: u64,
    causal_context: SyncCausalContext,
    physical_ms: u64,
    logical: u32,
    value: serde_json::Value,
) -> SyncOperation {
    SyncOperation {
        operation_id: operation_id.to_string(),
        source_device_id: device_id.to_string(),
        source_sequence: sequence,
        causal_context,
        version: SyncVersion {
            clock: HybridLogicalClock {
                physical_ms,
                logical,
            },
            device_id: device_id.to_string(),
            operation_id: operation_id.to_string(),
        },
        entity: SyncEntityKey {
            kind: SyncEntityKind::Project,
            id: "project-1".to_string(),
        },
        kind: SyncOperationKind::SetField {
            field: "name".to_string(),
            value,
        },
    }
}

#[test]
fn causally_later_field_change_wins_without_conflict() {
    let first = set_operation("op-a", "device-a", 1, context(&[]), 100, 0, json!("first"));
    let later = set_operation(
        "op-b",
        "device-b",
        1,
        context(&[("device-a", 1)]),
        90,
        0,
        json!("later"),
    );

    assert_eq!(
        merge_operations(&first, &later).unwrap(),
        SyncMergeOutcome {
            winner: later,
            conflict: None,
        }
    );
}

#[test]
fn concurrent_different_values_choose_deterministically_and_preserve_loser() {
    let left = set_operation(
        "op-left",
        "device-a",
        1,
        context(&[]),
        100,
        0,
        json!("left"),
    );
    let right = set_operation(
        "op-right",
        "device-b",
        1,
        context(&[]),
        100,
        0,
        json!("right"),
    );

    let outcome = merge_operations(&left, &right).unwrap();

    assert_eq!(outcome.winner, right);
    let conflict = outcome.conflict.unwrap();
    assert_eq!(conflict.kind, SyncConflictKind::ConcurrentWrite);
    assert_eq!(conflict.winner.operation_id, "op-right");
    assert_eq!(conflict.loser.operation_id, "op-left");
}

#[test]
fn concurrent_equal_values_converge_without_conflict() {
    let left = set_operation(
        "op-left",
        "device-a",
        1,
        context(&[]),
        100,
        0,
        json!("same"),
    );
    let right = set_operation(
        "op-right",
        "device-b",
        1,
        context(&[]),
        101,
        0,
        json!("same"),
    );

    let outcome = merge_operations(&left, &right).unwrap();

    assert_eq!(outcome.winner, right);
    assert_eq!(outcome.conflict, None);
}

#[test]
fn concurrent_delete_wins_and_preserves_the_edit() {
    let edit = set_operation(
        "op-edit",
        "device-a",
        1,
        context(&[]),
        200,
        0,
        json!("edited"),
    );
    let delete = SyncOperation {
        operation_id: "op-delete".to_string(),
        source_device_id: "device-b".to_string(),
        source_sequence: 1,
        causal_context: context(&[]),
        version: SyncVersion {
            clock: HybridLogicalClock {
                physical_ms: 100,
                logical: 0,
            },
            device_id: "device-b".to_string(),
            operation_id: "op-delete".to_string(),
        },
        entity: edit.entity.clone(),
        kind: SyncOperationKind::DeleteEntity,
    };

    let outcome = merge_operations(&edit, &delete).unwrap();

    assert_eq!(outcome.winner, delete);
    let conflict = outcome.conflict.unwrap();
    assert_eq!(conflict.kind, SyncConflictKind::DeleteVsWrite);
    assert_eq!(conflict.loser, edit);
}

#[test]
fn operations_for_different_fields_are_not_merge_candidates() {
    let first = set_operation("op-a", "device-a", 1, context(&[]), 100, 0, json!("name"));
    let mut second = set_operation(
        "op-b",
        "device-b",
        1,
        context(&[]),
        100,
        0,
        json!("description"),
    );
    second.kind = SyncOperationKind::SetField {
        field: "description".to_string(),
        value: json!("description"),
    };

    let error = merge_operations(&first, &second).unwrap_err();

    assert!(error.to_string().contains("same entity field"));
}

#[test]
fn same_device_operations_in_one_segment_follow_version_order_without_conflict() {
    let first = set_operation(
        "op-first",
        "device-a",
        7,
        context(&[]),
        100,
        0,
        json!("first"),
    );
    let later = set_operation(
        "op-later",
        "device-a",
        7,
        context(&[]),
        100,
        1,
        json!("later"),
    );

    let outcome = merge_operations(&first, &later).unwrap();

    assert_eq!(outcome.winner, later);
    assert_eq!(outcome.conflict, None);
}
