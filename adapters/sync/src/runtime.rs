use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};
use sona_core::sync::{
    SyncCausalContext, SyncDeleteResult, SyncDeviceCursor, SyncError, SyncLocalRepository,
    SyncLocalRuntimeState, SyncObjectKey, SyncObjectPrefix, SyncObjectStore,
    SyncPublishedCheckpoint, SyncPublishedSegment, SyncPutResult, SyncRemoteSegment, SyncRunResult,
};

use crate::{
    MAX_SEGMENT_ENCODED_BYTES, MAX_SEGMENT_OPERATIONS, SyncCheckpointV1, SyncSegmentV1,
    checkpoint_object_key, open_json, seal_json, segment_object_key, should_publish_checkpoint,
};

const SEGMENT_RETENTION_MS: u64 = 30 * 24 * 60 * 60 * 1_000;

/// Downloads and validates the remote state needed to preview joining a vault.
/// The returned batches are not applied to a local repository.
pub async fn load_remote_state_for_join(
    remote: &dyn SyncObjectStore,
    vault_id: &str,
    vault_key: &[u8],
) -> Result<Vec<SyncRemoteSegment>, SyncError> {
    let prefix = SyncObjectPrefix::parse(format!("sona-sync/v1/{vault_id}/devices"))?;
    let mut continuation = None;
    let mut checkpoints = BTreeMap::<String, Vec<(ParsedCheckpointKey, SyncObjectKey)>>::new();
    let mut segments = BTreeMap::<String, Vec<(ParsedSegmentKey, SyncObjectKey)>>::new();
    let mut device_ids = BTreeSet::new();

    loop {
        let page = remote.list(&prefix, continuation.as_deref()).await?;
        for metadata in page.objects {
            if let Some(parsed) = parse_checkpoint_key(&metadata.key, vault_id)? {
                device_ids.insert(parsed.device_id.clone());
                checkpoints
                    .entry(parsed.device_id.clone())
                    .or_default()
                    .push((parsed, metadata.key));
                continue;
            }
            if let Some(parsed) = parse_segment_key(&metadata.key, vault_id)? {
                device_ids.insert(parsed.device_id.clone());
                segments
                    .entry(parsed.device_id.clone())
                    .or_default()
                    .push((parsed, metadata.key));
            }
        }
        match page.continuation {
            Some(next) if !next.is_empty() => continuation = Some(next),
            _ => break,
        }
    }

    let mut cursors = BTreeMap::<String, SyncDeviceCursor>::new();
    let mut batches = Vec::new();
    for device_id in &device_ids {
        let candidates = checkpoints.entry(device_id.clone()).or_default();
        candidates.sort_by(|(left, _), (right, _)| {
            right
                .sequence
                .cmp(&left.sequence)
                .then_with(|| left.cipher_hash.cmp(&right.cipher_hash))
        });
        if candidates.len() > 1 && candidates[0].0.sequence == candidates[1].0.sequence {
            return Err(protocol_error(format!(
                "Remote device {device_id} has multiple checkpoints at sequence {}.",
                candidates[0].0.sequence
            )));
        }

        let Some((parsed, key)) = candidates.first() else {
            cursors.insert(device_id.clone(), SyncDeviceCursor::default());
            continue;
        };
        let object = remote
            .get(key)
            .await?
            .ok_or_else(|| protocol_error(format!("Remote checkpoint disappeared: {key}.")))?;
        if cipher_hash(&object.bytes) != parsed.cipher_hash {
            return Err(protocol_error(format!(
                "Remote checkpoint cipher hash does not match its key: {key}."
            )));
        }
        let aad_key = checkpoint_aad_key_parts(vault_id, device_id, parsed.sequence)?;
        let checkpoint: SyncCheckpointV1 =
            open_json(vault_key, aad_key.as_str().as_bytes(), &object.bytes)?;
        checkpoint.validate()?;
        validate_remote_checkpoint(&checkpoint, parsed, vault_id)?;
        cursors.insert(
            device_id.clone(),
            SyncDeviceCursor {
                sequence: checkpoint.sequence,
                cipher_hash: checkpoint.covered_segment_cipher_hash.clone(),
            },
        );
        batches.push(SyncRemoteSegment {
            device_id: device_id.clone(),
            sequence: checkpoint.sequence,
            cipher_hash: checkpoint.covered_segment_cipher_hash,
            operations: checkpoint.operations,
        });
    }

    for device_id in device_ids {
        let candidates = segments.entry(device_id.clone()).or_default();
        candidates.sort_by(|(left, _), (right, _)| {
            left.sequence
                .cmp(&right.sequence)
                .then_with(|| left.cipher_hash.cmp(&right.cipher_hash))
        });
        let mut cursor = cursors.remove(&device_id).unwrap_or_default();
        for window in candidates.windows(2) {
            if window[0].0.sequence == window[1].0.sequence {
                return Err(protocol_error(format!(
                    "Remote device {device_id} has multiple segments at sequence {}.",
                    window[0].0.sequence
                )));
            }
        }
        for (parsed, key) in candidates.iter() {
            if parsed.sequence <= cursor.sequence {
                continue;
            }
            if parsed.sequence != cursor.sequence.saturating_add(1) {
                return Err(protocol_error(format!(
                    "Remote segment chain has a gap for device {device_id}: expected {}, found {}.",
                    cursor.sequence.saturating_add(1),
                    parsed.sequence
                )));
            }
            let object = remote
                .get(key)
                .await?
                .ok_or_else(|| protocol_error(format!("Remote segment disappeared: {key}.")))?;
            if cipher_hash(&object.bytes) != parsed.cipher_hash {
                return Err(protocol_error(format!(
                    "Remote segment cipher hash does not match its key: {key}."
                )));
            }
            let aad_key = segment_aad_key_parts(vault_id, &device_id, parsed.sequence)?;
            let segment: SyncSegmentV1 =
                open_json(vault_key, aad_key.as_str().as_bytes(), &object.bytes)?;
            segment.validate()?;
            validate_remote_segment(&segment, parsed, &cursor, vault_id)?;
            cursor = SyncDeviceCursor {
                sequence: parsed.sequence,
                cipher_hash: parsed.cipher_hash.clone(),
            };
            batches.push(SyncRemoteSegment {
                device_id: device_id.clone(),
                sequence: parsed.sequence,
                cipher_hash: parsed.cipher_hash.clone(),
                operations: segment.operations,
            });
        }
    }

    Ok(batches)
}

pub struct SyncRuntime<'a> {
    local: &'a dyn SyncLocalRepository,
    remote: &'a dyn SyncObjectStore,
    vault_key: &'a [u8],
}

impl<'a> SyncRuntime<'a> {
    pub fn new(
        local: &'a dyn SyncLocalRepository,
        remote: &'a dyn SyncObjectStore,
        vault_key: &'a [u8],
    ) -> Self {
        Self {
            local,
            remote,
            vault_key,
        }
    }

    pub async fn run_at(&self, now_ms: u64) -> Result<SyncRunResult, SyncError> {
        let mut state = self.local.load_runtime_state()?;
        validate_runtime_state(&state)?;
        let mut result = SyncRunResult::default();

        self.pull_remote_checkpoints(&mut state, &mut result)
            .await?;
        self.pull_remote_segments(&mut state, &mut result).await?;
        self.push_pending_segment(&mut state, now_ms, &mut result)
            .await?;
        self.publish_checkpoint_if_due(&state, now_ms, &mut result)
            .await?;
        self.collect_garbage(&state, now_ms).await?;

        Ok(result)
    }

    async fn pull_remote_checkpoints(
        &self,
        state: &mut SyncLocalRuntimeState,
        result: &mut SyncRunResult,
    ) -> Result<(), SyncError> {
        let prefix = SyncObjectPrefix::parse(format!("sona-sync/v1/{}/devices", state.vault_id))?;
        let mut continuation = None;
        let mut latest_by_device = BTreeMap::<String, (ParsedCheckpointKey, SyncObjectKey)>::new();
        loop {
            let page = self.remote.list(&prefix, continuation.as_deref()).await?;
            for metadata in page.objects {
                let Some(parsed) = parse_checkpoint_key(&metadata.key, &state.vault_id)? else {
                    continue;
                };
                if parsed.device_id == state.device_id {
                    continue;
                }
                let cursor_sequence = state
                    .remote_cursors
                    .get(&parsed.device_id)
                    .map_or(0, |cursor| cursor.sequence);
                if parsed.sequence <= cursor_sequence {
                    continue;
                }
                match latest_by_device.get(&parsed.device_id) {
                    Some((current, _)) if current.sequence > parsed.sequence => {}
                    Some((current, _))
                        if current.sequence == parsed.sequence
                            && current.cipher_hash <= parsed.cipher_hash => {}
                    _ => {
                        latest_by_device.insert(parsed.device_id.clone(), (parsed, metadata.key));
                    }
                }
            }
            match page.continuation {
                Some(next) if !next.is_empty() => continuation = Some(next),
                _ => break,
            }
        }

        for (device_id, (parsed, key)) in latest_by_device {
            let object =
                self.remote.get(&key).await?.ok_or_else(|| {
                    protocol_error(format!("Remote checkpoint disappeared: {key}."))
                })?;
            let actual_hash = cipher_hash(&object.bytes);
            if actual_hash != parsed.cipher_hash {
                return Err(protocol_error(format!(
                    "Remote checkpoint cipher hash does not match its key: {key}."
                )));
            }
            let aad_key =
                checkpoint_aad_key_parts(&state.vault_id, &parsed.device_id, parsed.sequence)?;
            let checkpoint: SyncCheckpointV1 =
                open_json(self.vault_key, aad_key.as_str().as_bytes(), &object.bytes)?;
            checkpoint.validate()?;
            validate_remote_checkpoint(&checkpoint, &parsed, &state.vault_id)?;
            let remote = SyncRemoteSegment {
                device_id: device_id.clone(),
                sequence: checkpoint.sequence,
                cipher_hash: checkpoint.covered_segment_cipher_hash.clone(),
                operations: checkpoint.operations,
            };
            let applied = self.local.apply_remote_segment(&remote)?;
            state.remote_cursors.insert(
                device_id,
                SyncDeviceCursor {
                    sequence: remote.sequence,
                    cipher_hash: remote.cipher_hash,
                },
            );
            result.pulled_checkpoint_count += 1;
            result.applied_operation_count += applied.applied_operation_count;
            result.conflict_count += applied.conflict_count;
        }

        Ok(())
    }

    async fn pull_remote_segments(
        &self,
        state: &mut SyncLocalRuntimeState,
        result: &mut SyncRunResult,
    ) -> Result<(), SyncError> {
        let prefix = SyncObjectPrefix::parse(format!("sona-sync/v1/{}/devices", state.vault_id))?;
        let mut continuation = None;
        let mut remote_segments = Vec::new();
        loop {
            let page = self.remote.list(&prefix, continuation.as_deref()).await?;
            for metadata in page.objects {
                if let Some(parsed) = parse_segment_key(&metadata.key, &state.vault_id)?
                    && parsed.device_id != state.device_id
                {
                    remote_segments.push((parsed, metadata.key));
                }
            }
            match page.continuation {
                Some(next) if !next.is_empty() => continuation = Some(next),
                _ => break,
            }
        }
        remote_segments.sort_by(|(left, _), (right, _)| {
            left.device_id
                .cmp(&right.device_id)
                .then_with(|| left.sequence.cmp(&right.sequence))
        });

        for (parsed, key) in remote_segments {
            let cursor = state
                .remote_cursors
                .get(&parsed.device_id)
                .cloned()
                .unwrap_or_default();
            if parsed.sequence <= cursor.sequence {
                continue;
            }
            if parsed.sequence != cursor.sequence.saturating_add(1) {
                return Err(protocol_error(format!(
                    "Remote segment chain has a gap for device {}: expected {}, found {}.",
                    parsed.device_id,
                    cursor.sequence.saturating_add(1),
                    parsed.sequence
                )));
            }
            let object = self
                .remote
                .get(&key)
                .await?
                .ok_or_else(|| protocol_error(format!("Remote segment disappeared: {key}.")))?;
            let actual_hash = cipher_hash(&object.bytes);
            if actual_hash != parsed.cipher_hash {
                return Err(protocol_error(format!(
                    "Remote segment cipher hash does not match its key: {key}."
                )));
            }
            let aad_key =
                segment_aad_key_parts(&state.vault_id, &parsed.device_id, parsed.sequence)?;
            let segment: SyncSegmentV1 =
                open_json(self.vault_key, aad_key.as_str().as_bytes(), &object.bytes)?;
            segment.validate()?;
            validate_remote_segment(&segment, &parsed, &cursor, &state.vault_id)?;
            let remote = SyncRemoteSegment {
                device_id: parsed.device_id.clone(),
                sequence: parsed.sequence,
                cipher_hash: parsed.cipher_hash.clone(),
                operations: segment.operations,
            };
            let applied = self.local.apply_remote_segment(&remote)?;
            state.remote_cursors.insert(
                parsed.device_id,
                SyncDeviceCursor {
                    sequence: parsed.sequence,
                    cipher_hash: parsed.cipher_hash,
                },
            );
            result.pulled_segment_count += 1;
            result.applied_operation_count += applied.applied_operation_count;
            result.conflict_count += applied.conflict_count;
        }

        Ok(())
    }

    async fn push_pending_segment(
        &self,
        state: &mut SyncLocalRuntimeState,
        now_ms: u64,
        result: &mut SyncRunResult,
    ) -> Result<(), SyncError> {
        let mut operations = self.local.load_pending_operations(
            state.preset,
            MAX_SEGMENT_OPERATIONS,
            MAX_SEGMENT_ENCODED_BYTES,
        )?;
        if operations.is_empty() {
            return Ok(());
        }
        for operation in &mut operations {
            if operation.source_device_id != state.device_id
                || operation.version.device_id != state.device_id
            {
                return Err(protocol_error(
                    "Pending operation source device does not match the local device.",
                ));
            }
            operation.source_sequence = state.next_sequence;
        }
        let segment = SyncSegmentV1 {
            protocol_version: sona_core::sync::SYNC_PROTOCOL_VERSION,
            vault_id: state.vault_id.clone(),
            device_id: state.device_id.clone(),
            sequence: state.next_sequence,
            previous_cipher_hash: state.previous_cipher_hash.clone(),
            created_at_ms: now_ms,
            operations,
        };
        segment.validate()?;

        if let Some((hash, encrypted_bytes)) = self.find_existing_segment(&segment).await? {
            self.finish_segment_publish(segment, hash, encrypted_bytes, state, result)?;
            return Ok(());
        }

        let aad_key = segment_aad_key(&segment)?;
        let sealed = seal_json(self.vault_key, aad_key.as_str().as_bytes(), &segment)?;
        let hash = cipher_hash(&sealed);
        let key = segment_object_key(
            &segment.vault_id,
            &segment.device_id,
            segment.sequence,
            &hash,
        )?;

        match self.remote.put_if_absent(&key, sealed.clone()).await? {
            SyncPutResult::Created { .. } => {}
            SyncPutResult::AlreadyExists { .. } => {
                let existing = self
                    .remote
                    .get(&key)
                    .await?
                    .ok_or_else(|| protocol_error("Existing segment could not be read."))?;
                if existing.bytes != sealed {
                    return Err(protocol_error(
                        "Existing immutable segment does not match the local segment.",
                    ));
                }
            }
            SyncPutResult::Conflict { .. } => {
                return Err(protocol_error(
                    "Immutable segment creation reported an ETag conflict.",
                ));
            }
        }
        self.finish_segment_publish(segment, hash, sealed.len() as u64, state, result)?;
        Ok(())
    }

    async fn find_existing_segment(
        &self,
        intended: &SyncSegmentV1,
    ) -> Result<Option<(String, u64)>, SyncError> {
        let prefix = SyncObjectPrefix::parse(format!(
            "sona-sync/v1/{}/devices/{}/segments",
            intended.vault_id, intended.device_id
        ))?;
        let mut continuation = None;
        let mut candidates = Vec::new();
        loop {
            let page = self.remote.list(&prefix, continuation.as_deref()).await?;
            for metadata in page.objects {
                let Some(parsed) = parse_segment_key(&metadata.key, &intended.vault_id)? else {
                    continue;
                };
                if parsed.device_id == intended.device_id && parsed.sequence == intended.sequence {
                    candidates.push((parsed, metadata.key));
                }
            }
            match page.continuation {
                Some(next) if !next.is_empty() => continuation = Some(next),
                _ => break,
            }
        }
        candidates.sort_by(|(left, _), (right, _)| left.cipher_hash.cmp(&right.cipher_hash));

        if let Some((parsed, key)) = candidates.into_iter().next() {
            let object = self
                .remote
                .get(&key)
                .await?
                .ok_or_else(|| protocol_error(format!("Remote segment disappeared: {key}.")))?;
            if cipher_hash(&object.bytes) != parsed.cipher_hash {
                return Err(protocol_error(format!(
                    "Remote segment cipher hash does not match its key: {key}."
                )));
            }
            let aad_key =
                segment_aad_key_parts(&intended.vault_id, &intended.device_id, intended.sequence)?;
            let existing: SyncSegmentV1 =
                open_json(self.vault_key, aad_key.as_str().as_bytes(), &object.bytes)?;
            existing.validate()?;
            if !same_segment_payload(&existing, intended) {
                return Err(protocol_error(format!(
                    "Remote device sequence {} already contains different operations.",
                    intended.sequence
                )));
            }
            return Ok(Some((parsed.cipher_hash, object.bytes.len() as u64)));
        }
        Ok(None)
    }

    fn finish_segment_publish(
        &self,
        segment: SyncSegmentV1,
        cipher_hash: String,
        encrypted_bytes: u64,
        state: &mut SyncLocalRuntimeState,
        result: &mut SyncRunResult,
    ) -> Result<(), SyncError> {
        self.local.mark_segment_published(&SyncPublishedSegment {
            sequence: segment.sequence,
            cipher_hash: cipher_hash.clone(),
            operations: segment.operations.clone(),
            encrypted_bytes,
        })?;
        state.next_sequence = segment.sequence + 1;
        state.previous_cipher_hash = Some(cipher_hash);
        state.operations_since_checkpoint += segment.operations.len() as u64;
        state.bytes_since_checkpoint += encrypted_bytes;
        result.pushed_segment_count += 1;
        result.published_operation_count += segment.operations.len() as u64;
        Ok(())
    }

    async fn publish_checkpoint_if_due(
        &self,
        state: &SyncLocalRuntimeState,
        now_ms: u64,
        result: &mut SyncRunResult,
    ) -> Result<(), SyncError> {
        if !state.checkpoint_required
            && !should_publish_checkpoint(
                state.operations_since_checkpoint,
                state.bytes_since_checkpoint,
            )
        {
            return Ok(());
        }
        let sequence = state.next_sequence.saturating_sub(1);
        if sequence == 0 {
            return Err(protocol_error(
                "Cannot publish a checkpoint before the first segment.",
            ));
        }
        let covered_segment_cipher_hash = state.previous_cipher_hash.clone().ok_or_else(|| {
            protocol_error("Checkpoint is missing its covered segment cipher hash.")
        })?;
        let operations = self.local.load_checkpoint_operations()?;
        let checkpoint = SyncCheckpointV1 {
            protocol_version: sona_core::sync::SYNC_PROTOCOL_VERSION,
            vault_id: state.vault_id.clone(),
            device_id: state.device_id.clone(),
            sequence,
            covered_segment_cipher_hash,
            created_at_ms: now_ms,
            causal_context: SyncCausalContext {
                observed_sequences: state
                    .remote_cursors
                    .iter()
                    .map(|(device_id, cursor)| (device_id.clone(), cursor.sequence))
                    .collect(),
            },
            operations,
        };
        checkpoint.validate()?;
        if let Some((hash, encrypted_bytes, created_at_ms)) =
            self.find_existing_checkpoint(&checkpoint).await?
        {
            self.local
                .mark_checkpoint_published(&SyncPublishedCheckpoint {
                    sequence,
                    cipher_hash: hash,
                    encrypted_bytes,
                    created_at_ms,
                })?;
            result.checkpoint_published = true;
            return Ok(());
        }
        let aad_key = checkpoint_aad_key(&checkpoint)?;
        let sealed = seal_json(self.vault_key, aad_key.as_str().as_bytes(), &checkpoint)?;
        let hash = cipher_hash(&sealed);
        let key = checkpoint_object_key(
            &checkpoint.vault_id,
            &checkpoint.device_id,
            checkpoint.sequence,
            &hash,
        )?;
        match self.remote.put_if_absent(&key, sealed.clone()).await? {
            SyncPutResult::Created { .. } => {}
            SyncPutResult::AlreadyExists { .. } => {
                let existing = self
                    .remote
                    .get(&key)
                    .await?
                    .ok_or_else(|| protocol_error("Existing checkpoint could not be read."))?;
                if existing.bytes != sealed {
                    return Err(protocol_error(
                        "Existing immutable checkpoint does not match the local checkpoint.",
                    ));
                }
            }
            SyncPutResult::Conflict { .. } => {
                return Err(protocol_error(
                    "Immutable checkpoint creation reported an ETag conflict.",
                ));
            }
        }
        self.local
            .mark_checkpoint_published(&SyncPublishedCheckpoint {
                sequence,
                cipher_hash: hash,
                encrypted_bytes: sealed.len() as u64,
                created_at_ms: now_ms,
            })?;
        result.checkpoint_published = true;
        Ok(())
    }

    async fn find_existing_checkpoint(
        &self,
        intended: &SyncCheckpointV1,
    ) -> Result<Option<(String, u64, u64)>, SyncError> {
        let prefix = SyncObjectPrefix::parse(format!(
            "sona-sync/v1/{}/devices/{}/checkpoints",
            intended.vault_id, intended.device_id
        ))?;
        let mut continuation = None;
        let mut candidates = Vec::new();
        loop {
            let page = self.remote.list(&prefix, continuation.as_deref()).await?;
            for metadata in page.objects {
                let Some(parsed) = parse_checkpoint_key(&metadata.key, &intended.vault_id)? else {
                    continue;
                };
                if parsed.device_id == intended.device_id && parsed.sequence == intended.sequence {
                    candidates.push((parsed, metadata.key));
                }
            }
            match page.continuation {
                Some(next) if !next.is_empty() => continuation = Some(next),
                _ => break,
            }
        }
        candidates.sort_by(|(left, _), (right, _)| left.cipher_hash.cmp(&right.cipher_hash));

        if let Some((parsed, key)) = candidates.into_iter().next() {
            let object =
                self.remote.get(&key).await?.ok_or_else(|| {
                    protocol_error(format!("Remote checkpoint disappeared: {key}."))
                })?;
            if cipher_hash(&object.bytes) != parsed.cipher_hash {
                return Err(protocol_error(format!(
                    "Remote checkpoint cipher hash does not match its key: {key}."
                )));
            }
            let aad_key = checkpoint_aad_key_parts(
                &intended.vault_id,
                &intended.device_id,
                intended.sequence,
            )?;
            let existing: SyncCheckpointV1 =
                open_json(self.vault_key, aad_key.as_str().as_bytes(), &object.bytes)?;
            existing.validate()?;
            if !same_checkpoint_payload(&existing, intended) {
                return Err(protocol_error(format!(
                    "Remote checkpoint sequence {} contains a different snapshot.",
                    intended.sequence
                )));
            }
            return Ok(Some((
                parsed.cipher_hash,
                object.bytes.len() as u64,
                existing.created_at_ms,
            )));
        }
        Ok(None)
    }

    async fn collect_garbage(
        &self,
        state: &SyncLocalRuntimeState,
        now_ms: u64,
    ) -> Result<(), SyncError> {
        let checkpoint_prefix = SyncObjectPrefix::parse(format!(
            "sona-sync/v1/{}/devices/{}/checkpoints",
            state.vault_id, state.device_id
        ))?;
        let mut continuation = None;
        let mut checkpoints = Vec::new();
        loop {
            let page = self
                .remote
                .list(&checkpoint_prefix, continuation.as_deref())
                .await?;
            for metadata in page.objects {
                let Some(parsed) = parse_checkpoint_key(&metadata.key, &state.vault_id)? else {
                    continue;
                };
                if parsed.device_id != state.device_id {
                    continue;
                }
                let object = self.remote.get(&metadata.key).await?.ok_or_else(|| {
                    protocol_error(format!(
                        "Local-device checkpoint disappeared: {}.",
                        metadata.key
                    ))
                })?;
                if cipher_hash(&object.bytes) != parsed.cipher_hash {
                    return Err(protocol_error(format!(
                        "Local-device checkpoint cipher hash does not match its key: {}.",
                        metadata.key
                    )));
                }
                let aad_key =
                    checkpoint_aad_key_parts(&state.vault_id, &state.device_id, parsed.sequence)?;
                let checkpoint: SyncCheckpointV1 =
                    open_json(self.vault_key, aad_key.as_str().as_bytes(), &object.bytes)?;
                checkpoint.validate()?;
                validate_remote_checkpoint(&checkpoint, &parsed, &state.vault_id)?;
                checkpoints.push(VerifiedCheckpointObject {
                    parsed,
                    key: metadata.key,
                    etag: object.metadata.etag,
                    checkpoint,
                });
            }
            match page.continuation {
                Some(next) if !next.is_empty() => continuation = Some(next),
                _ => break,
            }
        }
        checkpoints.sort_by(|left, right| {
            right
                .parsed
                .sequence
                .cmp(&left.parsed.sequence)
                .then_with(|| right.parsed.cipher_hash.cmp(&left.parsed.cipher_hash))
        });
        let Some(latest) = checkpoints.first() else {
            return Ok(());
        };
        let covered_sequence = latest.checkpoint.sequence;

        for obsolete in checkpoints.iter().skip(2) {
            delete_if_unchanged(self.remote, &obsolete.key, obsolete.etag.as_deref()).await?;
        }

        let segment_prefix = SyncObjectPrefix::parse(format!(
            "sona-sync/v1/{}/devices/{}/segments",
            state.vault_id, state.device_id
        ))?;
        continuation = None;
        loop {
            let page = self
                .remote
                .list(&segment_prefix, continuation.as_deref())
                .await?;
            for metadata in page.objects {
                let Some(parsed) = parse_segment_key(&metadata.key, &state.vault_id)? else {
                    continue;
                };
                if parsed.device_id != state.device_id || parsed.sequence > covered_sequence {
                    continue;
                }
                let object = self.remote.get(&metadata.key).await?.ok_or_else(|| {
                    protocol_error(format!(
                        "Local-device segment disappeared: {}.",
                        metadata.key
                    ))
                })?;
                if cipher_hash(&object.bytes) != parsed.cipher_hash {
                    return Err(protocol_error(format!(
                        "Local-device segment cipher hash does not match its key: {}.",
                        metadata.key
                    )));
                }
                let aad_key =
                    segment_aad_key_parts(&state.vault_id, &state.device_id, parsed.sequence)?;
                let segment: SyncSegmentV1 =
                    open_json(self.vault_key, aad_key.as_str().as_bytes(), &object.bytes)?;
                segment.validate()?;
                if segment.vault_id != state.vault_id
                    || segment.device_id != state.device_id
                    || segment.sequence != parsed.sequence
                {
                    return Err(protocol_error(
                        "Local-device segment metadata does not match its object key.",
                    ));
                }
                if now_ms.saturating_sub(segment.created_at_ms) > SEGMENT_RETENTION_MS
                    && now_ms >= segment.created_at_ms
                {
                    delete_if_unchanged(
                        self.remote,
                        &metadata.key,
                        object.metadata.etag.as_deref(),
                    )
                    .await?;
                }
            }
            match page.continuation {
                Some(next) if !next.is_empty() => continuation = Some(next),
                _ => break,
            }
        }
        Ok(())
    }
}

async fn delete_if_unchanged(
    remote: &dyn SyncObjectStore,
    key: &SyncObjectKey,
    etag: Option<&str>,
) -> Result<(), SyncError> {
    let Some(etag) = etag else {
        return Ok(());
    };
    match remote.delete(key, Some(etag)).await? {
        SyncDeleteResult::Deleted | SyncDeleteResult::NotFound => Ok(()),
        SyncDeleteResult::Conflict { .. } => Ok(()),
    }
}

#[derive(Clone, Debug)]
struct ParsedSegmentKey {
    device_id: String,
    sequence: u64,
    cipher_hash: String,
}

#[derive(Clone, Debug)]
struct ParsedCheckpointKey {
    device_id: String,
    sequence: u64,
    cipher_hash: String,
}

struct VerifiedCheckpointObject {
    parsed: ParsedCheckpointKey,
    key: SyncObjectKey,
    etag: Option<String>,
    checkpoint: SyncCheckpointV1,
}

fn parse_segment_key(
    key: &SyncObjectKey,
    expected_vault_id: &str,
) -> Result<Option<ParsedSegmentKey>, SyncError> {
    let parts = key.as_str().split('/').collect::<Vec<_>>();
    if parts.len() != 7
        || parts[0] != "sona-sync"
        || parts[1] != "v1"
        || parts[2] != expected_vault_id
        || parts[3] != "devices"
        || parts[5] != "segments"
    {
        return Ok(None);
    }
    let Some(file_name) = parts[6].strip_suffix(".sync") else {
        return Ok(None);
    };
    let Some((sequence, cipher_hash)) = file_name.split_once('-') else {
        return Err(protocol_error(format!(
            "Remote segment key is malformed: {key}."
        )));
    };
    let sequence = sequence
        .parse::<u64>()
        .map_err(|_| protocol_error(format!("Remote segment sequence is invalid: {key}.")))?;
    if cipher_hash.is_empty() {
        return Err(protocol_error(format!(
            "Remote segment hash is missing: {key}."
        )));
    }
    Ok(Some(ParsedSegmentKey {
        device_id: parts[4].to_string(),
        sequence,
        cipher_hash: cipher_hash.to_string(),
    }))
}

fn parse_checkpoint_key(
    key: &SyncObjectKey,
    expected_vault_id: &str,
) -> Result<Option<ParsedCheckpointKey>, SyncError> {
    let parts = key.as_str().split('/').collect::<Vec<_>>();
    if parts.len() != 7
        || parts[0] != "sona-sync"
        || parts[1] != "v1"
        || parts[2] != expected_vault_id
        || parts[3] != "devices"
        || parts[5] != "checkpoints"
    {
        return Ok(None);
    }
    let Some(file_name) = parts[6].strip_suffix(".sync") else {
        return Ok(None);
    };
    let Some((sequence, cipher_hash)) = file_name.split_once('-') else {
        return Err(protocol_error(format!(
            "Remote checkpoint key is malformed: {key}."
        )));
    };
    let sequence = sequence
        .parse::<u64>()
        .map_err(|_| protocol_error(format!("Remote checkpoint sequence is invalid: {key}.")))?;
    if cipher_hash.is_empty() {
        return Err(protocol_error(format!(
            "Remote checkpoint hash is missing: {key}."
        )));
    }
    Ok(Some(ParsedCheckpointKey {
        device_id: parts[4].to_string(),
        sequence,
        cipher_hash: cipher_hash.to_string(),
    }))
}

fn validate_remote_segment(
    segment: &SyncSegmentV1,
    parsed: &ParsedSegmentKey,
    cursor: &SyncDeviceCursor,
    expected_vault_id: &str,
) -> Result<(), SyncError> {
    if segment.vault_id != expected_vault_id
        || segment.device_id != parsed.device_id
        || segment.sequence != parsed.sequence
    {
        return Err(protocol_error(
            "Remote segment metadata does not match its object key.",
        ));
    }
    let expected_previous = if cursor.sequence == 0 {
        None
    } else {
        Some(cursor.cipher_hash.as_str())
    };
    if segment.previous_cipher_hash.as_deref() != expected_previous {
        return Err(protocol_error(
            "Remote segment previous hash does not match the local cursor.",
        ));
    }
    Ok(())
}

fn validate_remote_checkpoint(
    checkpoint: &SyncCheckpointV1,
    parsed: &ParsedCheckpointKey,
    expected_vault_id: &str,
) -> Result<(), SyncError> {
    if checkpoint.vault_id != expected_vault_id
        || checkpoint.device_id != parsed.device_id
        || checkpoint.sequence != parsed.sequence
    {
        return Err(protocol_error(
            "Remote checkpoint metadata does not match its object key.",
        ));
    }
    Ok(())
}

fn same_segment_payload(left: &SyncSegmentV1, right: &SyncSegmentV1) -> bool {
    left.protocol_version == right.protocol_version
        && left.vault_id == right.vault_id
        && left.device_id == right.device_id
        && left.sequence == right.sequence
        && left.previous_cipher_hash == right.previous_cipher_hash
        && left.operations == right.operations
}

fn same_checkpoint_payload(left: &SyncCheckpointV1, right: &SyncCheckpointV1) -> bool {
    left.protocol_version == right.protocol_version
        && left.vault_id == right.vault_id
        && left.device_id == right.device_id
        && left.sequence == right.sequence
        && left.covered_segment_cipher_hash == right.covered_segment_cipher_hash
        && left.causal_context == right.causal_context
        && left.operations == right.operations
}

fn validate_runtime_state(state: &SyncLocalRuntimeState) -> Result<(), SyncError> {
    if state.vault_id.trim().is_empty() || state.device_id.trim().is_empty() {
        return Err(protocol_error(
            "Sync runtime state requires vault and device IDs.",
        ));
    }
    if state.next_sequence == 0 {
        return Err(protocol_error(
            "Sync runtime next sequence must be greater than zero.",
        ));
    }
    Ok(())
}

fn segment_aad_key(segment: &SyncSegmentV1) -> Result<SyncObjectKey, SyncError> {
    segment_aad_key_parts(&segment.vault_id, &segment.device_id, segment.sequence)
}

fn segment_aad_key_parts(
    vault_id: &str,
    device_id: &str,
    sequence: u64,
) -> Result<SyncObjectKey, SyncError> {
    SyncObjectKey::parse(format!(
        "sona-sync/v1/{}/devices/{}/segments/{:020}",
        vault_id, device_id, sequence
    ))
}

fn checkpoint_aad_key(checkpoint: &SyncCheckpointV1) -> Result<SyncObjectKey, SyncError> {
    checkpoint_aad_key_parts(
        &checkpoint.vault_id,
        &checkpoint.device_id,
        checkpoint.sequence,
    )
}

fn checkpoint_aad_key_parts(
    vault_id: &str,
    device_id: &str,
    sequence: u64,
) -> Result<SyncObjectKey, SyncError> {
    SyncObjectKey::parse(format!(
        "sona-sync/v1/{}/devices/{}/checkpoints/{:020}",
        vault_id, device_id, sequence
    ))
}

fn cipher_hash(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn protocol_error(message: impl Into<String>) -> SyncError {
    SyncError::Protocol(message.into())
}
