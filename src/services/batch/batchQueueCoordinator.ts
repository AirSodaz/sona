import type { AppConfig } from '../../types/config';
import type {
  BatchQueueItem,
  BatchQueueItemStatus,
} from '../../types/batchQueue';
import type { HistoryItem } from '../../types/history';
import type { RecoveryItemStage } from '../../types/recovery';
import type { TaskLedgerStatus } from '../../types/taskLedger';
import type { TranscriptSegment } from '../../types/transcript';
import { emitAutomationTaskSettled } from '../automationRuntimeBridge';
import { isAsrRequestConfigured, resolveAsrTranscriptionRequest } from '../asrConfigService';
import { processBatchQueueItem } from './batchItemProcessor';
import {
  createBatchTaskLedgerId,
  isTaskLedgerCancelRequested,
  patchTaskLedgerRecord,
} from '../taskLedgerBuilders';
import { logger } from '../../utils/logger';
import { extractErrorMessage } from '../../utils/errorUtils';

interface BatchQueueSchedulerPorts {
  getQueueItems: () => BatchQueueItem[];
  getMaxConcurrent: () => number;
  setQueueProcessing: (isProcessing: boolean) => void;
  processItem: (itemId: string) => void | Promise<void>;
}

interface BatchQueueLifecyclePorts {
  getQueueItems: () => BatchQueueItem[];
  getQueueItem: (itemId: string) => BatchQueueItem | undefined;
  getFallbackConfigSnapshot: () => AppConfig;
  updateItemStatus: (
    id: string,
    status: BatchQueueItemStatus,
    progress?: number,
    lastKnownStage?: RecoveryItemStage,
  ) => void;
  updateItemSegments: (id: string, segments: TranscriptSegment[]) => void;
  setItemError: (id: string, message: string) => void;
  applySavedHistory: (itemId: string, item: BatchQueueItem, historyItem: HistoryItem) => void | Promise<void>;
  setItemExportPath: (itemId: string, exportPath: string) => void;
  isActiveItem: (itemId: string) => boolean;
  scheduleNext: () => void;
}

export function processNextBatchQueueItems({
  getQueueItems,
  getMaxConcurrent,
  setQueueProcessing,
  processItem,
}: BatchQueueSchedulerPorts): void {
  const queueItems = getQueueItems();
  const maxConcurrent = getMaxConcurrent();
  const processingCount = queueItems.filter((item) => item.status === 'processing').length;

  if (processingCount >= maxConcurrent) {
    return;
  }

  const pendingItems = queueItems.filter((item) => item.status === 'pending');
  const slotsAvailable = maxConcurrent - processingCount;
  const itemsToStart = pendingItems.slice(0, slotsAvailable);

  if (itemsToStart.length === 0 && processingCount === 0) {
    setQueueProcessing(false);
    return;
  }

  setQueueProcessing(true);
  itemsToStart.forEach((item) => {
    void processItem(item.id);
  });
}

function toTaskLedgerStatus(status: BatchQueueItemStatus): TaskLedgerStatus {
  switch (status) {
    case 'pending':
      return 'pending';
    case 'processing':
      return 'running';
    case 'complete':
      return 'succeeded';
    case 'cancelled':
      return 'cancelled';
    case 'error':
      return 'failed';
  }
}

function patchQueueItemTask(item: BatchQueueItem, patch: Parameters<typeof patchTaskLedgerRecord>[1]): void {
  patchTaskLedgerRecord(createBatchTaskLedgerId(item.id), patch);
}

function resolveQueueItemConfig(
  item: BatchQueueItem,
  getFallbackConfigSnapshot: () => AppConfig,
): AppConfig {
  if (item.resolvedConfigSnapshot) {
    return item.resolvedConfigSnapshot;
  }

  return getFallbackConfigSnapshot();
}

async function notifyAutomationResult(
  item: BatchQueueItem,
  status: 'complete' | 'error' | 'discarded',
  getQueueItems: () => BatchQueueItem[],
  errorMessage?: string,
): Promise<void> {
  if (
    item.origin !== 'automation'
    || !item.automationRuleId
    || !item.sourceFingerprint
    || !item.fileStat
  ) {
    return;
  }

  const latestItem = getQueueItems().find((queueItem) => queueItem.id === item.id) || item;
  await emitAutomationTaskSettled({
    ruleId: item.automationRuleId,
    filePath: item.filePath,
    sourceFingerprint: item.sourceFingerprint,
    size: item.fileStat.size,
    mtimeMs: item.fileStat.mtimeMs,
    status,
    processedAt: Date.now(),
    historyId: latestItem.historyId,
    exportPath: latestItem.exportPath,
    errorMessage,
    stage: latestItem.lastKnownStage,
  });
}

async function settleCancelledItem(
  item: BatchQueueItem,
  ports: BatchQueueLifecyclePorts,
): Promise<void> {
  ports.updateItemStatus(item.id, 'cancelled', 0);
  patchQueueItemTask(item, {
    status: 'cancelled',
    progress: 0,
    cancelable: false,
    retryable: false,
    errorMessage: undefined,
  });
  await notifyAutomationResult(item, 'discarded', ports.getQueueItems);
}

export async function processBatchQueueItemLifecycle(
  itemId: string,
  ports: BatchQueueLifecyclePorts,
): Promise<void> {
  const item = ports.getQueueItem(itemId);
  if (!item || item.status !== 'pending') {
    return;
  }

  const config = resolveQueueItemConfig(item, ports.getFallbackConfigSnapshot);

  if (isTaskLedgerCancelRequested(createBatchTaskLedgerId(itemId))) {
    await settleCancelledItem(item, ports);
    return;
  }

  if (!isAsrRequestConfigured(resolveAsrTranscriptionRequest(config, 'batch'))) {
    const message = 'Batch ASR is not configured.';
    ports.setItemError(itemId, message);
    await notifyAutomationResult(item, 'error', ports.getQueueItems, message);
    return;
  }

  try {
    await processBatchQueueItem({
      item,
      config,
      callbacks: {
        updateStatus: (status, progress, lastKnownStage) => {
          ports.updateItemStatus(itemId, status, progress, lastKnownStage);
        },
        updateSegments: (segments) => {
          ports.updateItemSegments(itemId, segments);
        },
        onHistorySaved: async (historyItem) => {
          await ports.applySavedHistory(itemId, item, historyItem);
        },
        onExportComplete: (exportPath) => {
          ports.setItemExportPath(itemId, exportPath);
        },
        isActiveItem: () => ports.isActiveItem(itemId),
        isCancelRequested: () => isTaskLedgerCancelRequested(createBatchTaskLedgerId(itemId)),
      },
    });

    if (isTaskLedgerCancelRequested(createBatchTaskLedgerId(itemId))) {
      await settleCancelledItem(item, ports);
    } else {
      ports.updateItemStatus(itemId, 'complete', 100);
      patchQueueItemTask(item, {
        status: 'succeeded',
        progress: 100,
        cancelable: false,
        retryable: false,
        errorMessage: undefined,
      });
      await notifyAutomationResult(item, 'complete', ports.getQueueItems);
    }
  } catch (error) {
    const message = extractErrorMessage(error);
    logger.error(`[BatchQueue] Failed to process ${item.filename}:`, error);

    if (message === 'Task cancelled.') {
      await settleCancelledItem(item, ports);
    } else {
      ports.setItemError(itemId, message);
      await notifyAutomationResult(item, 'error', ports.getQueueItems, message);
    }
  } finally {
    ports.scheduleNext();
  }
}

export { toTaskLedgerStatus };
