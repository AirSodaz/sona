use std::collections::HashSet;

use crate::ports::time::UnixMillisClock;
use crate::recovery::RecoveryError;
use crate::recovery::normalization::{
    SourcePathStatus, SourcePathStatusProvider, recovered_item_from_queue_input_with_source_paths,
    recovered_item_from_saved_input_with_source_paths, snapshot_from_input_with_source_paths_at,
    snapshot_from_items_with_timestamp,
};
use crate::recovery::repository::RecoverySnapshotStore;
use crate::recovery::types::{RecoveryItemInput, RecoveryResolution, RecoverySnapshot};

pub struct RecoveryService<'a> {
    store: &'a dyn RecoverySnapshotStore,
    source_paths: &'a dyn SourcePathStatusProvider,
    clock: &'a dyn UnixMillisClock,
}

impl<'a> RecoveryService<'a> {
    pub fn new(
        store: &'a dyn RecoverySnapshotStore,
        source_paths: &'a dyn SourcePathStatusProvider,
        clock: &'a dyn UnixMillisClock,
    ) -> Self {
        Self {
            store,
            source_paths,
            clock,
        }
    }

    pub fn load_snapshot(&self) -> Result<RecoverySnapshot, RecoveryError> {
        self.load_snapshot_at(self.clock.now_ms()?)
    }

    pub fn load_snapshot_at(&self, now_ms: u64) -> Result<RecoverySnapshot, RecoveryError> {
        let input = self.store.load_snapshot_input()?;
        let source_paths = SourcePathStatusProviderRef(self.source_paths);
        Ok(snapshot_from_input_with_source_paths_at(
            input,
            false,
            &source_paths,
            now_ms,
        ))
    }

    pub fn save_snapshot_at(
        &self,
        items: Vec<RecoveryItemInput>,
        now_ms: u64,
    ) -> Result<RecoverySnapshot, RecoveryError> {
        let source_paths = SourcePathStatusProviderRef(self.source_paths);
        let items = items
            .into_iter()
            .filter_map(|input| {
                recovered_item_from_saved_input_with_source_paths(input, now_ms, &source_paths)
            })
            .filter(|item| item.resolution == RecoveryResolution::Pending)
            .collect::<Vec<_>>();
        let snapshot = snapshot_from_items_with_timestamp(items, now_ms);
        self.store.save_snapshot(&snapshot)?;
        Ok(snapshot)
    }

    pub fn save_snapshot(
        &self,
        items: Vec<RecoveryItemInput>,
    ) -> Result<RecoverySnapshot, RecoveryError> {
        self.save_snapshot_at(items, self.clock.now_ms()?)
    }

    pub fn persist_queue_snapshot_at(
        &self,
        queue_items: Vec<RecoveryItemInput>,
        resolved_ids: Vec<String>,
        now_ms: u64,
    ) -> Result<RecoverySnapshot, RecoveryError> {
        let mut observed_item_ids = resolved_ids
            .into_iter()
            .filter_map(|id| non_empty_string(&id))
            .collect::<HashSet<_>>();

        let source_paths = SourcePathStatusProviderRef(self.source_paths);
        let mut items = queue_items
            .into_iter()
            .filter_map(|input| {
                observed_item_ids.extend(collect_queue_recovery_ids(&input));
                recovered_item_from_queue_input_with_source_paths(input, now_ms, &source_paths)
            })
            .collect::<Vec<_>>();

        observed_item_ids.extend(items.iter().map(|item| item.id.clone()));
        items.extend(
            self.load_snapshot_at(now_ms)?
                .items
                .into_iter()
                .filter(|item| {
                    item.resolution == RecoveryResolution::Pending
                        && !observed_item_ids.contains(&item.id)
                }),
        );

        let snapshot = snapshot_from_items_with_timestamp(items, now_ms);
        self.store.save_snapshot(&snapshot)?;
        Ok(snapshot)
    }

    pub fn persist_queue_snapshot(
        &self,
        queue_items: Vec<RecoveryItemInput>,
        resolved_ids: Vec<String>,
    ) -> Result<RecoverySnapshot, RecoveryError> {
        self.persist_queue_snapshot_at(queue_items, resolved_ids, self.clock.now_ms()?)
    }
}

struct SourcePathStatusProviderRef<'a>(&'a dyn SourcePathStatusProvider);

impl SourcePathStatusProvider for SourcePathStatusProviderRef<'_> {
    fn status_for_path(&self, path: &str) -> SourcePathStatus {
        self.0.status_for_path(path)
    }
}

fn collect_queue_recovery_ids(input: &RecoveryItemInput) -> Vec<String> {
    [input.id.as_deref(), input.recovery_id.as_deref()]
        .into_iter()
        .flatten()
        .filter_map(non_empty_string)
        .collect()
}

fn non_empty_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}
