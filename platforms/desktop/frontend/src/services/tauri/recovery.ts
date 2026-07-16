import type {
  RecoveryItemInput_Serialize,
  RecoverySnapshot_Serialize,
} from '../../bindings';
import { TauriCommand } from './commands';
import { invokeTauri } from './invoke';

export async function recoveryLoadSnapshot(): Promise<RecoverySnapshot_Serialize> {
  return invokeTauri(TauriCommand.recovery.loadSnapshot);
}

export async function recoverySaveSnapshot(
  items: RecoveryItemInput_Serialize[],
): Promise<RecoverySnapshot_Serialize> {
  return invokeTauri(TauriCommand.recovery.saveSnapshot, { items });
}

export async function recoveryPersistQueueSnapshot(
  queueItems: RecoveryItemInput_Serialize[],
  resolvedIds?: string[],
): Promise<void> {
  await invokeTauri(TauriCommand.recovery.persistQueueSnapshot, {
    queueItems,
    ...(resolvedIds && resolvedIds.length > 0 ? { resolvedIds } : {}),
  });
}
