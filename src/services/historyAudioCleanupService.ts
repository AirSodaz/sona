import type { HistoryAudioCleanupReport } from '../types/history';
import { logger } from '../utils/logger';
import { useConfigStore } from '../stores/configStore';
import { useHistoryStore } from '../stores/historyStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { historyService } from './historyService';

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
  now = new Date(),
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
      const excludeHistoryId = useTranscriptSessionStore.getState().sourceHistoryId || null;
      const report = await historyService.cleanupAudio(retentionDays, excludeHistoryId);

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
