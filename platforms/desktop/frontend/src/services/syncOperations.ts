import { getPlatform, TauriCommand } from '../platform';
import type { SyncRunResult, SyncStatusSnapshot } from '../types/sync';

export async function getSyncStatus(): Promise<SyncStatusSnapshot> {
  return getPlatform().transport.invoke(TauriCommand.sync.getStatus);
}

export async function runSyncNow(): Promise<SyncRunResult> {
  return getPlatform().transport.invoke(TauriCommand.sync.runNow);
}
