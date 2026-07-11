import type { AppConfig } from '../../types/config';
import type {
  BatchQueueItem,
  BatchQueueItemStatus,
} from '../../types/batchQueue';
import type { HistoryItem } from '../../types/history';
import type { RecoveryItemStage } from '../../types/recovery';
import type { TaskLedgerStatus } from '../../types/taskLedger';
import type { TranscriptSegment } from '../../types/transcript';
import { emitAutomationTaskSettled } from '../automationEventBus';
import { asrConfigService } from '../asrConfigService';
import { batchItemProcessor } from './batchItemProcessor';
import {
  createBatchTaskLedgerId,
  isTaskLedgerCancelRequested,
  patchTaskLedgerRecord,
} from '../taskLedgerBuilders';
import { logger } from '../../utils/logger';
import { extractErrorMessage } from '../../utils/errorUtils';

export interface BatchQueueSchedulerPorts {
  getQueueItems: () => BatchQueueItem[];
  getMaxConcurrent: () => number;
  setQueueProcessing: (isProcessing: boolean) => void;
  processItem: (itemId: string) => void | Promise<void>;
}

export interface BatchQueueLifecyclePorts {
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

export interface BatchQueueCoordinatorPorts {
  emitAutomationTaskSettled: typeof emitAutomationTaskSettled;
  asrConfigService: typeof asrConfigService;
  batchItemProcessor: typeof batchItemProcessor;
  createBatchTaskLedgerId: typeof createBatchTaskLedgerId;
  isTaskLedgerCancelRequested: typeof isTaskLedgerCancelRequested;
  patchTaskLedgerRecord: typeof patchTaskLedgerRecord;
}

export class BatchQueueCoordinator {
  constructor(private readonly ports: BatchQueueCoordinatorPorts) {}

  processNextBatchQueueItems = ({
    getQueueItems,
    getMaxConcurrent,
    setQueueProcessing,
    processItem,
  }: BatchQueueSchedulerPorts): void => {
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

  toTaskLedgerStatus = (status: BatchQueueItemStatus): TaskLedgerStatus => {
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

  private patchQueueItemTask = (item: BatchQueueItem, patch: Parameters<typeof patchTaskLedgerRecord>[1]): void => {
    this.ports.patchTaskLedgerRecord(this.ports.createBatchTaskLedgerId(item.id), patch);
  }

  private resolveQueueItemConfig = (
    item: BatchQueueItem,
    getFallbackConfigSnapshot: () => AppConfig,
  ): AppConfig => {
    if (item.resolvedConfigSnapshot) {
      return item.resolvedConfigSnapshot;
    }

    return getFallbackConfigSnapshot();
  }

  private notifyAutomationResult = async (
    item: BatchQueueItem,
    status: 'complete' | 'error' | 'discarded',
    getQueueItems: () => BatchQueueItem[],
    errorMessage?: string,
  ): Promise<void> => {
    if (
      item.origin !== 'automation'
      || !item.automationRuleId
      || !item.sourceFingerprint
      || !item.fileStat
    ) {
      return;
    }

    const latestItem = getQueueItems().find((queueItem) => queueItem.id === item.id) || item;
    await this.ports.emitAutomationTaskSettled({
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

  private settleCancelledItem = async (
    item: BatchQueueItem,
    lifecyclePorts: BatchQueueLifecyclePorts,
  ): Promise<void> => {
    lifecyclePorts.updateItemStatus(item.id, 'cancelled', 0);
    this.patchQueueItemTask(item, {
      status: 'cancelled',
      progress: 0,
      cancelable: false,
      retryable: false,
      errorMessage: undefined,
    });
    await this.notifyAutomationResult(item, 'discarded', lifecyclePorts.getQueueItems);
  }

  processBatchQueueItemLifecycle = async (
    itemId: string,
    lifecyclePorts: BatchQueueLifecyclePorts,
  ): Promise<void> => {
    const item = lifecyclePorts.getQueueItem(itemId);
    if (!item || item.status !== 'pending') {
      return;
    }

    const config = this.resolveQueueItemConfig(item, lifecyclePorts.getFallbackConfigSnapshot);

    if (this.ports.isTaskLedgerCancelRequested(this.ports.createBatchTaskLedgerId(itemId))) {
      await this.settleCancelledItem(item, lifecyclePorts);
      return;
    }

    if (!this.ports.asrConfigService.isAsrRequestConfigured(
      this.ports.asrConfigService.resolveAsrTranscriptionRequest(config, 'batch')
    )) {
      const message = 'Batch ASR is not configured.';
      lifecyclePorts.setItemError(itemId, message);
      await this.notifyAutomationResult(item, 'error', lifecyclePorts.getQueueItems, message);
      return;
    }

    try {
      await this.ports.batchItemProcessor.processBatchQueueItem({
        item,
        config,
        callbacks: {
          updateStatus: (status, progress, lastKnownStage) => {
            lifecyclePorts.updateItemStatus(itemId, status, progress, lastKnownStage);
          },
          updateSegments: (segments) => {
            lifecyclePorts.updateItemSegments(itemId, segments);
          },
          onHistorySaved: async (historyItem) => {
            await lifecyclePorts.applySavedHistory(itemId, item, historyItem);
          },
          onExportComplete: (exportPath) => {
            lifecyclePorts.setItemExportPath(itemId, exportPath);
          },
          isActiveItem: () => lifecyclePorts.isActiveItem(itemId),
          isCancelRequested: () => this.ports.isTaskLedgerCancelRequested(this.ports.createBatchTaskLedgerId(itemId)),
        },
      });

      if (this.ports.isTaskLedgerCancelRequested(this.ports.createBatchTaskLedgerId(itemId))) {
        await this.settleCancelledItem(item, lifecyclePorts);
      } else {
        lifecyclePorts.updateItemStatus(itemId, 'complete', 100);
        this.patchQueueItemTask(item, {
          status: 'succeeded',
          progress: 100,
          cancelable: false,
          retryable: false,
          errorMessage: undefined,
        });
        await this.notifyAutomationResult(item, 'complete', lifecyclePorts.getQueueItems);
      }
    } catch (error) {
      const message = extractErrorMessage(error);
      logger.error(`[BatchQueue] Failed to process ${item.filename}:`, error);

      if (message === 'Task cancelled.') {
        await this.settleCancelledItem(item, lifecyclePorts);
      } else {
        lifecyclePorts.setItemError(itemId, message);
        await this.notifyAutomationResult(item, 'error', lifecyclePorts.getQueueItems, message);
      }
    } finally {
      lifecyclePorts.scheduleNext();
    }
  }
}

export function createBatchQueueCoordinator(ports: BatchQueueCoordinatorPorts): BatchQueueCoordinator {
  return new BatchQueueCoordinator(ports);
}

export const batchQueueCoordinator = createBatchQueueCoordinator({
  emitAutomationTaskSettled,
  asrConfigService,
  batchItemProcessor,
  createBatchTaskLedgerId,
  isTaskLedgerCancelRequested,
  patchTaskLedgerRecord,
});

export const {
  processNextBatchQueueItems,
  processBatchQueueItemLifecycle,
  toTaskLedgerStatus,
} = batchQueueCoordinator;
