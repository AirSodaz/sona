import { useConfigStore } from '../stores/configStore';
import { useHistoryStore } from '../stores/historyStore';
import { useTranscriptStore } from '../stores/transcriptStore';
import type { HistoryAudioCleanupReport } from '../types/history';
import { logger } from '../utils/logger';
import { historyService } from './historyService';

export function collectOpenHistoryExclusionIds(): string[] {
  const { sessions, activeSessionId } = useTranscriptStore.getState();
  const ids = new Set<string>();
  for (const [sessionId, session] of Object.entries(sessions)) {
    if (session.sourceHistoryId) {
      ids.add(session.sourceHistoryId);
    } else if (sessionId && sessionId !== 'default') {
      // Only include IDs that exist in completed history items if session lacks sourceHistoryId
      const matching = useHistoryStore.getState().items.find((item) => item.id === sessionId);
      if (matching) {
        ids.add(matching.id);
      }
    }
  }
  const activeSourceId = sessions[activeSessionId]?.sourceHistoryId;
  if (activeSourceId) {
    ids.add(activeSourceId);
  }
  return Array.from(ids);
}

let cleanupInFlight: Promise<HistoryAudioCleanupReport | null> | null = null;
let lastCleanupDayKey: string | null = null;

function getLocalDayKey(date: Date): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  return `${year}-${month}-${day}`;
}

function hasStatusChanges(report: HistoryAudioCleanupReport): boolean {
  return report.removedCount > 0 || report.missingMarkedCount > 0;
}

export async function runHistoryAudioCleanupForCurrentConfig(
  now = new Date()
): Promise<HistoryAudioCleanupReport | null> {
  const retentionDays = useConfigStore.getState().config.historyAudioRetentionDays ?? null;
  if (retentionDays === null) {
    return null;
  }

  const dayKey = getLocalDayKey(now);
  if (cleanupInFlight) {
    return cleanupInFlight;
  }
  if (lastCleanupDayKey === dayKey) {
    return null;
  }

  cleanupInFlight = (async () => {
    lastCleanupDayKey = dayKey;

    try {
      const excludeHistoryIds = collectOpenHistoryExclusionIds();
      const report = await historyService.cleanupAudio(
        retentionDays,
        excludeHistoryIds[0] ?? null,
        excludeHistoryIds
      );

      logger.info('[HistoryAudioCleanup] Automatic cleanup finished', report);

      if (hasStatusChanges(report)) {
        await useHistoryStore.getState().refresh();
      }

      return report;
    } catch (error) {
      logger.error('[HistoryAudioCleanup] Automatic cleanup failed:', error);
      return null;
    } finally {
      cleanupInFlight = null;
    }
  })();

  return cleanupInFlight;
}

export function resetHistoryAudioCleanupServiceForTests(): void {
  cleanupInFlight = null;
  lastCleanupDayKey = null;
}
