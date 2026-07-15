use serde_json::Value;
use std::collections::HashSet;

use crate::ports::time::UnixMillisClock;
use crate::recovery::normalization::{
    SourcePathStatus, SourcePathStatusProvider, recovered_item_from_queue_value_with_source_paths,
    recovered_item_from_saved_value_with_source_paths, snapshot_from_items_with_timestamp,
    snapshot_from_value_with_source_paths_at,
};
use crate::recovery::repository::RecoverySnapshotStore;
use crate::recovery::types::RecoverySnapshot;

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

    pub fn load_snapshot(&self) -> Result<RecoverySnapshot, String> {
        self.load_snapshot_at(self.clock.now_ms()?)
    }

    pub fn load_snapshot_at(&self, now_ms: u64) -> Result<RecoverySnapshot, String> {
        let value = self.store.load_snapshot_value()?;
        let source_paths = SourcePathStatusProviderRef(self.source_paths);
        Ok(snapshot_from_value_with_source_paths_at(
            value,
            false,
            &source_paths,
            now_ms,
        ))
    }

    pub fn save_snapshot_at(
        &self,
        items: Vec<Value>,
        now_ms: u64,
    ) -> Result<RecoverySnapshot, String> {
        let source_paths = SourcePathStatusProviderRef(self.source_paths);
        let items = items
            .into_iter()
            .filter_map(|value| {
                recovered_item_from_saved_value_with_source_paths(value, now_ms, &source_paths)
            })
            .filter(|item| item.resolution == "pending")
            .collect::<Vec<_>>();
        let snapshot = snapshot_from_items_with_timestamp(items, now_ms);
        self.store.save_snapshot(&snapshot)?;
        Ok(snapshot)
    }

    pub fn save_snapshot(&self, items: Vec<Value>) -> Result<RecoverySnapshot, String> {
        self.save_snapshot_at(items, self.clock.now_ms()?)
    }

    pub fn persist_queue_snapshot_at(
        &self,
        queue_items: Vec<Value>,
        resolved_ids: Vec<String>,
        now_ms: u64,
    ) -> Result<RecoverySnapshot, String> {
        let mut observed_item_ids = resolved_ids
            .into_iter()
            .filter_map(|id| non_empty_string(&id))
            .collect::<HashSet<_>>();

        let source_paths = SourcePathStatusProviderRef(self.source_paths);
        let mut items = queue_items
            .into_iter()
            .filter_map(|value| {
                observed_item_ids.extend(collect_queue_recovery_ids(&value));
                recovered_item_from_queue_value_with_source_paths(value, now_ms, &source_paths)
            })
            .collect::<Vec<_>>();

        observed_item_ids.extend(items.iter().map(|item| item.id.clone()));
        items.extend(
            self.load_snapshot_at(now_ms)?
                .items
                .into_iter()
                .filter(|item| {
                    item.resolution == "pending" && !observed_item_ids.contains(&item.id)
                }),
        );

        let snapshot = snapshot_from_items_with_timestamp(items, now_ms);
        self.store.save_snapshot(&snapshot)?;
        Ok(snapshot)
    }

    pub fn persist_queue_snapshot(
        &self,
        queue_items: Vec<Value>,
        resolved_ids: Vec<String>,
    ) -> Result<RecoverySnapshot, String> {
        self.persist_queue_snapshot_at(queue_items, resolved_ids, self.clock.now_ms()?)
    }
}

struct SourcePathStatusProviderRef<'a>(&'a dyn SourcePathStatusProvider);

impl SourcePathStatusProvider for SourcePathStatusProviderRef<'_> {
    fn status_for_path(&self, path: &str) -> SourcePathStatus {
        self.0.status_for_path(path)
    }
}

fn collect_queue_recovery_ids(value: &Value) -> Vec<String> {
    let Some(object) = value.as_object() else {
        return Vec::new();
    };
    ["id", "recoveryId"]
        .iter()
        .filter_map(|key| object.get(*key).and_then(Value::as_str))
        .filter_map(non_empty_string)
        .collect()
}

fn non_empty_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}
