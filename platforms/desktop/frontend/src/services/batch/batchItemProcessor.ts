import { v4 as uuidv4 } from 'uuid';
import { useHistoryStore } from '../../stores/historyStore';
import { useProjectStore } from '../../stores/projectStore';
import type { BatchQueueItem, BatchQueueItemStatus } from '../../types/batchQueue';
import type { AppConfig } from '../../types/config';
import type { HistoryItem } from '../../types/history';
import type { RecoveryItemStage } from '../../types/recovery';
import type { TranscriptSegment } from '../../types/transcript';
import { extractErrorMessage } from '../../utils/errorUtils';
import type { ExportFormat } from '../../utils/exportFormats';
import { logger } from '../../utils/logger';
import { asrConfigService, isLlamaCppBatchRequest } from '../asrConfigService';
import { exportService } from '../exportService';
import { historyService } from '../historyService';
import { pipelineExecutionEngine } from '../pipeline/pipelineExecutionEngine';
import { polishService } from '../polishService';
import { resolveItemPipeline } from '../projectPipeline';
import { summaryService } from '../summaryService';
import { remove } from '../tauri/platform/fs';
import { join, tempDir } from '../tauri/platform/path';
import { transcriptionService } from '../transcriptionService';
import { translationService } from '../translationService';

export interface BatchItemProcessorCallbacks {
  updateStatus: (
    status: BatchQueueItemStatus,
    progress?: number,
    lastKnownStage?: RecoveryItemStage
  ) => void;
  updateSegments: (segments: TranscriptSegment[]) => void;
  onHistorySaved: (historyItem: HistoryItem) => void | Promise<void>;
  onExportComplete: (exportPath: string) => void;
  isActiveItem: () => boolean;
  isCancelRequested: () => boolean;
  /** Called as soon as the batch `instanceId` is known, so the store can persist it for cancellation. */
  onInstanceIdAssigned: (instanceId: string) => void;
}

export interface ProcessBatchItemOptions {
  item: BatchQueueItem;
  config: AppConfig;
  callbacks: BatchItemProcessorCallbacks;
}

export interface BatchItemProcessorPorts {
  transcriptionService: typeof transcriptionService;
  historyService: typeof historyService;
  polishService: typeof polishService;
  translationService: typeof translationService;
  summaryService: typeof summaryService;
  exportTranscriptToDirectory: typeof exportService.exportTranscriptToDirectory;
  asrConfigService: typeof asrConfigService;
  useHistoryStore: typeof useHistoryStore;
  useProjectStore?: typeof useProjectStore;
  pipelineExecutionEngine?: typeof pipelineExecutionEngine;
}

export class BatchItemProcessor {
  constructor(private readonly ports: BatchItemProcessorPorts) {}

  processBatchQueueItem = async ({
    item,
    config,
    callbacks,
  }: ProcessBatchItemOptions): Promise<void> => {
    if (
      item.projectId &&
      !this.ports.useProjectStore?.getState?.().getProjectById(item.projectId)
    ) {
      item.projectId = null;
      item.pipelineSnapshot = undefined;
    }
    const language = config.language;
    const pipeline =
      item.pipelineSnapshot ??
      resolveItemPipeline(
        item.projectId,
        this.ports.useProjectStore?.getState?.().projects ?? [],
        config
      );
    const hotwordsOverride = this.ports.asrConfigService.buildHotwordsWithPipeline?.(
      config,
      pipeline,
      item.projectId
    );
    const batchAsr = this.ports.asrConfigService.resolveAsrTranscriptionRequest(
      config,
      'batch',
      hotwordsOverride ? { hotwords: hotwordsOverride } : {}
    );
    const isLlamaCpp = isLlamaCppBatchRequest(batchAsr);

    if (!this.ports.asrConfigService.isAsrRequestConfigured(batchAsr)) {
      throw new Error('Batch ASR is not configured.');
    }

    callbacks.updateStatus('processing', 0, 'transcribing');

    let currentSegments: TranscriptSegment[] = [];
    const segmentBuffer = new Map<string, TranscriptSegment>();
    let lastUpdateTime = 0;
    let tempWavPath: string | undefined;
    let savedHistoryId: string | null = item.historyId || null;

    const persistHistorySnapshot = async (): Promise<void> => {
      if (!savedHistoryId) {
        return;
      }

      await this.ports.useHistoryStore.getState().updateTranscript(savedHistoryId, currentSegments);
    };

    const ensureHistorySaved = async (): Promise<void> => {
      if (savedHistoryId || currentSegments.length === 0) {
        return;
      }

      const duration = this.calculateDuration(currentSegments);
      const convertedPath = batchAsr.engine === 'local' && !isLlamaCpp ? tempWavPath : undefined;
      const historyItem =
        typeof this.ports.historyService.saveImportedFileToProject === 'function'
          ? await this.ports.historyService.saveImportedFileToProject(
              item.filePath,
              currentSegments,
              duration,
              convertedPath,
              item.projectId,
              item.id
            )
          : await this.ports.historyService.saveImportedFile(
              item.filePath,
              currentSegments,
              duration,
              convertedPath,
              item.projectId,
              item.id
            );

      if (!historyItem) {
        return;
      }

      savedHistoryId = historyItem.id;
      this.ports.useHistoryStore.getState().addItem(historyItem);
      await callbacks.onHistorySaved(historyItem);
    };

    const setCurrentSegments = (segments: TranscriptSegment[]): void => {
      currentSegments = segments;
      callbacks.updateSegments(segments);
    };

    const flushSegmentBuffer = (): void => {
      if (segmentBuffer.size === 0) {
        return;
      }
      const merged = new Map(currentSegments.map((segment) => [segment.id, segment]));
      for (const segment of segmentBuffer.values()) {
        merged.set(segment.id, segment);
      }
      segmentBuffer.clear();
      setCurrentSegments([...merged.values()].sort((left, right) => left.start - right.start));
    };

    try {
      if (batchAsr.engine === 'local') {
        this.ports.transcriptionService.setModelPath(batchAsr.modelPath);
      }
      this.ports.transcriptionService.setEnableITN(config.enableITN ?? false);

      const tempDirectory = await tempDir();
      tempWavPath = isLlamaCpp ? undefined : await join(tempDirectory, `${uuidv4()}.wav`);

      this.throwIfCancelRequested(callbacks);

      const segments = await this.ports.transcriptionService.transcribeFile(
        item.filePath,
        (progress) => {
          callbacks.updateStatus('processing', progress);
        },
        (segment) => {
          segmentBuffer.set(segment.id, segment);
          const now = Date.now();

          if (segmentBuffer.size >= 50 || now - lastUpdateTime > 500) {
            flushSegmentBuffer();
            lastUpdateTime = now;
          }
        },
        language === 'auto' ? undefined : language,
        tempWavPath,
        config,
        (instanceId) => {
          callbacks.onInstanceIdAssigned(instanceId);
        },
        item.projectId
      );

      this.throwIfCancelRequested(callbacks);
      setCurrentSegments(segments);
      await ensureHistorySaved();
      await persistHistorySnapshot();

      const engine = this.ports.pipelineExecutionEngine ?? pipelineExecutionEngine;
      const pipeline =
        item.pipelineSnapshot ??
        resolveItemPipeline(
          item.projectId,
          this.ports.useProjectStore?.getState?.().projects ?? [],
          config
        );

      const pipelineResult = await engine.execute({
        historyId: savedHistoryId || '',
        segments: currentSegments,
        pipeline: {
          ...pipeline,
          autoExport: pipeline.autoExport || Boolean(item.exportConfig),
          exportDirectory: item.exportConfig?.directory || pipeline.exportDirectory,
          exportFormat:
            (item.exportConfig?.format as ExportFormat | undefined) || pipeline.exportFormat,
          exportFileNamePrefix: item.exportFileNamePrefix || pipeline.exportFileNamePrefix,
        },
        globalConfig: config,
        baseFileName: this.buildAutomationExportBaseName(item),
        onProgress: (stage, progress) => {
          callbacks.updateStatus(
            'processing',
            progress,
            stage === 'summarizing' ? undefined : stage
          );
        },
        onSegmentsUpdated: async (updatedSegments) => {
          setCurrentSegments(updatedSegments);
          await persistHistorySnapshot();
        },
        onExportComplete: (exportPath) => {
          callbacks.onExportComplete(exportPath);
        },
        isCancelRequested: () => callbacks.isCancelRequested(),
      });

      if (pipelineResult.segments) {
        currentSegments = pipelineResult.segments;
      }
      if (savedHistoryId && callbacks.isActiveItem()) {
        await this.ports.summaryService.persistSummary(savedHistoryId);
      }
    } catch (error) {
      const isCancelled =
        callbacks.isCancelRequested() || extractErrorMessage(error).includes('Task cancelled');
      try {
        if (!isCancelled && currentSegments.length > 0) {
          await ensureHistorySaved();
          await persistHistorySnapshot();
        }
      } catch (historyError) {
        logger.error('[BatchQueue] Failed to persist partial result after error:', historyError);
      }

      throw error;
    } finally {
      await this.removeTempFile(tempWavPath);
    }
  };

  private calculateDuration(segments: TranscriptSegment[]): number {
    return segments.length > 0 ? segments[segments.length - 1].end : 0;
  }

  private buildAutomationExportBaseName(item: BatchQueueItem): string {
    const baseName = item.filename.replace(/\.[^.]+$/, '');
    const prefix = (item.exportFileNamePrefix || '').trim();
    return prefix ? `${prefix} ${baseName}`.trim() : baseName;
  }

  private async removeTempFile(tempWavPath: string | undefined): Promise<void> {
    if (!tempWavPath) {
      return;
    }

    try {
      await remove(tempWavPath);
    } catch (error) {
      logger.warn('[BatchQueue] Failed to remove temp file:', error);
    }
  }

  private throwIfCancelRequested(callbacks: BatchItemProcessorCallbacks): void {
    if (callbacks.isCancelRequested()) {
      throw new Error('Task cancelled.');
    }
  }
}

export function createBatchItemProcessor(ports: BatchItemProcessorPorts): BatchItemProcessor {
  return new BatchItemProcessor(ports);
}

// Ensure exportService has exportTranscriptToDirectory, but it wasn't yet ported to Pattern D.
// I'll assume exportService exports that function directly. If exportService isn't Pattern D yet, I'll export it from there.
export const batchItemProcessor = createBatchItemProcessor({
  transcriptionService,
  historyService,
  polishService,
  translationService,
  summaryService,
  exportTranscriptToDirectory: exportService.exportTranscriptToDirectory,
  asrConfigService,
  useHistoryStore,
  useProjectStore,
  pipelineExecutionEngine,
});

export const { processBatchQueueItem } = batchItemProcessor;
