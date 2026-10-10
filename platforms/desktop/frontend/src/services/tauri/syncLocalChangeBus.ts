type SyncLocalChangeListener = () => void;

const listeners = new Set<SyncLocalChangeListener>();
const SYNC_RELEVANT_MUTATIONS: Record<string, true> = {
  save_app_config: true,
  set_app_setting: true,
  migrate_app_config: true,
  history_complete_live_draft: true,
  history_save_recording: true,
  history_save_imported_file: true,
  history_delete_items: true,
  history_trash_items: true,
  history_restore_items: true,
  history_purge_items: true,
  history_update_transcript: true,
  history_create_transcript_snapshot: true,
  history_restore_transcript_diff_rows: true,
  history_update_item_meta: true,
  history_update_project_assignments: true,
  history_reassign_project: true,
  history_save_summary: true,
  history_delete_summary: true,
  project_create: true,
  project_update: true,
  project_delete: true,
  project_reorder: true,
  automation_persist_rules: true,
  automation_persist_repository_state: true,
  sync_change_preset: true,
  sync_resolve_conflict: true,
};

export function notifySyncLocalChangeForCommand(command: string): void {
  if (SYNC_RELEVANT_MUTATIONS[command] !== true) {
    return;
  }
  listeners.forEach((listener) => {
    listener();
  });
}

export function subscribeToSyncLocalChanges(listener: SyncLocalChangeListener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}
