import { describe, expect, it, vi } from 'vitest';
import { processNextBatchQueueItems } from '../batchQueueCoordinator';
import type { BatchQueueItem, BatchQueueItemStatus } from '../../../types/batchQueue';

function makeQueueItem(id: string, status: BatchQueueItemStatus): BatchQueueItem {
  return {
    id,
    filename: `${id}.wav`,
    filePath: `/audio/${id}.wav`,
    status,
    progress: status === 'complete' ? 100 : 0,
    segments: [],
    audioUrl: `asset:///audio/${id}.wav`,
    projectId: null,
  };
}

describe('batchQueueCoordinator', () => {
  it('starts pending items only up to the available concurrency slots', async () => {
    const setQueueProcessing = vi.fn();
    const processItem = vi.fn();

    await processNextBatchQueueItems({
      getQueueItems: () => [
        makeQueueItem('running', 'processing'),
        makeQueueItem('pending-a', 'pending'),
        makeQueueItem('pending-b', 'pending'),
      ],
      getMaxConcurrent: () => 2,
      setQueueProcessing,
      processItem,
    });

    expect(setQueueProcessing).toHaveBeenCalledWith(true);
    expect(processItem).toHaveBeenCalledTimes(1);
    expect(processItem).toHaveBeenCalledWith('pending-a');
  });

  it('leaves the queue alone when all concurrency slots are occupied', async () => {
    const setQueueProcessing = vi.fn();
    const processItem = vi.fn();

    await processNextBatchQueueItems({
      getQueueItems: () => [
        makeQueueItem('running-a', 'processing'),
        makeQueueItem('running-b', 'processing'),
        makeQueueItem('pending-a', 'pending'),
      ],
      getMaxConcurrent: () => 2,
      setQueueProcessing,
      processItem,
    });

    expect(setQueueProcessing).not.toHaveBeenCalled();
    expect(processItem).not.toHaveBeenCalled();
  });

  it('marks the queue idle when no pending or processing items remain', async () => {
    const setQueueProcessing = vi.fn();
    const processItem = vi.fn();

    await processNextBatchQueueItems({
      getQueueItems: () => [
        makeQueueItem('done', 'complete'),
        makeQueueItem('failed', 'error'),
      ],
      getMaxConcurrent: () => 2,
      setQueueProcessing,
      processItem,
    });

    expect(setQueueProcessing).toHaveBeenCalledWith(false);
    expect(processItem).not.toHaveBeenCalled();
  });
});
