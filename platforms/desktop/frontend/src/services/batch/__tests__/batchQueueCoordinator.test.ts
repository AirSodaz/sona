import { describe, expect, it, vi } from 'vitest';
import type { BatchQueueItem, BatchQueueItemStatus } from '../../../types/batchQueue';
import type { AppConfig } from '../../../types/config';
import {
  createBatchTaskLedgerId,
  isTaskLedgerCancelRequested,
  patchTaskLedgerRecord,
} from '../../taskLedgerBuilders';
import { createBatchQueueCoordinator, processNextBatchQueueItems } from '../batchQueueCoordinator';

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
      getQueueItems: () => [makeQueueItem('done', 'complete'), makeQueueItem('failed', 'error')],
      getMaxConcurrent: () => 2,
      setQueueProcessing,
      processItem,
    });

    expect(setQueueProcessing).toHaveBeenCalledWith(false);
    expect(processItem).not.toHaveBeenCalled();
  });

  it('allows upload retry when mediaServerContext is present and ASR is unconfigured', async () => {
    const mediaItem: BatchQueueItem = {
      ...makeQueueItem('ms-upload-retry', 'pending'),
      errorStage: 'upload',
      segments: [{ id: 'seg-1', start: 0, end: 1, text: 'Hello', isFinal: true }],
      mediaServerContext: {
        serverId: 'srv-1',
        serverType: 'jellyfin',
        serverUrl: 'http://localhost:8096',
        itemId: 'item-1',
        itemName: 'Movie',
        audioStreamIndex: 1,
        subtitleMode: 'bilingual',
        targetLanguage: 'zh-CN',
      },
    };
    const processBatchQueueItem = vi.fn().mockResolvedValue(undefined);
    const coordinator = createBatchQueueCoordinator({
      emitAutomationTaskSettled: vi.fn(),
      asrConfigService: {
        resolveAsrTranscriptionRequest: vi.fn().mockReturnValue({ engine: 'local', mode: 'batch' }),
        isAsrRequestConfigured: vi.fn().mockReturnValue(false),
      } as unknown as Parameters<typeof createBatchQueueCoordinator>[0]['asrConfigService'],
      batchItemProcessor: {
        processBatchQueueItem,
      } as unknown as Parameters<typeof createBatchQueueCoordinator>[0]['batchItemProcessor'],
      createBatchTaskLedgerId,
      isTaskLedgerCancelRequested,
      patchTaskLedgerRecord,
    });

    const updateItemStatus = vi.fn();
    const setItemError = vi.fn();
    await coordinator.processBatchQueueItemLifecycle(mediaItem.id, {
      getQueueItems: () => [mediaItem],
      getQueueItem: () => mediaItem,
      getFallbackConfigSnapshot: () => ({}) as AppConfig,
      updateItemStatus,
      updateItemSegments: vi.fn(),
      setItemError,
      applySavedHistory: vi.fn(),
      setItemExportPath: vi.fn(),
      setItemActiveInstanceId: vi.fn(),
      isActiveItem: () => true,
      scheduleNext: vi.fn(),
    });

    expect(setItemError).not.toHaveBeenCalled();
    expect(processBatchQueueItem).toHaveBeenCalledTimes(1);
    expect(updateItemStatus).toHaveBeenCalledWith(mediaItem.id, 'complete', 100);
  });
});
