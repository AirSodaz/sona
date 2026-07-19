import type {
  RecoveredTranscriptSegment_Deserialize,
  RecoveredTranscriptSegment_Serialize,
  RecoveryItemInput_Deserialize,
  RecoveryItemInput_Serialize,
  RecoverySnapshot_Serialize,
} from '../../bindings';
import { TauriCommand } from './commands';
import { invokeTauri } from './invoke';

function toRecoveredTranscriptSegmentTransport(
  segment: RecoveredTranscriptSegment_Serialize,
): RecoveredTranscriptSegment_Deserialize {
  return {
    ...segment,
    tokens: segment.tokens ?? null,
    timestamps: segment.timestamps ?? null,
    durations: segment.durations ?? null,
    translation: segment.translation ?? null,
    speaker: segment.speaker ?? null,
    speakerAttribution: segment.speakerAttribution ?? null,
  };
}

function toRecoveryItemTransport(
  item: RecoveryItemInput_Serialize,
): RecoveryItemInput_Deserialize {
  return {
    ...item,
    segments: item.segments.map(toRecoveredTranscriptSegmentTransport),
  };
}

export async function recoveryLoadSnapshot(): Promise<RecoverySnapshot_Serialize> {
  return invokeTauri(TauriCommand.recovery.loadSnapshot);
}

export async function recoverySaveSnapshot(
  items: RecoveryItemInput_Serialize[],
): Promise<RecoverySnapshot_Serialize> {
  return invokeTauri(TauriCommand.recovery.saveSnapshot, {
    items: items.map(toRecoveryItemTransport),
  });
}

export async function recoveryPersistQueueSnapshot(
  queueItems: RecoveryItemInput_Serialize[],
  resolvedIds?: string[],
): Promise<void> {
  await invokeTauri(TauriCommand.recovery.persistQueueSnapshot, {
    queueItems: queueItems.map(toRecoveryItemTransport),
    ...(resolvedIds && resolvedIds.length > 0 ? { resolvedIds } : {}),
  });
}
