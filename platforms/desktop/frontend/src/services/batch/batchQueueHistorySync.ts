import type { BatchQueueItem } from '../../types/batchQueue';
import type { HistoryItem } from '../../types/history';
import { logger } from '../../utils/logger';

export interface SavedBatchHistoryMeta {
  historyId: string;
  title: string;
  icon: string | null;
  projectId: string | null;
  tagIds?: string[];
  audioUrl?: string | null;
}

interface ResolveSavedBatchHistoryMetaInput {
  historyItem: HistoryItem;
  fallbackProjectId?: string | null;
  getAudioUrl: (historyId: string) => Promise<string | null | undefined>;
}

export async function resolveSavedBatchHistoryMeta({
  historyItem,
  fallbackProjectId,
  getAudioUrl,
}: ResolveSavedBatchHistoryMetaInput): Promise<SavedBatchHistoryMeta> {
  let historyAudioUrl: string | null | undefined = null;

  if (historyItem.audioPath) {
    try {
      historyAudioUrl = await getAudioUrl(historyItem.id);
    } catch {
      logger.debug('[BatchQueue] Audio URL resolution failed, falling back to null');
      historyAudioUrl = null;
    }
  }

  return {
    historyId: historyItem.id,
    title: historyItem.title,
    icon: historyItem.icon || null,
    projectId: historyItem.projectId ?? fallbackProjectId ?? null,
    tagIds: historyItem.tagIds
      ?? (historyItem.projectId ? [historyItem.projectId] : fallbackProjectId ? [fallbackProjectId] : []),
    audioUrl: historyAudioUrl || null,
  };
}

export function applySavedBatchHistoryToQueue(
  queueItems: BatchQueueItem[],
  itemId: string,
  meta: SavedBatchHistoryMeta,
): BatchQueueItem[] {
  return queueItems.map((queueItem) => (
    queueItem.id === itemId
      ? {
        ...queueItem,
        historyId: meta.historyId,
        historyTitle: meta.title,
        audioUrl: meta.audioUrl ?? queueItem.audioUrl,
        projectId: meta.projectId ?? queueItem.projectId,
        tagIds: meta.tagIds ?? queueItem.tagIds ?? [],
      }
      : queueItem
  ));
}
