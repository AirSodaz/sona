import type { BatchQueueItem } from '../../types/batchQueue';
import type { RecoveredQueueItem, RecoverySnapshot } from '../../types/recovery';
import { TauriCommand } from './commands';
import { invokeTauri } from './invoke';

export async function recoveryLoadSnapshot(): Promise<RecoverySnapshot> {
  return invokeTauri(TauriCommand.recovery.loadSnapshot);
}

export async function recoverySaveSnapshot(
  items: RecoveredQueueItem[],
): Promise<RecoverySnapshot> {
  return invokeTauri(TauriCommand.recovery.saveSnapshot, { items });
}

export async function recoveryPersistQueueSnapshot(
  queueItems: BatchQueueItem[],
): Promise<void> {
  await invokeTauri(TauriCommand.recovery.persistQueueSnapshot, { queueItems });
}
