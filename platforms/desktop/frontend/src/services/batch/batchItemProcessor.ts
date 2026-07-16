import { v4 as uuidv4 } from 'uuid';
import type { AppConfig } from '../../types/config';
import type { AutomationStageConfig } from '../../types/automation';
import type { BatchQueueItem, BatchQueueItemStatus } from '../../types/batchQueue';
import type { HistoryItem } from '../../types/history';
import type { RecoveryItemStage } from '../../types/recovery';
import type { TranscriptSegment } from '../../types/transcript';
import { transcriptionService } from '../transcriptionService';
import { asrConfigService } from '../asrConfigService';
import { historyService } from '../historyService';
import { polishService } from '../polishService';
import { translationService } from '../translationService';
import { getFeatureLlmConfig, isLlmConfigComplete } from '../llm/configUtils';
import { summaryService } from '../summaryService';
import { exportService } from '../exportService';
import { useHistoryStore } from '../../stores/historyStore';
import { logger } from '../../utils/logger';
import { remove } from '../tauri/platform/fs';
import { join, tempDir } from '../tauri/platform/path';

export interface BatchItemProcessorCallbacks {
  updateStatus: (
    status: BatchQueueItemStatus,
    progress?: number,
    lastKnownStage?: RecoveryItemStage,
  ) => void;
  updateSegments: (segments: TranscriptSegment[]) => void;
  onHistorySaved: (historyItem: HistoryItem) => void | Promise<void>;
  onExportComplete: (exportPath: string) => void;
  isActiveItem: () => boolean;
  isCancelRequested: () => boolean;
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
}

export class BatchItemProcessor {
  constructor(private readonly ports: BatchItemProcessorPorts) {}

  processBatchQueueItem = async ({
    item,
    config,
    callbacks,
  }: ProcessBatchItemOptions): Promise<void> => {
    const language = config.language;
    const batchAsr = this.ports.asrConfigService.resolveAsrTranscriptionRequest(config, 'batch');
    const stageConfig = this.getAutomationStageConfig(item, config);

    if (!this.ports.asrConfigService.isAsrRequestConfigured(batchAsr)) {
      throw new Error('Batch ASR is not configured.');
    }

    callbacks.updateStatus('processing', 0, 'transcribing');

    let currentSegments: TranscriptSegment[] = [];
    let segmentBuffer: TranscriptSegment[] = [];
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

      const historyItem = await this.ports.historyService.saveImportedFile(
        item.filePath,
        currentSegments,
        this.calculateDuration(currentSegments),
        batchAsr.engine === 'local-sherpa' ? tempWavPath : undefined,
        item.tagIds ?? (item.projectId ? [item.projectId] : []),
        item.id,
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

    try {
      if (batchAsr.engine === 'local-sherpa') {
        this.ports.transcriptionService.setModelPath(batchAsr.modelPath);
      }
      this.ports.transcriptionService.setEnableITN(config.enableITN ?? false);

      const tempDirectory = await tempDir();
      tempWavPath = await join(tempDirectory, `${uuidv4()}.wav`);

      const segments = await this.ports.transcriptionService.transcribeFile(
        item.filePath,
        (progress) => {
          callbacks.updateStatus('processing', progress);
        },
        (segment) => {
          segmentBuffer.push(segment);
          const now = Date.now();

          if (segmentBuffer.length >= 50 || now - lastUpdateTime > 500) {
            setCurrentSegments([...currentSegments, ...segmentBuffer]);
            segmentBuffer = [];
            lastUpdateTime = now;
          }
        },
        language === 'auto' ? undefined : language,
        tempWavPath,
        config,
      );

      this.throwIfCancelRequested(callbacks);
      setCurrentSegments(segments);
      await ensureHistorySaved();
      await persistHistorySnapshot();

      if (stageConfig.autoPolish && currentSegments.length > 0) {
        this.throwIfCancelRequested(callbacks);
        const llm = getFeatureLlmConfig(config, 'polish');
        if (!isLlmConfigComplete(llm)) {
          throw new Error('Polish model is not configured.');
        }

        callbacks.updateStatus('processing', 96, 'polishing');
        await this.ports.polishService.polishSegmentsWithConfig(
          config,
          currentSegments,
          async (polishedChunk) => {
            const nextSegments = this.ports.polishService.applyPolishedSegmentsInMemory(currentSegments, polishedChunk);
            setCurrentSegments(nextSegments);
          },
        );
        await persistHistorySnapshot();
      }

      if (stageConfig.autoTranslate && currentSegments.length > 0) {
        this.throwIfCancelRequested(callbacks);
        const llm = getFeatureLlmConfig(config, 'translation');
        if (!isLlmConfigComplete(llm)) {
          throw new Error('Translation model is not configured.');
        }

        callbacks.updateStatus('processing', 98, 'translating');
        await this.ports.translationService.translateSegmentsWithConfig(
          config,
          currentSegments,
          async (translatedChunk) => {
            const nextSegments = this.ports.translationService.applyTranslationsInMemory(currentSegments, translatedChunk);
            setCurrentSegments(nextSegments);
          },
        );
        await persistHistorySnapshot();
      }

      if (item.exportConfig) {
        this.throwIfCancelRequested(callbacks);
        callbacks.updateStatus('processing', 99, 'exporting');
        const exportPath = await this.ports.exportTranscriptToDirectory({
          segments: currentSegments,
          directory: item.exportConfig.directory,
          baseFileName: this.buildAutomationExportBaseName(item),
          format: item.exportConfig.format,
          mode: item.exportConfig.mode,
        });
        callbacks.onExportComplete(exportPath);
      }

      if (savedHistoryId && callbacks.isActiveItem()) {
        await this.ports.summaryService.persistSummary(savedHistoryId);
      }
    } catch (error) {
      try {
        if (currentSegments.length > 0) {
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
  }

  private calculateDuration(segments: TranscriptSegment[]): number {
    return segments.length > 0 ? segments[segments.length - 1].end : 0;
  }

  private getAutomationStageConfig(item: BatchQueueItem, config: AppConfig): AutomationStageConfig {
    return item.stageConfig || {
      autoPolish: config.autoPolish ?? false,
      autoTranslate: false,
      exportEnabled: false,
    };
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
});

export const { processBatchQueueItem } = batchItemProcessor;
