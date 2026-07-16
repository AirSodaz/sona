use std::cmp::Ordering;

use super::{
    SyncConflict, SyncConflictKind, SyncError, SyncMergeOutcome, SyncOperation, SyncOperationKind,
};

pub fn merge_operations(
    left: &SyncOperation,
    right: &SyncOperation,
) -> Result<SyncMergeOutcome, SyncError> {
    validate_merge_target(left, right)?;

    if left.observes(right) {
        return Ok(outcome(left.clone(), None));
    }
    if right.observes(left) {
        return Ok(outcome(right.clone(), None));
    }

    if left.kind.has_same_value(&right.kind) {
        return Ok(outcome(deterministic_winner(left, right).clone(), None));
    }

    let conflict_kind = match (&left.kind, &right.kind) {
        (SyncOperationKind::DeleteEntity, SyncOperationKind::SetField { .. })
        | (SyncOperationKind::SetField { .. }, SyncOperationKind::DeleteEntity) => {
            SyncConflictKind::DeleteVsWrite
        }
        _ => SyncConflictKind::ConcurrentWrite,
    };
    let winner = match (&left.kind, &right.kind) {
        (SyncOperationKind::DeleteEntity, SyncOperationKind::SetField { .. }) => left,
        (SyncOperationKind::SetField { .. }, SyncOperationKind::DeleteEntity) => right,
        _ => deterministic_winner(left, right),
    };
    let loser = if std::ptr::eq(winner, left) {
        right
    } else {
        left
    };
    let winner = winner.clone();
    Ok(outcome(
        winner.clone(),
        Some(SyncConflict {
            kind: conflict_kind,
            winner,
            loser: loser.clone(),
        }),
    ))
}

fn validate_merge_target(left: &SyncOperation, right: &SyncOperation) -> Result<(), SyncError> {
    if left.entity != right.entity {
        return Err(SyncError::InvalidOperation(
            "Merge candidates must target the same entity.".to_string(),
        ));
    }

    if let (
        SyncOperationKind::SetField {
            field: left_field, ..
        },
        SyncOperationKind::SetField {
            field: right_field, ..
        },
    ) = (&left.kind, &right.kind)
        && left_field != right_field
    {
        return Err(SyncError::InvalidOperation(
            "Merge candidates must target the same entity field.".to_string(),
        ));
    }

    Ok(())
}

fn deterministic_winner<'a>(
    left: &'a SyncOperation,
    right: &'a SyncOperation,
) -> &'a SyncOperation {
    match left.version.cmp(&right.version) {
        Ordering::Less => right,
        Ordering::Equal | Ordering::Greater => left,
    }
}

fn outcome(winner: SyncOperation, conflict: Option<SyncConflict>) -> SyncMergeOutcome {
    SyncMergeOutcome { winner, conflict }
}
