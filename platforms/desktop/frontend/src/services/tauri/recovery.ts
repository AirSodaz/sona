import type { RecoveryItemInput_Deserialize, RecoverySnapshot_Serialize } from '../../bindings';
import { TauriCommand } from './commands';
import { invokeTauri } from './invoke';

function toRecoveryItemTransport(
  item: RecoveryItemInput_Deserialize
): RecoveryItemInput_Deserialize {
  return {
    ...item,
    segments: (item.segments ?? []).map((segment) => ({
      ...segment,
      tokens: segment.tokens ?? null,
      timestamps: segment.timestamps ?? null,
      durations: segment.durations ?? null,
      translation: segment.translation ?? null,
      speaker: segment.speaker ?? null,
      speakerAttribution: segment.speakerAttribution ?? null,
    })),
  };
}

export async function recoveryLoadSnapshot(): Promise<RecoverySnapshot_Serialize> {
  return invokeTauri(TauriCommand.recovery.loadSnapshot);
}

export async function recoverySaveSnapshot(
  items: RecoveryItemInput_Deserialize[]
): Promise<RecoverySnapshot_Serialize> {
  return invokeTauri(TauriCommand.recovery.saveSnapshot, {
    items: items.map(toRecoveryItemTransport),
  });
}

export async function recoveryPersistQueueSnapshot(
  queueItems: RecoveryItemInput_Deserialize[],
  resolvedIds?: string[]
): Promise<void> {
  await invokeTauri(TauriCommand.recovery.persistQueueSnapshot, {
    queueItems: queueItems.map(toRecoveryItemTransport),
    ...(resolvedIds && resolvedIds.length > 0 ? { resolvedIds } : {}),
  });
}
