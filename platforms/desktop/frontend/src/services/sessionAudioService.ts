import { useBatchQueueStore } from '../stores/batchQueueStore';
import { useTranscriptSessionStore } from '../stores/transcriptSessionStore';
import { historyService } from './historyService';

export async function resolveCurrentSessionAudioPath(): Promise<string | null> {
  const session = useTranscriptSessionStore.getState();
  if (session.sourceHistoryId) {
    const path = await historyService.getAudioAbsolutePath(session.sourceHistoryId);
    if (path) {
      return path;
    }
  }

  const activeBatchItem = useBatchQueueStore
    .getState()
    .queueItems.find((item) => item.id === useBatchQueueStore.getState().activeItemId);
  if (activeBatchItem?.filePath) {
    return activeBatchItem.filePath;
  }

  return null;
}
