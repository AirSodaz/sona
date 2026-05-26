import type { BatchQueueItem } from '../../types/batchQueue';
import type { HistoryItem } from '../../types/history';

export interface SavedBatchHistoryMeta {
  historyId: string;
  title: string;
  icon: string | null;
  projectId: string | null;
  audioUrl?: string | null;
}

interface ResolveSavedBatchHistoryMetaInput {
  historyItem: HistoryItem;
  fallbackAudioUrl?: string | null;
  fallbackProjectId?: string | null;
  getAudioUrl: (audioPath: string) => Promise<string | null | undefined>;
}

export async function resolveSavedBatchHistoryMeta({
  historyItem,
  fallbackAudioUrl,
  fallbackProjectId,
  getAudioUrl,
}: ResolveSavedBatchHistoryMetaInput): Promise<SavedBatchHistoryMeta> {
  let historyAudioUrl: string | null | undefined = null;

  if (historyItem.audioPath) {
    try {
      historyAudioUrl = await getAudioUrl(historyItem.audioPath);
    } catch {
      historyAudioUrl = null;
    }
  }

  return {
    historyId: historyItem.id,
    title: historyItem.title,
    icon: historyItem.icon || null,
    projectId: historyItem.projectId ?? fallbackProjectId ?? null,
    audioUrl: historyAudioUrl || fallbackAudioUrl || null,
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
      }
      : queueItem
  ));
}
