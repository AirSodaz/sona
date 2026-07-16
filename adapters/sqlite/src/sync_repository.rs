use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use rusqlite::{OptionalExtension, Transaction, params};
use serde_json::Value;
use sona_core::sync::{
    HybridLogicalClock, SyncCausalContext, SyncConflict, SyncConflictDetail,
    SyncConflictResolution, SyncConflictSummary, SyncDeviceCursor, SyncEntityKey, SyncEntityKind,
    SyncError, SyncJoinPreview, SyncLocalRepository, SyncLocalRuntimeState, SyncOperation,
    SyncOperationKind, SyncPresetV1, SyncPublishedCheckpoint, SyncPublishedSegment,
    SyncRemoteApplyResult, SyncRemoteSegment, SyncVersion, merge_operations,
};

use crate::{Database, DatabaseError};

const DELETE_FIELD: &str = "__entity__";

#[derive(Clone)]
pub struct SqliteSyncRepository {
    db: Arc<Database>,
}

impl SqliteSyncRepository {
    pub fn open_existing(db: Arc<Database>) -> Result<Option<Self>, SyncError> {
        let exists = db
            .with_connection(|connection| {
                connection
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM sync_state WHERE id = 1)",
                        [],
                        |row| row.get::<_, bool>(0),
                    )
                    .map_err(DatabaseError::QueryError)
            })
            .map_err(sync_database_error)?;
        Ok(exists.then_some(Self { db }))
    }

    pub fn initialize(
        db: Arc<Database>,
        vault_id: &str,
        device_id: &str,
        preset: SyncPresetV1,
    ) -> Result<Self, SyncError> {
        validate_identifier(vault_id, "vault ID")?;
        validate_identifier(device_id, "device ID")?;
        let preset_json = serde_json::to_string(&preset).map_err(sync_serialization_error)?;
        db.with_rw_transaction(|transaction| {
            let inserted = transaction.execute(
                "INSERT OR IGNORE INTO sync_state (
                    id, vault_id, device_id, preset
                 ) VALUES (1, ?1, ?2, ?3)",
                params![vault_id, device_id, preset_json],
            )?;
            let existing = transaction.query_row(
                "SELECT vault_id, device_id FROM sync_state WHERE id = 1",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )?;
            if existing.0 != vault_id || existing.1 != device_id {
                return Err(DatabaseError::Internal(
                    "SQLite sync repository is already connected to another vault or device."
                        .to_string(),
                ));
            }
            if inserted == 1 {
                seed_sync_baseline_in_transaction(transaction, preset)?;
            }
            Ok(())
        })
        .map_err(sync_database_error)?;
        Ok(Self { db })
    }

    pub fn preview_join(
        db: Arc<Database>,
        vault_id: &str,
        preview_device_id: &str,
        preset: SyncPresetV1,
        remote_segments: &[SyncRemoteSegment],
    ) -> Result<SyncJoinPreview, SyncError> {
        validate_identifier(vault_id, "vault ID")?;
        validate_identifier(preview_device_id, "preview device ID")?;
        let preset_json = serde_json::to_string(&preset).map_err(sync_serialization_error)?;
        db.with_write_connection(|connection| {
            let transaction = connection
                .unchecked_transaction()
                .map_err(DatabaseError::QueryError)?;
            let preview = (|| {
                transaction.execute(
                    "INSERT INTO sync_state (id, vault_id, device_id, preset)
                     VALUES (1, ?1, ?2, ?3)",
                    params![vault_id, preview_device_id, preset_json],
                )?;
                seed_sync_baseline_in_transaction(&transaction, preset)?;
                let local_operation_count =
                    transaction.query_row("SELECT COUNT(*) FROM sync_outbox", [], |row| {
                        row.get::<_, i64>(0)
                    })?;
                let mut remote_operation_count = 0_u64;
                let mut projected_conflict_count = 0_u64;
                for segment in remote_segments {
                    remote_operation_count = remote_operation_count.saturating_add(
                        u64::try_from(segment.operations.len()).unwrap_or(u64::MAX),
                    );
                    let applied = apply_remote_segment_in_transaction(&transaction, segment)?;
                    projected_conflict_count =
                        projected_conflict_count.saturating_add(applied.conflict_count);
                }
                Ok(SyncJoinPreview {
                    local_operation_count: u64::try_from(local_operation_count).unwrap_or(0),
                    remote_operation_count,
                    projected_conflict_count,
                })
            })();
            let rollback = transaction.rollback().map_err(DatabaseError::QueryError);
            match (preview, rollback) {
                (Err(error), _) => Err(error),
                (Ok(_), Err(error)) => Err(error),
                (Ok(preview), Ok(())) => Ok(preview),
            }
        })
        .map_err(sync_database_error)
    }

    pub fn record_local_operation(&self, operation: &SyncOperation) -> Result<(), SyncError> {
        self.db
            .with_rw_transaction(|transaction| {
                record_sync_operation_in_transaction(transaction, operation)
            })
            .map_err(sync_database_error)
    }

    pub fn list_unresolved_conflicts(&self) -> Result<Vec<SyncConflict>, SyncError> {
        self.db
            .with_connection(|connection| {
                let mut statement = connection.prepare(
                    "SELECT conflict_json
                     FROM sync_conflicts
                     WHERE resolved_at IS NULL
                     ORDER BY created_at, conflict_id",
                )?;
                statement
                    .query_map([], |row| row.get::<_, String>(0))?
                    .map(|row| {
                        let json = row?;
                        serde_json::from_str(&json).map_err(DatabaseError::SerializationError)
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(sync_database_error)
    }

    pub fn list_conflict_summaries(&self) -> Result<Vec<SyncConflictSummary>, SyncError> {
        self.db
            .with_connection(|connection| {
                let mut statement = connection.prepare(
                    "SELECT conflict_id, field_name, conflict_json, created_at
                     FROM sync_conflicts
                     WHERE resolved_at IS NULL
                     ORDER BY created_at, conflict_id",
                )?;
                let rows = statement.query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                })?;
                rows.map(|row| {
                    let (conflict_id, field_name, json, created_at) = row?;
                    let conflict: SyncConflict = serde_json::from_str(&json)?;
                    conflict_summary(conflict_id, field_name, created_at, &conflict)
                })
                .collect::<Result<Vec<_>, _>>()
            })
            .map_err(sync_database_error)
    }

    pub fn get_conflict_detail(
        &self,
        conflict_id: &str,
    ) -> Result<Option<SyncConflictDetail>, SyncError> {
        self.db
            .with_connection(|connection| {
                let row = connection
                    .query_row(
                        "SELECT field_name, conflict_json, created_at
                         FROM sync_conflicts
                         WHERE conflict_id = ?1 AND resolved_at IS NULL",
                        [conflict_id],
                        |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, i64>(2)?,
                            ))
                        },
                    )
                    .optional()?;
                let Some((field_name, json, created_at)) = row else {
                    return Ok(None);
                };
                let conflict: SyncConflict = serde_json::from_str(&json)?;
                let summary =
                    conflict_summary(conflict_id.to_string(), field_name, created_at, &conflict)?;
                Ok(Some(SyncConflictDetail {
                    summary,
                    current: conflict.winner,
                    conflicting: conflict.loser,
                }))
            })
            .map_err(sync_database_error)
    }

    pub fn resolve_conflict(
        &self,
        conflict_id: &str,
        resolution: SyncConflictResolution,
        resolved_at_ms: u64,
    ) -> Result<(), SyncError> {
        self.db
            .with_rw_transaction(|transaction| {
                let json = transaction
                    .query_row(
                        "SELECT conflict_json FROM sync_conflicts
                         WHERE conflict_id = ?1 AND resolved_at IS NULL",
                        [conflict_id],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()?
                    .ok_or_else(|| {
                        DatabaseError::NotFoundError(format!(
                            "Sync conflict not found: {conflict_id}"
                        ))
                    })?;
                let conflict: SyncConflict = serde_json::from_str(&json)?;
                match resolution {
                    SyncConflictResolution::KeepCurrent => {}
                    SyncConflictResolution::UseConflicting => {
                        record_local_change_in_transaction(
                            transaction,
                            conflict.loser.entity.kind,
                            &conflict.loser.entity.id,
                            conflict.loser.kind.clone(),
                            resolved_at_ms,
                        )?;
                        apply_domain_operation(transaction, &conflict.loser)?;
                    }
                    SyncConflictResolution::KeepBoth => {
                        preserve_conflicting_transcript_snapshot(
                            transaction,
                            &conflict,
                            resolved_at_ms,
                        )?;
                    }
                }
                transaction.execute(
                    "UPDATE sync_conflicts SET resolved_at = ?1 WHERE conflict_id = ?2",
                    params![
                        u64_to_i64(resolved_at_ms, "conflict resolution time")?,
                        conflict_id,
                    ],
                )?;
                Ok(())
            })
            .map_err(sync_database_error)
    }

    pub fn change_preset(
        &self,
        next: SyncPresetV1,
        confirm_shrink: bool,
    ) -> Result<u64, SyncError> {
        self.db
            .with_rw_transaction(|transaction| {
                let current = transaction.query_row(
                    "SELECT preset FROM sync_state WHERE id = 1",
                    [],
                    |row| parse_preset(row.get::<_, String>(0)?),
                )?;
                if current == next {
                    return Ok(0);
                }
                validate_preset_transition(current, next, confirm_shrink)?;
                let shrinking = preset_rank(next) < preset_rank(current);

                let mut tombstone_count = 0_u64;
                if shrinking {
                    let entities = sync_entities_excluded_by_preset(transaction, next)?;
                    let now_ms = sync_now_ms();
                    for (kind, entity_id) in entities {
                        record_local_delete_in_transaction(transaction, kind, &entity_id, now_ms)?;
                        tombstone_count += 1;
                    }
                }

                transaction.execute(
                    "UPDATE sync_state
                     SET preset = ?1, checkpoint_required = 1
                     WHERE id = 1",
                    [serde_json::to_string(&next)?],
                )?;

                if !shrinking {
                    if current == SyncPresetV1::Content && next != SyncPresetV1::Content {
                        crate::config_store::seed_sync_config_baseline_in_transaction(transaction)?;
                    }
                    if current != SyncPresetV1::Full && next == SyncPresetV1::Full {
                        crate::automation::seed_sync_automation_baseline_in_transaction(
                            transaction,
                        )?;
                    }
                }
                Ok(tombstone_count)
            })
            .map_err(sync_database_error)
    }

    pub fn validate_preset_change(
        &self,
        next: SyncPresetV1,
        confirm_shrink: bool,
    ) -> Result<(), SyncError> {
        self.db
            .with_connection(|connection| {
                let current = connection.query_row(
                    "SELECT preset FROM sync_state WHERE id = 1",
                    [],
                    |row| parse_preset(row.get::<_, String>(0)?),
                )?;
                validate_preset_transition(current, next, confirm_shrink)
            })
            .map_err(sync_database_error)
    }

    pub fn pending_operation_count(&self) -> Result<u64, SyncError> {
        let state = self.load_runtime_state()?;
        let count = self
            .load_pending_operations(state.preset, usize::MAX, usize::MAX)?
            .len();
        u64::try_from(count).map_err(|error| SyncError::LocalRepository(error.to_string()))
    }

    pub fn unresolved_conflict_count(&self) -> Result<u64, SyncError> {
        self.db
            .with_connection(|connection| {
                connection
                    .query_row(
                        "SELECT COUNT(*) FROM sync_conflicts WHERE resolved_at IS NULL",
                        [],
                        |row| row.get::<_, i64>(0),
                    )
                    .map_err(DatabaseError::QueryError)
            })
            .map_err(sync_database_error)
            .and_then(|count| {
                u64::try_from(count).map_err(|error| SyncError::LocalRepository(error.to_string()))
            })
    }

    pub fn disconnect(&self) -> Result<(), SyncError> {
        self.db
            .with_rw_transaction(|transaction| {
                transaction.execute("DELETE FROM sync_conflicts", [])?;
                transaction.execute("DELETE FROM sync_entity_versions", [])?;
                transaction.execute("DELETE FROM sync_outbox", [])?;
                transaction.execute("DELETE FROM sync_device_cursors", [])?;
                transaction.execute("DELETE FROM sync_state", [])?;
                Ok(())
            })
            .map_err(sync_database_error)
    }

    pub fn is_paused(&self) -> Result<bool, SyncError> {
        self.db
            .with_connection(|connection| {
                connection
                    .query_row("SELECT paused FROM sync_state WHERE id = 1", [], |row| {
                        Ok(row.get::<_, i64>(0)? != 0)
                    })
                    .map_err(DatabaseError::QueryError)
            })
            .map_err(sync_database_error)
    }

    pub fn set_paused(&self, paused: bool) -> Result<(), SyncError> {
        self.db
            .with_rw_transaction(|transaction| {
                transaction.execute(
                    "UPDATE sync_state SET paused = ?1 WHERE id = 1",
                    [i64::from(paused)],
                )?;
                Ok(())
            })
            .map_err(sync_database_error)
    }
}

fn preset_rank(preset: SyncPresetV1) -> u8 {
    match preset {
        SyncPresetV1::Content => 0,
        SyncPresetV1::Standard => 1,
        SyncPresetV1::Full => 2,
    }
}

fn validate_preset_transition(
    current: SyncPresetV1,
    next: SyncPresetV1,
    confirm_shrink: bool,
) -> Result<(), DatabaseError> {
    if preset_rank(next) < preset_rank(current) && !confirm_shrink {
        Err(DatabaseError::Internal(
            "Shrinking the sync preset requires explicit confirmation.".to_string(),
        ))
    } else {
        Ok(())
    }
}

fn sync_entities_excluded_by_preset(
    transaction: &Transaction<'_>,
    next: SyncPresetV1,
) -> Result<BTreeSet<(SyncEntityKind, String)>, DatabaseError> {
    let mut statement = transaction.prepare_cached(
        "SELECT DISTINCT entity_kind, entity_id
         FROM sync_entity_versions
         ORDER BY entity_kind, entity_id",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut entities = BTreeSet::new();
    for row in rows {
        let (kind_json, entity_id) = row?;
        let kind: SyncEntityKind = serde_json::from_str(&kind_json)?;
        if !preset_allows(next, kind) {
            entities.insert((kind, entity_id));
        }
    }
    Ok(entities)
}

fn seed_sync_baseline_in_transaction(
    transaction: &Transaction<'_>,
    preset: SyncPresetV1,
) -> Result<(), DatabaseError> {
    for tag in crate::tag::load_tags_in_transaction(transaction)? {
        let sort_order = tag.sort_order;
        crate::tag::record_tag_sync_fields(transaction, &tag, Some(sort_order))?;
    }
    seed_history_baseline_in_transaction(transaction)?;
    if preset != SyncPresetV1::Content {
        crate::config_store::seed_sync_config_baseline_in_transaction(transaction)?;
    }
    if preset == SyncPresetV1::Full {
        crate::automation::seed_sync_automation_baseline_in_transaction(transaction)?;
    }
    Ok(())
}

fn seed_history_baseline_in_transaction(
    transaction: &Transaction<'_>,
) -> Result<(), DatabaseError> {
    let items = {
        let mut statement = transaction.prepare_cached(
            "SELECT h.id, h.timestamp, h.duration, h.title, h.preview_text, h.icon, h.kind,
                    COALESCE((
                        SELECT json_group_array(tag_id)
                        FROM (
                            SELECT hit.tag_id
                            FROM history_item_tags hit
                            JOIN tags tag ON tag.id = hit.tag_id
                            WHERE hit.history_id = h.id
                            ORDER BY tag.sort_order, tag.id
                        )
                    ), '[]'),
                    h.deleted_at
             FROM history_items
             AS h
             WHERE status = 'complete'
             ORDER BY id",
        )?;
        statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, f64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, Option<i64>>(8)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    for (id, timestamp, duration, title, preview_text, icon, kind, tag_ids_json, deleted_at) in
        items
    {
        let now_ms = u64::try_from(timestamp).unwrap_or(0);
        let tag_ids: Vec<String> = serde_json::from_str(&tag_ids_json)?;
        for (field, value) in [
            ("timestamp", serde_json::json!(timestamp)),
            ("duration", serde_json::json!(duration)),
            ("title", serde_json::json!(title)),
            ("previewText", serde_json::json!(preview_text)),
            ("icon", serde_json::json!(icon)),
            ("kind", serde_json::json!(kind)),
            ("tagIds", serde_json::json!(tag_ids)),
            ("deletedAt", serde_json::json!(deleted_at)),
        ] {
            record_local_field_change_in_transaction(
                transaction,
                SyncEntityKind::HistoryItem,
                &id,
                field,
                value,
                now_ms,
            )?;
        }
    }

    let transcripts = load_json_documents(
        transaction,
        "SELECT t.history_id, t.segments
         FROM history_transcripts t
         JOIN history_items h ON h.id = t.history_id
         WHERE h.status = 'complete'
         ORDER BY t.history_id",
    )?;
    for (history_id, document) in transcripts {
        record_local_field_change_in_transaction(
            transaction,
            SyncEntityKind::HistoryTranscript,
            &history_id,
            "document",
            document,
            sync_now_ms(),
        )?;
    }
    let summaries = load_json_documents(
        transaction,
        "SELECT s.history_id, s.payload
         FROM history_summaries s
         JOIN history_items h ON h.id = s.history_id
         WHERE h.status = 'complete'
         ORDER BY s.history_id",
    )?;
    for (history_id, document) in summaries {
        record_local_field_change_in_transaction(
            transaction,
            SyncEntityKind::HistorySummary,
            &history_id,
            "document",
            document,
            sync_now_ms(),
        )?;
    }

    let snapshots = {
        let mut statement = transaction.prepare_cached(
            "SELECT s.history_id, s.id, s.reason, s.created_at, s.segment_count, s.segments
             FROM transcript_snapshots s
             JOIN history_items h ON h.id = s.history_id
             WHERE h.status = 'complete'
             ORDER BY s.history_id, s.id",
        )?;
        statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?
    };
    for (history_id, snapshot_id, reason, created_at, segment_count, segments) in snapshots {
        let entity_id = format!("{history_id}::{snapshot_id}");
        let now_ms = u64::try_from(created_at).unwrap_or(0);
        for (field, value) in [
            ("document", serde_json::from_str(&segments)?),
            ("reason", serde_json::json!(reason)),
            ("createdAt", serde_json::json!(created_at)),
            ("segmentCount", serde_json::json!(segment_count)),
        ] {
            record_local_field_change_in_transaction(
                transaction,
                SyncEntityKind::TranscriptSnapshot,
                &entity_id,
                field,
                value,
                now_ms,
            )?;
        }
    }
    Ok(())
}

fn load_json_documents(
    transaction: &Transaction<'_>,
    sql: &str,
) -> Result<Vec<(String, Value)>, DatabaseError> {
    let mut statement = transaction.prepare_cached(sql)?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    rows.map(|row| {
        let (id, json) = row?;
        Ok((id, serde_json::from_str(&json)?))
    })
    .collect()
}

fn conflict_summary(
    conflict_id: String,
    field_name: String,
    created_at: i64,
    conflict: &SyncConflict,
) -> Result<SyncConflictSummary, DatabaseError> {
    Ok(SyncConflictSummary {
        conflict_id,
        kind: conflict.kind,
        entity: conflict.winner.entity.clone(),
        field: (field_name != DELETE_FIELD).then_some(field_name),
        created_at_ms: i64_to_u64(created_at, "conflict creation time")?,
    })
}

fn preserve_conflicting_transcript_snapshot(
    transaction: &Transaction<'_>,
    conflict: &SyncConflict,
    created_at_ms: u64,
) -> Result<(), DatabaseError> {
    if conflict.loser.entity.kind != SyncEntityKind::HistoryTranscript {
        return Err(DatabaseError::Internal(
            "Keep-both conflict resolution is only supported for transcript documents.".to_string(),
        ));
    }
    let SyncOperationKind::SetField { field, value } = &conflict.loser.kind else {
        return Err(DatabaseError::Internal(
            "Keep-both transcript resolution requires a document version.".to_string(),
        ));
    };
    if field != "document" {
        return Err(DatabaseError::Internal(
            "Keep-both transcript resolution requires a document field.".to_string(),
        ));
    }
    let snapshot_id = format!("sync-conflict-{}", uuid::Uuid::new_v4());
    let segments = serde_json::to_string(value)?;
    let segment_count = value.as_array().map_or(0, Vec::len);
    transaction.execute(
        "INSERT INTO transcript_snapshots
         (id, history_id, reason, created_at, segment_count, segments)
         VALUES (?1, ?2, 'sync_conflict', ?3, ?4, ?5)",
        params![
            snapshot_id,
            conflict.loser.entity.id,
            u64_to_i64(created_at_ms, "conflict snapshot time")?,
            usize_to_i64(segment_count, "conflict snapshot segment count")?,
            segments,
        ],
    )?;
    let entity_id = format!("{}::{snapshot_id}", conflict.loser.entity.id);
    for (field, value) in [
        ("document", value.clone()),
        ("reason", Value::String("sync_conflict".to_string())),
        ("createdAt", serde_json::json!(created_at_ms)),
        ("segmentCount", serde_json::json!(segment_count)),
    ] {
        record_local_field_change_in_transaction(
            transaction,
            SyncEntityKind::TranscriptSnapshot,
            &entity_id,
            field,
            value,
            created_at_ms,
        )?;
    }
    Ok(())
}

pub fn record_sync_operation_in_transaction(
    transaction: &Transaction<'_>,
    operation: &SyncOperation,
) -> Result<(), DatabaseError> {
    let operation = canonicalize_operation(operation);
    let configured_device = transaction
        .query_row("SELECT device_id FROM sync_state WHERE id = 1", [], |row| {
            row.get::<_, String>(0)
        })
        .optional()?;
    let Some(configured_device) = configured_device else {
        return Ok(());
    };
    if operation.source_device_id != configured_device
        || operation.version.device_id != configured_device
    {
        return Err(DatabaseError::Internal(
            "Local sync operation device does not match sync_state.".to_string(),
        ));
    }
    validate_operation(&operation).map_err(|error| DatabaseError::Internal(error.to_string()))?;
    let entity_kind = entity_kind_json(operation.entity.kind)?;
    let field_name = operation_field(&operation.kind);
    let operation_json = serde_json::to_string(&operation)?;
    match &operation.kind {
        SyncOperationKind::DeleteEntity => {
            transaction.execute(
                "DELETE FROM sync_outbox
                 WHERE entity_kind = ?1 AND entity_id = ?2",
                params![entity_kind, operation.entity.id],
            )?;
        }
        SyncOperationKind::SetField { .. } => {
            transaction.execute(
                "DELETE FROM sync_outbox
                 WHERE entity_kind = ?1 AND entity_id = ?2 AND field_name = ?3",
                params![entity_kind, operation.entity.id, field_name],
            )?;
        }
    }
    transaction.execute(
        "INSERT INTO sync_outbox (
            operation_id, entity_kind, entity_id, field_name, operation_json, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(operation_id) DO UPDATE SET
            operation_json = excluded.operation_json",
        params![
            operation.operation_id,
            entity_kind,
            operation.entity.id,
            field_name,
            operation_json,
            u64_to_i64(operation.version.clock.physical_ms, "operation clock")?,
        ],
    )?;
    transaction.execute(
        "INSERT INTO sync_entity_versions (
            entity_kind, entity_id, field_name, operation_json
         ) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(entity_kind, entity_id, field_name) DO UPDATE SET
            operation_json = excluded.operation_json",
        params![entity_kind, operation.entity.id, field_name, operation_json,],
    )?;
    transaction.execute(
        "UPDATE sync_state
         SET hlc_physical = ?1, hlc_logical = ?2
         WHERE id = 1",
        params![
            u64_to_i64(operation.version.clock.physical_ms, "operation clock")?,
            i64::from(operation.version.clock.logical),
        ],
    )?;
    Ok(())
}

pub(crate) fn record_local_field_change_in_transaction(
    transaction: &Transaction<'_>,
    entity_kind: SyncEntityKind,
    entity_id: &str,
    field: &str,
    value: Value,
    now_ms: u64,
) -> Result<(), DatabaseError> {
    record_local_change_in_transaction(
        transaction,
        entity_kind,
        entity_id,
        SyncOperationKind::SetField {
            field: field.to_string(),
            value,
        },
        now_ms,
    )
}

pub(crate) fn record_local_delete_in_transaction(
    transaction: &Transaction<'_>,
    entity_kind: SyncEntityKind,
    entity_id: &str,
    now_ms: u64,
) -> Result<(), DatabaseError> {
    record_local_change_in_transaction(
        transaction,
        entity_kind,
        entity_id,
        SyncOperationKind::DeleteEntity,
        now_ms,
    )
}

pub(crate) fn sync_now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

fn record_local_change_in_transaction(
    transaction: &Transaction<'_>,
    entity_kind: SyncEntityKind,
    entity_id: &str,
    kind: SyncOperationKind,
    now_ms: u64,
) -> Result<(), DatabaseError> {
    let state = transaction
        .query_row(
            "SELECT device_id, preset, next_sequence, hlc_physical, hlc_logical
             FROM sync_state WHERE id = 1",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    parse_preset(row.get::<_, String>(1)?)?,
                    i64_to_u64(row.get(2)?, "next sequence")?,
                    i64_to_u64(row.get(3)?, "HLC physical time")?,
                    u32::try_from(row.get::<_, i64>(4)?).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            4,
                            rusqlite::types::Type::Integer,
                            Box::new(error),
                        )
                    })?,
                ))
            },
        )
        .optional()?;
    let Some((device_id, preset, source_sequence, physical_ms, logical)) = state else {
        return Ok(());
    };
    let operation_id = uuid::Uuid::new_v4().to_string();
    let operation = SyncOperation {
        operation_id: operation_id.clone(),
        source_device_id: device_id.clone(),
        source_sequence,
        causal_context: load_causal_context(transaction)?,
        version: SyncVersion {
            clock: HybridLogicalClock {
                physical_ms,
                logical,
            }
            .tick(now_ms),
            device_id,
            operation_id,
        },
        entity: SyncEntityKey {
            kind: entity_kind,
            id: entity_id.to_string(),
        },
        kind,
    };
    if !operation_allowed(preset, &operation) {
        return Ok(());
    }
    record_sync_operation_in_transaction(transaction, &operation)
}

fn load_causal_context(transaction: &Transaction<'_>) -> Result<SyncCausalContext, DatabaseError> {
    let mut statement = transaction
        .prepare_cached("SELECT device_id, sequence FROM sync_device_cursors ORDER BY device_id")?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    let mut observed_sequences = BTreeMap::new();
    for row in rows {
        let (device_id, sequence) = row?;
        observed_sequences.insert(device_id, i64_to_u64(sequence, "remote cursor sequence")?);
    }
    Ok(SyncCausalContext { observed_sequences })
}

impl SyncLocalRepository for SqliteSyncRepository {
    fn load_runtime_state(&self) -> Result<SyncLocalRuntimeState, SyncError> {
        self.db
            .with_connection(|connection| {
                let mut state = connection.query_row(
                    "SELECT vault_id, device_id, preset, next_sequence,
                            previous_cipher_hash, operations_since_checkpoint,
                            bytes_since_checkpoint, checkpoint_required
                     FROM sync_state WHERE id = 1",
                    [],
                    |row| {
                        Ok(SyncLocalRuntimeState {
                            vault_id: row.get(0)?,
                            device_id: row.get(1)?,
                            preset: parse_preset(row.get::<_, String>(2)?)?,
                            next_sequence: i64_to_u64(row.get(3)?, "next sequence")?,
                            previous_cipher_hash: row.get(4)?,
                            remote_cursors: BTreeMap::new(),
                            operations_since_checkpoint: i64_to_u64(
                                row.get(5)?,
                                "checkpoint operation count",
                            )?,
                            bytes_since_checkpoint: i64_to_u64(
                                row.get(6)?,
                                "checkpoint byte count",
                            )?,
                            checkpoint_required: row.get::<_, i64>(7)? != 0,
                        })
                    },
                )?;
                let mut statement = connection.prepare(
                    "SELECT device_id, sequence, cipher_hash
                     FROM sync_device_cursors ORDER BY device_id",
                )?;
                let cursors = statement.query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        SyncDeviceCursor {
                            sequence: i64_to_u64(row.get(1)?, "remote cursor sequence")?,
                            cipher_hash: row.get(2)?,
                        },
                    ))
                })?;
                for cursor in cursors {
                    let (device_id, cursor) = cursor?;
                    state.remote_cursors.insert(device_id, cursor);
                }
                Ok(state)
            })
            .map_err(sync_database_error)
    }

    fn load_pending_operations(
        &self,
        preset: SyncPresetV1,
        maximum_operations: usize,
        maximum_bytes: usize,
    ) -> Result<Vec<SyncOperation>, SyncError> {
        self.db
            .with_connection(|connection| {
                let mut statement = connection.prepare(
                    "SELECT operation_json
                     FROM sync_outbox
                     ORDER BY created_at, operation_id",
                )?;
                let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
                let mut operations = Vec::new();
                let mut encoded_bytes = 0_usize;
                for row in rows {
                    let json = row?;
                    let operation = canonicalize_operation(&serde_json::from_str(&json)?);
                    if !operation_allowed(preset, &operation)
                        && !matches!(operation.kind, SyncOperationKind::DeleteEntity)
                    {
                        continue;
                    }
                    if operations.len() >= maximum_operations {
                        break;
                    }
                    if !operations.is_empty()
                        && encoded_bytes.saturating_add(json.len()) > maximum_bytes
                    {
                        break;
                    }
                    encoded_bytes = encoded_bytes.saturating_add(json.len());
                    operations.push(operation);
                }
                Ok(operations)
            })
            .map_err(sync_database_error)
    }

    fn mark_segment_published(&self, published: &SyncPublishedSegment) -> Result<(), SyncError> {
        self.db
            .with_rw_transaction(|transaction| {
                let next_sequence = transaction.query_row(
                    "SELECT next_sequence FROM sync_state WHERE id = 1",
                    [],
                    |row| row.get::<_, i64>(0),
                )?;
                if i64_to_u64(next_sequence, "next sequence")? != published.sequence {
                    return Err(DatabaseError::Internal(
                        "Published sync segment sequence does not match sync_state.".to_string(),
                    ));
                }
                for operation in &published.operations {
                    transaction.execute(
                        "DELETE FROM sync_outbox WHERE operation_id = ?1",
                        [&operation.operation_id],
                    )?;
                    let entity_kind = entity_kind_json(operation.entity.kind)?;
                    let field_name = operation_field(&operation.kind);
                    let current_json = transaction
                        .query_row(
                            "SELECT operation_json
                             FROM sync_entity_versions
                             WHERE entity_kind = ?1 AND entity_id = ?2 AND field_name = ?3",
                            params![entity_kind, operation.entity.id, field_name],
                            |row| row.get::<_, String>(0),
                        )
                        .optional()?;
                    let should_normalize = current_json
                        .as_deref()
                        .map(serde_json::from_str::<SyncOperation>)
                        .transpose()?
                        .is_some_and(|current| current.operation_id == operation.operation_id);
                    if should_normalize {
                        transaction.execute(
                            "UPDATE sync_entity_versions
                             SET operation_json = ?1
                             WHERE entity_kind = ?2 AND entity_id = ?3 AND field_name = ?4",
                            params![
                                serde_json::to_string(operation)?,
                                entity_kind,
                                operation.entity.id,
                                field_name,
                            ],
                        )?;
                    }
                }
                transaction.execute(
                    "UPDATE sync_state
                     SET next_sequence = ?1,
                         previous_cipher_hash = ?2,
                         operations_since_checkpoint = operations_since_checkpoint + ?3,
                         bytes_since_checkpoint = bytes_since_checkpoint + ?4
                     WHERE id = 1",
                    params![
                        u64_to_i64(published.sequence.saturating_add(1), "next sequence")?,
                        published.cipher_hash,
                        usize_to_i64(published.operations.len(), "published operation count")?,
                        u64_to_i64(published.encrypted_bytes, "published encrypted bytes")?,
                    ],
                )?;
                Ok(())
            })
            .map_err(sync_database_error)
    }

    fn load_checkpoint_operations(&self) -> Result<Vec<SyncOperation>, SyncError> {
        self.db
            .with_connection(|connection| {
                let mut statement = connection.prepare(
                    "SELECT operation_json
                     FROM sync_entity_versions
                     ORDER BY entity_kind, entity_id, field_name",
                )?;
                statement
                    .query_map([], |row| row.get::<_, String>(0))?
                    .map(|row| {
                        let json = row?;
                        serde_json::from_str(&json)
                            .map(|operation| canonicalize_operation(&operation))
                            .map_err(DatabaseError::SerializationError)
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(sync_database_error)
    }

    fn mark_checkpoint_published(
        &self,
        checkpoint: &SyncPublishedCheckpoint,
    ) -> Result<(), SyncError> {
        self.db
            .with_rw_transaction(|transaction| {
                transaction.execute(
                    "UPDATE sync_state
                     SET operations_since_checkpoint = 0,
                         bytes_since_checkpoint = 0,
                         checkpoint_required = 0,
                         last_checkpoint_sequence = ?1,
                         last_checkpoint_hash = ?2,
                         last_checkpoint_created_at = ?3
                     WHERE id = 1",
                    params![
                        u64_to_i64(checkpoint.sequence, "checkpoint sequence")?,
                        checkpoint.cipher_hash,
                        u64_to_i64(checkpoint.created_at_ms, "checkpoint creation time")?,
                    ],
                )?;
                Ok(())
            })
            .map_err(sync_database_error)
    }

    fn apply_remote_segment(
        &self,
        segment: &SyncRemoteSegment,
    ) -> Result<SyncRemoteApplyResult, SyncError> {
        self.db
            .with_rw_transaction(|transaction| {
                apply_remote_segment_in_transaction(transaction, segment)
            })
            .map_err(sync_database_error)
    }
}

fn apply_remote_segment_in_transaction(
    transaction: &Transaction<'_>,
    segment: &SyncRemoteSegment,
) -> Result<SyncRemoteApplyResult, DatabaseError> {
    let configured_preset =
        transaction.query_row("SELECT preset FROM sync_state WHERE id = 1", [], |row| {
            parse_preset(row.get::<_, String>(0)?)
        })?;
    let existing_cursor = transaction
        .query_row(
            "SELECT sequence FROM sync_device_cursors WHERE device_id = ?1",
            [&segment.device_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;
    if existing_cursor
        .map(|sequence| i64_to_u64(sequence, "remote cursor sequence"))
        .transpose()?
        .is_some_and(|sequence| sequence >= segment.sequence)
    {
        return Ok(SyncRemoteApplyResult::default());
    }

    let mut result = SyncRemoteApplyResult::default();
    for operation in &segment.operations {
        let operation = canonicalize_operation(operation);
        validate_operation(&operation)
            .map_err(|error| DatabaseError::Internal(error.to_string()))?;
        if !operation_allowed(configured_preset, &operation)
            && !matches!(operation.kind, SyncOperationKind::DeleteEntity)
        {
            return Err(DatabaseError::Internal(format!(
                "Sync operation targets an excluded or unsupported field: {:?} {}.",
                operation.entity.kind,
                operation_field(&operation.kind),
            )));
        }
        let entity_kind = entity_kind_json(operation.entity.kind)?;
        apply_merged_remote_operation(transaction, &operation, &entity_kind, &mut result)?;
        result.applied_operation_count += 1;
    }
    observe_remote_clocks(transaction, &segment.operations)?;
    transaction.execute(
        "INSERT INTO sync_device_cursors (device_id, sequence, cipher_hash)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(device_id) DO UPDATE SET
            sequence = excluded.sequence,
            cipher_hash = excluded.cipher_hash",
        params![
            segment.device_id,
            u64_to_i64(segment.sequence, "remote segment sequence")?,
            segment.cipher_hash,
        ],
    )?;
    Ok(result)
}

fn observe_remote_clocks(
    transaction: &Transaction<'_>,
    operations: &[SyncOperation],
) -> Result<(), DatabaseError> {
    let Some(remote_clock) = operations
        .iter()
        .map(|operation| operation.version.clock)
        .max()
    else {
        return Ok(());
    };
    let (physical_ms, logical) = transaction.query_row(
        "SELECT hlc_physical, hlc_logical FROM sync_state WHERE id = 1",
        [],
        |row| {
            Ok((
                i64_to_u64(row.get(0)?, "HLC physical time")?,
                u32::try_from(row.get::<_, i64>(1)?).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Integer,
                        Box::new(error),
                    )
                })?,
            ))
        },
    )?;
    let observed = HybridLogicalClock {
        physical_ms,
        logical,
    }
    .observe(remote_clock, remote_clock.physical_ms);
    transaction.execute(
        "UPDATE sync_state SET hlc_physical = ?1, hlc_logical = ?2 WHERE id = 1",
        params![
            u64_to_i64(observed.physical_ms, "HLC physical time")?,
            i64::from(observed.logical),
        ],
    )?;
    Ok(())
}

fn apply_merged_remote_operation(
    transaction: &Transaction<'_>,
    operation: &SyncOperation,
    entity_kind: &str,
    result: &mut SyncRemoteApplyResult,
) -> Result<(), DatabaseError> {
    match &operation.kind {
        SyncOperationKind::SetField { field, .. } => {
            let mut resurrecting = false;
            if let Some(tombstone) =
                load_entity_version(transaction, entity_kind, &operation.entity.id, DELETE_FIELD)?
            {
                let outcome = merge_operations(&tombstone, operation)
                    .map_err(|error| DatabaseError::Internal(error.to_string()))?;
                persist_merge_conflict(transaction, outcome.conflict.as_ref(), result)?;
                if outcome.winner != *operation {
                    return Ok(());
                }
                transaction.execute(
                    "DELETE FROM sync_entity_versions
                     WHERE entity_kind = ?1 AND entity_id = ?2 AND field_name = ?3",
                    params![entity_kind, operation.entity.id, DELETE_FIELD],
                )?;
                resurrecting = true;
            }

            let current =
                load_entity_version(transaction, entity_kind, &operation.entity.id, field)?;
            let winner = if let Some(current) = current {
                let outcome = merge_operations(&current, operation)
                    .map_err(|error| DatabaseError::Internal(error.to_string()))?;
                persist_merge_conflict(transaction, outcome.conflict.as_ref(), result)?;
                outcome.winner
            } else {
                operation.clone()
            };
            if resurrecting || winner == *operation {
                apply_domain_operation(transaction, &winner)?;
            }
            upsert_entity_version(transaction, entity_kind, field, &winner)
        }
        SyncOperationKind::DeleteEntity => {
            let mut statement = transaction.prepare_cached(
                "SELECT operation_json
                 FROM sync_entity_versions
                 WHERE entity_kind = ?1 AND entity_id = ?2
                 ORDER BY field_name",
            )?;
            let current_versions = statement
                .query_map(params![entity_kind, operation.entity.id], |row| {
                    row.get::<_, String>(0)
                })?
                .collect::<Result<Vec<_>, _>>()?;
            drop(statement);

            let mut delete_wins = true;
            for current_json in current_versions {
                let current: SyncOperation = serde_json::from_str(&current_json)?;
                let outcome = merge_operations(&current, operation)
                    .map_err(|error| DatabaseError::Internal(error.to_string()))?;
                persist_merge_conflict(transaction, outcome.conflict.as_ref(), result)?;
                if outcome.winner != *operation
                    && !matches!(outcome.winner.kind, SyncOperationKind::DeleteEntity)
                {
                    delete_wins = false;
                }
            }
            if !delete_wins {
                return Ok(());
            }

            apply_domain_operation(transaction, operation)?;
            transaction.execute(
                "DELETE FROM sync_entity_versions
                 WHERE entity_kind = ?1 AND entity_id = ?2",
                params![entity_kind, operation.entity.id],
            )?;
            transaction.execute(
                "DELETE FROM sync_outbox
                 WHERE entity_kind = ?1 AND entity_id = ?2",
                params![entity_kind, operation.entity.id],
            )?;
            upsert_entity_version(transaction, entity_kind, DELETE_FIELD, operation)
        }
    }
}

fn load_entity_version(
    transaction: &Transaction<'_>,
    entity_kind: &str,
    entity_id: &str,
    field_name: &str,
) -> Result<Option<SyncOperation>, DatabaseError> {
    let json = transaction
        .query_row(
            "SELECT operation_json
             FROM sync_entity_versions
             WHERE entity_kind = ?1 AND entity_id = ?2 AND field_name = ?3",
            params![entity_kind, entity_id, field_name],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    json.as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(DatabaseError::SerializationError)
}

fn upsert_entity_version(
    transaction: &Transaction<'_>,
    entity_kind: &str,
    field_name: &str,
    operation: &SyncOperation,
) -> Result<(), DatabaseError> {
    transaction.execute(
        "INSERT INTO sync_entity_versions (
            entity_kind, entity_id, field_name, operation_json
         ) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(entity_kind, entity_id, field_name) DO UPDATE SET
            operation_json = excluded.operation_json",
        params![
            entity_kind,
            operation.entity.id,
            field_name,
            serde_json::to_string(operation)?,
        ],
    )?;
    Ok(())
}

fn persist_merge_conflict(
    transaction: &Transaction<'_>,
    conflict: Option<&SyncConflict>,
    result: &mut SyncRemoteApplyResult,
) -> Result<(), DatabaseError> {
    if let Some(conflict) = conflict {
        insert_conflict(transaction, conflict)?;
        result.conflict_count += 1;
    }
    Ok(())
}

fn insert_conflict(
    transaction: &Transaction<'_>,
    conflict: &SyncConflict,
) -> Result<(), DatabaseError> {
    let entity_kind = entity_kind_json(conflict.winner.entity.kind)?;
    let field_name = operation_field(&conflict.winner.kind);
    transaction.execute(
        "INSERT INTO sync_conflicts (
            conflict_id, entity_kind, entity_id, field_name,
            conflict_json, created_at, resolved_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
        params![
            uuid::Uuid::new_v4().to_string(),
            entity_kind,
            conflict.winner.entity.id,
            field_name,
            serde_json::to_string(conflict)?,
            u64_to_i64(
                conflict
                    .winner
                    .version
                    .clock
                    .physical_ms
                    .max(conflict.loser.version.clock.physical_ms,),
                "conflict creation time",
            )?,
        ],
    )?;
    Ok(())
}

fn validate_identifier(value: &str, label: &str) -> Result<(), SyncError> {
    if value.trim().is_empty() {
        Err(SyncError::LocalRepository(format!(
            "Sync {label} must not be empty."
        )))
    } else {
        Ok(())
    }
}

fn validate_operation(operation: &SyncOperation) -> Result<(), SyncError> {
    if operation.operation_id.trim().is_empty()
        || operation.source_device_id.trim().is_empty()
        || operation.entity.id.trim().is_empty()
    {
        Err(SyncError::InvalidOperation(
            "Sync operation IDs and entity ID must not be empty.".to_string(),
        ))
    } else {
        Ok(())
    }
}

fn canonicalize_operation(operation: &SyncOperation) -> SyncOperation {
    let mut operation = operation.clone();
    if operation.entity.kind == SyncEntityKind::Project {
        operation.entity.kind = SyncEntityKind::Tag;
    }
    operation
}

fn operation_field(kind: &SyncOperationKind) -> &str {
    kind.field().unwrap_or(DELETE_FIELD)
}

fn operation_allowed(preset: SyncPresetV1, operation: &SyncOperation) -> bool {
    if !preset_allows(preset, operation.entity.kind) {
        return false;
    }
    let Some(field) = operation.kind.field() else {
        return true;
    };
    match operation.entity.kind {
        SyncEntityKind::Tag | SyncEntityKind::Project => matches!(
            field,
            "name"
                | "description"
                | "icon"
                | "color"
                | "sortOrder"
                | "createdAt"
                | "updatedAt"
                | "summaryTemplateId"
                | "translationLanguage"
                | "polishPresetId"
                | "polishScenario"
                | "polishContext"
                | "exportFileNamePrefix"
                | "enabledTextReplacementSetIds"
                | "enabledHotwordSetIds"
                | "enabledPolishKeywordSetIds"
                | "enabledSpeakerProfileIds"
        ),
        SyncEntityKind::HistoryItem => matches!(
            field,
            "timestamp"
                | "duration"
                | "title"
                | "previewText"
                | "icon"
                | "kind"
                | "tagIds"
                | "deletedAt"
                | "projectId"
        ),
        SyncEntityKind::HistoryTranscript | SyncEntityKind::HistorySummary => field == "document",
        SyncEntityKind::TranscriptSnapshot => {
            matches!(field, "document" | "reason" | "createdAt" | "segmentCount")
        }
        SyncEntityKind::Setting => {
            field == "value" && portable_setting_entity_key(&operation.entity.id).is_some()
        }
        SyncEntityKind::SummaryTemplate => matches!(
            field,
            "name" | "instructions" | "sortOrder" | "createdAt" | "updatedAt"
        ),
        SyncEntityKind::PolishPreset => matches!(
            field,
            "name" | "context" | "sortOrder" | "createdAt" | "updatedAt"
        ),
        SyncEntityKind::VocabularySet => matches!(
            field,
            "name"
                | "enabled"
                | "ignoreCase"
                | "keywords"
                | "sortOrder"
                | "createdAt"
                | "updatedAt"
        ),
        SyncEntityKind::VocabularyRule => {
            matches!(field, "from" | "to" | "text" | "sortOrder")
        }
        SyncEntityKind::SpeakerProfile => matches!(
            field,
            "name" | "enabled" | "sortOrder" | "createdAt" | "updatedAt"
        ),
        SyncEntityKind::AutomationRule => matches!(
            field,
            "name"
                | "projectId"
                | "saveHistory"
                | "tagIds"
                | "presetId"
                | "recursive"
                | "stageAutoPolish"
                | "stagePolishPresetId"
                | "stageAutoTranslate"
                | "stageTranslationLanguage"
                | "stageExportEnabled"
                | "exportFormat"
                | "exportMode"
                | "exportPrefix"
                | "createdAt"
                | "updatedAt"
        ),
        SyncEntityKind::CredentialProfile => {
            matches!(
                field,
                "providerId" | "authorizedDeviceId" | "authorizedAtMs"
            )
        }
    }
}

fn apply_domain_operation(
    transaction: &Transaction<'_>,
    operation: &SyncOperation,
) -> Result<(), DatabaseError> {
    match &operation.kind {
        SyncOperationKind::DeleteEntity => {
            apply_domain_delete(transaction, operation.entity.kind, &operation.entity.id)
        }
        SyncOperationKind::SetField { field, value } => apply_domain_field(
            transaction,
            operation.entity.kind,
            &operation.entity.id,
            field,
            value,
        ),
    }
}

fn apply_domain_delete(
    transaction: &Transaction<'_>,
    kind: SyncEntityKind,
    entity_id: &str,
) -> Result<(), DatabaseError> {
    match kind {
        SyncEntityKind::Tag | SyncEntityKind::Project => {
            execute_delete(transaction, "tags", "id", entity_id)
        }
        SyncEntityKind::HistoryItem => {
            execute_delete(transaction, "history_items", "id", entity_id)
        }
        SyncEntityKind::HistoryTranscript => {
            execute_delete(transaction, "history_transcripts", "history_id", entity_id)
        }
        SyncEntityKind::HistorySummary => {
            execute_delete(transaction, "history_summaries", "history_id", entity_id)
        }
        SyncEntityKind::TranscriptSnapshot => {
            let (history_id, snapshot_id) = split_entity_id(entity_id, "transcript snapshot")?;
            transaction.execute(
                "DELETE FROM transcript_snapshots WHERE history_id = ?1 AND id = ?2",
                params![history_id, snapshot_id],
            )?;
            Ok(())
        }
        SyncEntityKind::Setting => remove_app_config_field(transaction, entity_id),
        SyncEntityKind::SummaryTemplate => {
            execute_delete(transaction, "summary_templates", "id", entity_id)
        }
        SyncEntityKind::PolishPreset => {
            execute_delete(transaction, "polish_presets", "id", entity_id)
        }
        SyncEntityKind::VocabularySet => {
            let (set_kind, set_id) = split_entity_id(entity_id, "vocabulary set")?;
            transaction.execute(
                "DELETE FROM vocabulary_sets WHERE kind = ?1 AND id = ?2",
                params![set_kind, set_id],
            )?;
            Ok(())
        }
        SyncEntityKind::VocabularyRule => {
            let (set_kind, rest) = split_entity_id(entity_id, "vocabulary rule")?;
            let (set_id, rule_id) = split_entity_id(rest, "vocabulary rule")?;
            transaction.execute(
                "DELETE FROM vocabulary_rules
                 WHERE set_kind = ?1 AND set_id = ?2 AND id = ?3",
                params![set_kind, set_id, rule_id],
            )?;
            Ok(())
        }
        SyncEntityKind::SpeakerProfile => {
            execute_delete(transaction, "speaker_profiles", "id", entity_id)
        }
        SyncEntityKind::AutomationRule => {
            execute_delete(transaction, "automation_rules", "id", entity_id)
        }
        SyncEntityKind::CredentialProfile => Ok(()),
    }
}

fn apply_domain_field(
    transaction: &Transaction<'_>,
    kind: SyncEntityKind,
    entity_id: &str,
    field: &str,
    value: &Value,
) -> Result<(), DatabaseError> {
    match kind {
        SyncEntityKind::Tag | SyncEntityKind::Project => {
            apply_tag_field(transaction, entity_id, field, value, kind)
        }
        SyncEntityKind::HistoryItem => {
            apply_history_item_field(transaction, entity_id, field, value)
        }
        SyncEntityKind::HistoryTranscript => {
            require_field(field, "document")?;
            let document = serde_json::to_string(value)?;
            transaction.execute(
                "INSERT INTO history_transcripts (history_id, segments) VALUES (?1, ?2)
                 ON CONFLICT(history_id) DO UPDATE SET segments = excluded.segments",
                params![entity_id, document],
            )?;
            Ok(())
        }
        SyncEntityKind::HistorySummary => {
            require_field(field, "document")?;
            let document = serde_json::to_string(value)?;
            transaction.execute(
                "INSERT INTO history_summaries (history_id, payload) VALUES (?1, ?2)
                 ON CONFLICT(history_id) DO UPDATE SET payload = excluded.payload",
                params![entity_id, document],
            )?;
            Ok(())
        }
        SyncEntityKind::TranscriptSnapshot => {
            apply_transcript_snapshot_field(transaction, entity_id, field, value)
        }
        SyncEntityKind::Setting => apply_app_config_field(transaction, entity_id, field, value),
        SyncEntityKind::SummaryTemplate => apply_simple_record_field(
            transaction,
            "summary_templates",
            entity_id,
            field,
            value,
            &[
                ("name", "name"),
                ("instructions", "instructions"),
                ("sortOrder", "sort_order"),
                ("createdAt", "created_at"),
                ("updatedAt", "updated_at"),
            ],
        ),
        SyncEntityKind::PolishPreset => apply_simple_record_field(
            transaction,
            "polish_presets",
            entity_id,
            field,
            value,
            &[
                ("name", "name"),
                ("context", "context"),
                ("sortOrder", "sort_order"),
                ("createdAt", "created_at"),
                ("updatedAt", "updated_at"),
            ],
        ),
        SyncEntityKind::VocabularySet => {
            apply_vocabulary_set_field(transaction, entity_id, field, value)
        }
        SyncEntityKind::VocabularyRule => {
            apply_vocabulary_rule_field(transaction, entity_id, field, value)
        }
        SyncEntityKind::SpeakerProfile => apply_simple_record_field(
            transaction,
            "speaker_profiles",
            entity_id,
            field,
            value,
            &[
                ("name", "name"),
                ("enabled", "enabled"),
                ("sortOrder", "sort_order"),
                ("createdAt", "created_at"),
                ("updatedAt", "updated_at"),
            ],
        ),
        SyncEntityKind::AutomationRule => {
            apply_automation_rule_field(transaction, entity_id, field, value)
        }
        SyncEntityKind::CredentialProfile => Ok(()),
    }
}

fn apply_tag_field(
    transaction: &Transaction<'_>,
    entity_id: &str,
    field: &str,
    value: &Value,
    kind: SyncEntityKind,
) -> Result<(), DatabaseError> {
    transaction.execute(
        "INSERT OR IGNORE INTO tags (id, created_at, updated_at) VALUES (?1, 0, 0)",
        [entity_id],
    )?;
    if let Some(link_kind) = match field {
        "enabledTextReplacementSetIds" => Some("text_replacement"),
        "enabledHotwordSetIds" => Some("hotword"),
        "enabledPolishKeywordSetIds" => Some("polish_keyword"),
        "enabledSpeakerProfileIds" => Some("speaker_profile"),
        _ => None,
    } {
        let targets = json_string_array(value, field)?;
        transaction.execute(
            "DELETE FROM tag_default_links WHERE tag_id = ?1 AND kind = ?2",
            params![entity_id, link_kind],
        )?;
        for (sort_order, target_id) in targets.iter().enumerate() {
            transaction.execute(
                "INSERT INTO tag_default_links (tag_id, kind, target_id, sort_order)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    entity_id,
                    link_kind,
                    target_id,
                    usize_to_i64(sort_order, "sort order")?
                ],
            )?;
        }
        return Ok(());
    }
    let column = match field {
        "name" => "name",
        "description" => "description",
        "icon" => "icon",
        "color" => "color",
        "sortOrder" => "sort_order",
        "createdAt" => "created_at",
        "updatedAt" => "updated_at",
        "summaryTemplateId" => "summary_template_id",
        "translationLanguage" => "translation_language",
        "polishPresetId" => "polish_preset_id",
        "polishScenario" => "polish_scenario",
        "polishContext" => "polish_context",
        "exportFileNamePrefix" => "export_file_name_prefix",
        _ => return unsupported_field(kind, field),
    };
    update_json_column(transaction, "tags", "id", entity_id, column, value)
}

fn apply_history_item_field(
    transaction: &Transaction<'_>,
    entity_id: &str,
    field: &str,
    value: &Value,
) -> Result<(), DatabaseError> {
    transaction.execute(
        "INSERT OR IGNORE INTO history_items (id, timestamp) VALUES (?1, 0)",
        [entity_id],
    )?;
    if field == "tagIds" {
        let tag_ids = json_string_array(value, field)?;
        transaction.execute(
            "DELETE FROM history_item_tags WHERE history_id = ?1",
            [entity_id],
        )?;
        for tag_id in tag_ids {
            transaction.execute(
                "INSERT OR IGNORE INTO history_item_tags (history_id, tag_id) VALUES (?1, ?2)",
                params![entity_id, tag_id],
            )?;
        }
        return Ok(());
    }
    if field == "projectId" {
        transaction.execute(
            "DELETE FROM history_item_tags WHERE history_id = ?1",
            [entity_id],
        )?;
        if let Some(tag_id) = value.as_str().filter(|tag_id| !tag_id.is_empty()) {
            transaction.execute(
                "INSERT OR IGNORE INTO history_item_tags (history_id, tag_id) VALUES (?1, ?2)",
                params![entity_id, tag_id],
            )?;
        }
        return Ok(());
    }
    let column = match field {
        "timestamp" => "timestamp",
        "duration" => "duration",
        "title" => "title",
        "previewText" => "preview_text",
        "icon" => "icon",
        "kind" => "kind",
        "deletedAt" => "deleted_at",
        _ => return unsupported_field(SyncEntityKind::HistoryItem, field),
    };
    update_json_column(transaction, "history_items", "id", entity_id, column, value)
}

fn apply_transcript_snapshot_field(
    transaction: &Transaction<'_>,
    entity_id: &str,
    field: &str,
    value: &Value,
) -> Result<(), DatabaseError> {
    let (history_id, snapshot_id) = split_entity_id(entity_id, "transcript snapshot")?;
    transaction.execute(
        "INSERT OR IGNORE INTO transcript_snapshots
         (id, history_id, reason, created_at, segment_count, segments)
         VALUES (?1, ?2, 'sync', 0, 0, '[]')",
        params![snapshot_id, history_id],
    )?;
    let (column, encoded) = match field {
        "document" => ("segments", Value::String(serde_json::to_string(value)?)),
        "reason" => ("reason", value.clone()),
        "createdAt" => ("created_at", value.clone()),
        "segmentCount" => ("segment_count", value.clone()),
        _ => return unsupported_field(SyncEntityKind::TranscriptSnapshot, field),
    };
    update_json_composite_column(
        transaction,
        "transcript_snapshots",
        history_id,
        snapshot_id,
        column,
        &encoded,
    )
}

fn apply_app_config_field(
    transaction: &Transaction<'_>,
    entity_id: &str,
    field: &str,
    value: &Value,
) -> Result<(), DatabaseError> {
    let Some(setting_key) = portable_setting_entity_key(entity_id) else {
        return unsupported_field(SyncEntityKind::Setting, field);
    };
    require_field(field, "value")?;
    let current = transaction
        .query_row("SELECT config FROM app_config WHERE id = 1", [], |row| {
            row.get::<_, String>(0)
        })
        .optional()?
        .unwrap_or_else(|| "{}".to_string());
    let mut config = serde_json::from_str::<Value>(&current)?;
    let object = config.as_object_mut().ok_or_else(|| {
        DatabaseError::Internal("Stored app config is not a JSON object.".to_string())
    })?;
    object.insert(setting_key.to_string(), value.clone());
    transaction.execute(
        "INSERT INTO app_config (id, config) VALUES (1, ?1)
         ON CONFLICT(id) DO UPDATE SET config = excluded.config",
        [serde_json::to_string(&config)?],
    )?;
    Ok(())
}

fn remove_app_config_field(
    transaction: &Transaction<'_>,
    entity_id: &str,
) -> Result<(), DatabaseError> {
    let setting_key = portable_setting_entity_key(entity_id)
        .ok_or_else(|| DatabaseError::Internal("Sync setting entity ID is invalid.".to_string()))?;
    let Some(current) = transaction
        .query_row("SELECT config FROM app_config WHERE id = 1", [], |row| {
            row.get::<_, String>(0)
        })
        .optional()?
    else {
        return Ok(());
    };
    let mut config = serde_json::from_str::<Value>(&current)?;
    let object = config.as_object_mut().ok_or_else(|| {
        DatabaseError::Internal("Stored app config is not a JSON object.".to_string())
    })?;
    object.remove(setting_key);
    transaction.execute(
        "UPDATE app_config SET config = ?1 WHERE id = 1",
        [serde_json::to_string(&config)?],
    )?;
    Ok(())
}

fn operation_allowed_field_for_setting(field: &str) -> bool {
    matches!(
        field,
        "language"
            | "enableTimeline"
            | "enableITN"
            | "batchVadEnabled"
            | "vadBufferSize"
            | "maxConcurrent"
            | "llmSettings"
            | "asr"
            | "summaryEnabled"
            | "summaryTemplateId"
            | "translationLanguage"
            | "polishKeywords"
            | "polishPresetId"
            | "polishContext"
            | "polishScenario"
            | "autoPolish"
            | "autoPolishFrequency"
    )
}

fn portable_setting_entity_key(entity_id: &str) -> Option<&str> {
    let setting_key = entity_id.strip_prefix("app-config::")?;
    operation_allowed_field_for_setting(setting_key).then_some(setting_key)
}

fn apply_simple_record_field(
    transaction: &Transaction<'_>,
    table: &str,
    entity_id: &str,
    field: &str,
    value: &Value,
    fields: &[(&str, &str)],
) -> Result<(), DatabaseError> {
    let insert_sql = format!("INSERT OR IGNORE INTO {table} (id) VALUES (?1)");
    transaction.execute(&insert_sql, [entity_id])?;
    let Some((_, column)) = fields.iter().find(|(candidate, _)| *candidate == field) else {
        return Err(DatabaseError::Internal(format!(
            "Unsupported sync field {field} for {table}."
        )));
    };
    update_json_column(transaction, table, "id", entity_id, column, value)
}

fn apply_vocabulary_set_field(
    transaction: &Transaction<'_>,
    entity_id: &str,
    field: &str,
    value: &Value,
) -> Result<(), DatabaseError> {
    let (set_kind, set_id) = split_entity_id(entity_id, "vocabulary set")?;
    if !matches!(set_kind, "text_replacement" | "hotword" | "polish_keyword") {
        return Err(DatabaseError::Internal(
            "Sync vocabulary set kind is invalid.".to_string(),
        ));
    }
    transaction.execute(
        "INSERT OR IGNORE INTO vocabulary_sets (id, kind) VALUES (?1, ?2)",
        params![set_id, set_kind],
    )?;
    let column = match field {
        "name" => "name",
        "enabled" => "enabled",
        "ignoreCase" => "ignore_case",
        "keywords" => "keywords",
        "sortOrder" => "sort_order",
        "createdAt" => "created_at",
        "updatedAt" => "updated_at",
        _ => return unsupported_field(SyncEntityKind::VocabularySet, field),
    };
    let sql = format!("UPDATE vocabulary_sets SET {column} = ?1 WHERE kind = ?2 AND id = ?3");
    transaction.execute(&sql, params![json_to_sql_value(value)?, set_kind, set_id])?;
    Ok(())
}

fn apply_vocabulary_rule_field(
    transaction: &Transaction<'_>,
    entity_id: &str,
    field: &str,
    value: &Value,
) -> Result<(), DatabaseError> {
    let (set_kind, rest) = split_entity_id(entity_id, "vocabulary rule")?;
    let (set_id, rule_id) = split_entity_id(rest, "vocabulary rule")?;
    transaction.execute(
        "INSERT OR IGNORE INTO vocabulary_rules (id, set_kind, set_id)
         VALUES (?1, ?2, ?3)",
        params![rule_id, set_kind, set_id],
    )?;
    let column = match field {
        "from" => "from_text",
        "to" => "to_text",
        "text" => "text",
        "sortOrder" => "sort_order",
        _ => return unsupported_field(SyncEntityKind::VocabularyRule, field),
    };
    let sql = format!(
        "UPDATE vocabulary_rules SET {column} = ?1
         WHERE set_kind = ?2 AND set_id = ?3 AND id = ?4"
    );
    transaction.execute(
        &sql,
        params![json_to_sql_value(value)?, set_kind, set_id, rule_id],
    )?;
    Ok(())
}

fn apply_automation_rule_field(
    transaction: &Transaction<'_>,
    entity_id: &str,
    field: &str,
    value: &Value,
) -> Result<(), DatabaseError> {
    transaction.execute(
        "INSERT OR IGNORE INTO automation_rules (id) VALUES (?1)",
        [entity_id],
    )?;
    if field == "tagIds" {
        let tag_ids = json_string_array(value, field)?;
        transaction.execute(
            "DELETE FROM automation_rule_tags WHERE rule_id = ?1",
            [entity_id],
        )?;
        for tag_id in tag_ids {
            transaction.execute(
                "INSERT OR IGNORE INTO automation_rule_tags (rule_id, tag_id) VALUES (?1, ?2)",
                params![entity_id, tag_id],
            )?;
        }
        return Ok(());
    }
    if field == "projectId" {
        let project_id = value.as_str().unwrap_or_default();
        transaction.execute(
            "DELETE FROM automation_rule_tags WHERE rule_id = ?1",
            [entity_id],
        )?;
        transaction.execute(
            "UPDATE automation_rules SET save_history = ?1 WHERE id = ?2",
            params![(project_id != "none") as i64, entity_id],
        )?;
        if !matches!(project_id, "" | "inbox" | "none") {
            transaction.execute(
                "INSERT OR IGNORE INTO automation_rule_tags (rule_id, tag_id) VALUES (?1, ?2)",
                params![entity_id, project_id],
            )?;
        }
        return Ok(());
    }
    let column = match field {
        "name" => "name",
        "saveHistory" => "save_history",
        "presetId" => "preset_id",
        "recursive" => "recursive",
        "stageAutoPolish" => "stage_auto_polish",
        "stagePolishPresetId" => "stage_polish_preset_id",
        "stageAutoTranslate" => "stage_auto_translate",
        "stageTranslationLanguage" => "stage_translation_language",
        "stageExportEnabled" => "stage_export_enabled",
        "exportFormat" => "export_format",
        "exportMode" => "export_mode",
        "exportPrefix" => "export_prefix",
        "createdAt" => "created_at",
        "updatedAt" => "updated_at",
        _ => return unsupported_field(SyncEntityKind::AutomationRule, field),
    };
    update_json_column(
        transaction,
        "automation_rules",
        "id",
        entity_id,
        column,
        value,
    )
}

fn update_json_column(
    transaction: &Transaction<'_>,
    table: &str,
    id_column: &str,
    entity_id: &str,
    column: &str,
    value: &Value,
) -> Result<(), DatabaseError> {
    let sql = format!("UPDATE {table} SET {column} = ?1 WHERE {id_column} = ?2");
    transaction.execute(&sql, params![json_to_sql_value(value)?, entity_id])?;
    Ok(())
}

fn update_json_composite_column(
    transaction: &Transaction<'_>,
    table: &str,
    history_id: &str,
    entity_id: &str,
    column: &str,
    value: &Value,
) -> Result<(), DatabaseError> {
    let sql = format!("UPDATE {table} SET {column} = ?1 WHERE history_id = ?2 AND id = ?3");
    transaction.execute(
        &sql,
        params![json_to_sql_value(value)?, history_id, entity_id],
    )?;
    Ok(())
}

fn json_to_sql_value(value: &Value) -> Result<rusqlite::types::Value, DatabaseError> {
    Ok(match value {
        Value::Null => rusqlite::types::Value::Null,
        Value::Bool(value) => rusqlite::types::Value::Integer(i64::from(*value)),
        Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                rusqlite::types::Value::Integer(value)
            } else if let Some(value) = value.as_u64() {
                rusqlite::types::Value::Integer(i64::try_from(value).map_err(|error| {
                    DatabaseError::Internal(format!("Sync integer value is out of range: {error}"))
                })?)
            } else {
                rusqlite::types::Value::Real(value.as_f64().ok_or_else(|| {
                    DatabaseError::Internal("Sync numeric value is invalid.".to_string())
                })?)
            }
        }
        Value::String(value) => rusqlite::types::Value::Text(value.clone()),
        Value::Array(_) | Value::Object(_) => {
            rusqlite::types::Value::Text(serde_json::to_string(value)?)
        }
    })
}

fn execute_delete(
    transaction: &Transaction<'_>,
    table: &str,
    id_column: &str,
    entity_id: &str,
) -> Result<(), DatabaseError> {
    let sql = format!("DELETE FROM {table} WHERE {id_column} = ?1");
    transaction.execute(&sql, [entity_id])?;
    Ok(())
}

fn require_field(field: &str, expected: &str) -> Result<(), DatabaseError> {
    if field == expected {
        Ok(())
    } else {
        Err(DatabaseError::Internal(format!(
            "Expected sync field {expected}, found {field}."
        )))
    }
}

fn split_entity_id<'a>(
    entity_id: &'a str,
    label: &str,
) -> Result<(&'a str, &'a str), DatabaseError> {
    entity_id
        .split_once("::")
        .ok_or_else(|| DatabaseError::Internal(format!("Sync {label} ID is malformed.")))
}

fn json_string_array(value: &Value, field: &str) -> Result<Vec<String>, DatabaseError> {
    value
        .as_array()
        .ok_or_else(|| DatabaseError::Internal(format!("Sync field {field} must be an array.")))?
        .iter()
        .map(|value| {
            value.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                DatabaseError::Internal(format!("Sync field {field} must contain only strings."))
            })
        })
        .collect()
}

fn unsupported_field<T>(kind: SyncEntityKind, field: &str) -> Result<T, DatabaseError> {
    Err(DatabaseError::Internal(format!(
        "Unsupported sync field {field} for {kind:?}."
    )))
}

fn entity_kind_json(kind: SyncEntityKind) -> Result<String, DatabaseError> {
    let kind = if kind == SyncEntityKind::Project {
        SyncEntityKind::Tag
    } else {
        kind
    };
    serde_json::to_string(&kind).map_err(DatabaseError::SerializationError)
}

fn parse_preset(value: String) -> Result<SyncPresetV1, rusqlite::Error> {
    serde_json::from_str(&value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}

fn preset_allows(preset: SyncPresetV1, kind: SyncEntityKind) -> bool {
    match kind {
        SyncEntityKind::Tag
        | SyncEntityKind::Project
        | SyncEntityKind::HistoryItem
        | SyncEntityKind::HistoryTranscript
        | SyncEntityKind::HistorySummary
        | SyncEntityKind::TranscriptSnapshot => true,
        SyncEntityKind::Setting
        | SyncEntityKind::SummaryTemplate
        | SyncEntityKind::PolishPreset
        | SyncEntityKind::VocabularySet
        | SyncEntityKind::VocabularyRule
        | SyncEntityKind::SpeakerProfile => preset != SyncPresetV1::Content,
        SyncEntityKind::AutomationRule | SyncEntityKind::CredentialProfile => {
            preset == SyncPresetV1::Full
        }
    }
}

fn i64_to_u64(value: i64, label: &str) -> Result<u64, rusqlite::Error> {
    u64::try_from(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Integer,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid {label}: {error}"),
            )),
        )
    })
}

fn u64_to_i64(value: u64, label: &str) -> Result<i64, DatabaseError> {
    i64::try_from(value)
        .map_err(|error| DatabaseError::Internal(format!("Invalid {label}: {error}")))
}

fn usize_to_i64(value: usize, label: &str) -> Result<i64, DatabaseError> {
    i64::try_from(value)
        .map_err(|error| DatabaseError::Internal(format!("Invalid {label}: {error}")))
}

fn sync_database_error(error: DatabaseError) -> SyncError {
    SyncError::LocalRepository(error.to_string())
}

fn sync_serialization_error(error: serde_json::Error) -> SyncError {
    SyncError::LocalRepository(error.to_string())
}
