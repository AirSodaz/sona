type SyncLocalChangeListener = () => void;

const listeners = new Set<SyncLocalChangeListener>();
const SYNC_RELEVANT_MUTATIONS = new Set([
  'save_app_config',
  'set_app_setting',
  'migrate_app_config',
  'history_complete_live_draft',
  'history_save_recording',
  'history_save_imported_file',
  'history_delete_items',
  'history_update_transcript',
  'history_create_transcript_snapshot',
  'history_restore_transcript_diff_rows',
  'history_update_item_meta',
  'history_update_project_assignments',
  'history_reassign_project',
  'history_save_summary',
  'history_delete_summary',
  'project_save_all',
  'project_create',
  'project_update',
  'project_delete',
  'project_reorder',
  'automation_persist_rules',
  'automation_persist_repository_state',
  'sync_change_preset',
  'sync_resolve_conflict',
]);

export function notifySyncLocalChangeForCommand(command: string): void {
  if (!SYNC_RELEVANT_MUTATIONS.has(command)) {
    return;
  }
  listeners.forEach((listener) => listener());
}

export function subscribeToSyncLocalChanges(
  listener: SyncLocalChangeListener,
): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}
