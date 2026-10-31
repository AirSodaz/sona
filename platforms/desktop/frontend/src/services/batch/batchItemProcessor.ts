import { v4 as uuidv4 } from 'uuid';
import { useHistoryStore } from '../../stores/historyStore';
import { useMediaServerHistoryStore } from '../../stores/mediaServerHistoryStore';
import { useProjectStore } from '../../stores/projectStore';
import type { BatchQueueItem, BatchQueueItemStatus } from '../../types/batchQueue';
import type { AppConfig, TextReplacementRuleSet } from '../../types/config';
import type { HistoryItem } from '../../types/history';
import type { RecoveryItemStage } from '../../types/recovery';
import type { TranscriptSegment } from '../../types/transcript';
import { extractErrorMessage } from '../../utils/errorUtils';
import type { ExportFormat } from '../../utils/exportFormats';
import { logger } from '../../utils/logger';
import { getScenarioLanguage } from '../../utils/scenarioModels';
import { applyTextReplacements, extractReplacementsFromTerms } from '../../utils/textProcessing';
import { getTermsForProject, parseYamlDictionary } from '../../utils/yamlDictionaryParser';
import { asrConfigService, isLlamaCppBatchRequest } from '../asrConfigService';
import { exportService } from '../exportService';
import { historyService } from '../historyService';
import {
  downloadMediaServerAudio,
  syncMediaServerSubtitles,
} from '../mediaServer/mediaServerService';
import { pipelineExecutionEngine } from '../pipeline/pipelineExecutionEngine';
import { polishService } from '../polishService';
import { resolveItemPipeline } from '../projectPipeline';
import { summaryService } from '../summaryService';
import { remove } from '../tauri/platform/fs';
import { join, tempDir } from '../tauri/platform/path';
import { transcriptionService } from '../transcriptionService';
import { translationService } from '../translationService';
import { StreamingTranslationScheduler } from './streamingTranslationScheduler';

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
  updateFilePath?: (filePath: string) => void;
  /** Called as soon as the batch `instanceId` is known, so the store can persist it for cancellation. */
  onInstanceIdAssigned: (instanceId: string | null) => void;
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

function resolveEffectiveReplacementSets(
  config: AppConfig,
  pipeline: { replacementSetIds?: string[]; customTerms?: string[] },
  projectId?: string | null
): TextReplacementRuleSet[] {
  const effectiveReplacementSets: TextReplacementRuleSet[] = [];
  if (config.dictionaryContent?.trim()) {
    const parsed = parseYamlDictionary(config.dictionaryContent);
    const terms = getTermsForProject(parsed, projectId ?? undefined);
    if (terms.replacements.length > 0) {
      effectiveReplacementSets.push({
        id: 'unified-dictionary-replacements',
        name: 'Unified Dictionary',
        enabled: true,
        ignoreCase: false,
        rules: terms.replacements.map((r, i) => ({
          id: `rep_${i}`,
          from: r.from,
          to: r.to,
        })),
      });
    }
  } else {
    if (pipeline.replacementSetIds && pipeline.replacementSetIds.length > 0) {
      const globalSets = config.textReplacementSets ?? [];
      for (const setId of pipeline.replacementSetIds) {
        const found = globalSets.find((s) => s.id === setId);
        if (found) effectiveReplacementSets.push(found);
      }
    }
    const customReplacements = extractReplacementsFromTerms(pipeline.customTerms);
    if (customReplacements.length > 0) {
      effectiveReplacementSets.push({
        id: 'pipeline-custom-terms',
        name: 'Project Terms',
        enabled: true,
        ignoreCase: false,
        rules: customReplacements,
      });
    }
  }
  return effectiveReplacementSets;
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
    const language = getScenarioLanguage(config, 'batch');
    const pipeline =
      item.pipelineSnapshot ??
      resolveItemPipeline(
        item.projectId,
        this.ports.useProjectStore?.getState?.().projects ?? [],
        config
      );
    const mediaContext = item.mediaServerContext ?? item.embyContext;
    if (mediaContext && item.errorStage === 'upload' && item.segments && item.segments.length > 0) {
      this.throwIfCancelRequested(callbacks);
      callbacks.updateStatus('processing', 95);
      callbacks.updateSegments(item.segments);
      try {
        const targetLanguage =
          mediaContext.targetLanguage ||
          pipeline.targetLanguage ||
          config.translationLanguage ||
          'zh-CN';
        await syncMediaServerSubtitles(
          item.segments,
          { ...mediaContext, targetLanguage },
          { deleteExistingIndex: mediaContext.existingExternalSubtitleIndex }
        );
        item.errorStage = undefined;
        return;
      } catch (uploadError) {
        logger.error(
          '[BatchQueue] Fast-path retry failed to upload subtitle to media server:',
          uploadError
        );
        item.errorStage = 'upload';
        throw uploadError;
      }
    }

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
    let downloadedEmbyPath: string | undefined;
    let savedHistoryId: string | null = item.historyId || null;

    if (mediaContext && item.historyId) {
      useMediaServerHistoryStore.getState().setContext(item.historyId, mediaContext);
    }

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
      const isStream =
        !downloadedEmbyPath &&
        (item.sourceType === 'url_stream' ||
          /^(https?|rtmp|rtsp):\/\//i.test(item.streamUrl || item.filePath));
      const isEmbyItem = Boolean(mediaContext || downloadedEmbyPath);
      const convertedPath =
        batchAsr.engine === 'local' && !isLlamaCpp && (!isStream || item.retainAudio)
          ? tempWavPath
          : undefined;
      const inputSource = downloadedEmbyPath || item.streamUrl || item.filePath;
      const historyItem =
        typeof this.ports.historyService.saveImportedFileToProject === 'function'
          ? await this.ports.historyService.saveImportedFileToProject(
              inputSource,
              currentSegments,
              duration,
              convertedPath,
              item.projectId,
              item.id
            )
          : await this.ports.historyService.saveImportedFile(
              inputSource,
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
      if ((isStream || isEmbyItem) && item.filename && item.filename !== historyItem.title) {
        historyItem.title = item.filename;
        if (typeof this.ports.useHistoryStore?.getState?.().updateItemMeta === 'function') {
          try {
            await this.ports.useHistoryStore.getState().updateItemMeta(historyItem.id, {
              title: item.filename,
            });
          } catch (metaErr) {
            logger.warn('[BatchQueue] Failed to update stream history title meta:', metaErr);
          }
        }
      }

      if (mediaContext && savedHistoryId) {
        useMediaServerHistoryStore.getState().setContext(savedHistoryId, mediaContext);
      }

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

    const resolvedTargetLanguage =
      mediaContext?.targetLanguage ||
      pipeline.targetLanguage ||
      config.translationLanguage ||
      'zh-CN';
    const shouldStreamTranslate =
      (pipeline.autoTranslate || Boolean(mediaContext)) && !pipeline.autoPolish;

    const effectiveReplacementSets = resolveEffectiveReplacementSets(
      config,
      pipeline,
      item.projectId
    );

    let scheduler: StreamingTranslationScheduler | null = null;
    if (shouldStreamTranslate) {
      scheduler = new StreamingTranslationScheduler({
        config,
        targetLanguage: resolvedTargetLanguage,
        onBatchTranslated: async (translatedChunk) => {
          const transMap = new Map(translatedChunk.map((c) => [c.id, c.translation]));
          for (const [id, translation] of transMap.entries()) {
            const buffered = segmentBuffer.get(id);
            if (buffered) {
              segmentBuffer.set(id, { ...buffered, translation });
            }
          }
          currentSegments = currentSegments.map((seg) => {
            const translation = transMap.get(seg.id);
            return translation ? { ...seg, translation } : seg;
          });
          callbacks.updateSegments(currentSegments);
          if (savedHistoryId) {
            await persistHistorySnapshot();
          }
        },
        onError: (err) => {
          logger.warn('[BatchQueue] Streaming translation batch error:', err);
        },
        isCancelRequested: () => callbacks.isCancelRequested(),
        translationService: this.ports.translationService,
      });
    }

    item.errorStage = undefined;
    try {
      if (batchAsr.engine === 'local') {
        this.ports.transcriptionService.setModelPath(batchAsr.modelPath);
      }
      this.ports.transcriptionService.setEnableITN(config.enableITN ?? false);

      if (mediaContext && !item.streamUrl && !item.filePath) {
        callbacks.updateStatus('processing', 0);
        const downloadedPath = await downloadMediaServerAudio(
          mediaContext.itemId,
          mediaContext.audioStreamIndex,
          mediaContext.serverId
        );
        downloadedEmbyPath = downloadedPath;
        item.filePath = downloadedPath;
        callbacks.updateFilePath?.(downloadedPath);
      }

      const isStream =
        !downloadedEmbyPath &&
        (item.sourceType === 'url_stream' ||
          /^(https?|rtmp|rtsp):\/\//i.test(item.streamUrl || item.filePath));
      const shouldCreateTempWav =
        !isLlamaCpp && batchAsr.engine !== 'online' && (!isStream || Boolean(item.retainAudio));
      tempWavPath = shouldCreateTempWav
        ? await join(await tempDir(), `${uuidv4()}.wav`)
        : undefined;
      this.throwIfCancelRequested(callbacks);

      const inputSource = downloadedEmbyPath || item.streamUrl || item.filePath;
      let segments: TranscriptSegment[];
      try {
        segments = await this.ports.transcriptionService.transcribeFile(
          inputSource,
          (progress) => {
            callbacks.updateStatus('processing', progress);
          },
          (segment) => {
            const cleanText =
              effectiveReplacementSets.length > 0
                ? applyTextReplacements(segment.text, effectiveReplacementSets)
                : segment.text;
            const cleanSegment =
              cleanText === segment.text ? segment : { ...segment, text: cleanText };

            if (cleanSegment.isFinal !== false) {
              scheduler?.push(cleanSegment);
            }
            segmentBuffer.set(cleanSegment.id, cleanSegment);
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
      } catch (transcribeError) {
        flushSegmentBuffer();
        const isStream =
          !downloadedEmbyPath &&
          (item.sourceType === 'url_stream' ||
            /^(https?|rtmp|rtsp):\/\//i.test(item.streamUrl || item.filePath));
        const isExplicitCancel = callbacks.isCancelRequested();
        const isTaskCancelError = extractErrorMessage(transcribeError).includes('Task cancelled');
        if (isStream && !isExplicitCancel && isTaskCancelError && currentSegments.length > 0) {
          segments = currentSegments;
        } else {
          scheduler?.cancel();
          throw transcribeError;
        }
      } finally {
        callbacks.onInstanceIdAssigned(null);
      }

      flushSegmentBuffer();
      this.throwIfCancelRequested(callbacks);

      const cleanedSegments =
        effectiveReplacementSets.length > 0
          ? segments.map((seg) => {
              const cleanText = applyTextReplacements(seg.text, effectiveReplacementSets);
              return cleanText === seg.text ? seg : { ...seg, text: cleanText };
            })
          : segments;

      if (scheduler) {
        callbacks.updateStatus('processing', 95, 'translating');
        await scheduler.drain();
        const translationMap = scheduler.getTranslations();
        const reconciled = cleanedSegments.map((seg) => ({
          ...seg,
          translation: translationMap.get(seg.id) ?? seg.translation,
        }));
        setCurrentSegments(reconciled);
      } else {
        setCurrentSegments(cleanedSegments);
      }
      await ensureHistorySaved();
      await persistHistorySnapshot();

      const engine = this.ports.pipelineExecutionEngine ?? pipelineExecutionEngine;

      const pipelineResult = await engine.execute({
        historyId: savedHistoryId || '',
        projectId: item.projectId,
        segments: currentSegments,
        pipeline: {
          ...pipeline,
          replacementSetIds: [],
          customTerms: [],
          autoExport: pipeline.autoExport || Boolean(item.exportConfig),
          autoTranslate: Boolean(mediaContext) || pipeline.autoTranslate,
          targetLanguage: resolvedTargetLanguage,
          exportDirectory: item.exportConfig?.directory || pipeline.exportDirectory,
          exportFormat:
            (item.exportConfig?.format as ExportFormat | undefined) || pipeline.exportFormat,
          exportFileNamePrefix: item.exportFileNamePrefix || pipeline.exportFileNamePrefix,
        },
        globalConfig: { ...config, dictionaryContent: undefined },
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
      this.throwIfCancelRequested(callbacks);
      if (mediaContext && currentSegments.length > 0) {
        try {
          const targetLanguage =
            mediaContext.targetLanguage ||
            pipeline.targetLanguage ||
            config.translationLanguage ||
            'zh-CN';
          await syncMediaServerSubtitles(
            currentSegments,
            { ...mediaContext, targetLanguage },
            { deleteExistingIndex: mediaContext.existingExternalSubtitleIndex }
          );
        } catch (uploadError) {
          logger.error('[BatchQueue] Failed to upload subtitle to media server:', uploadError);
          item.errorStage = 'upload';
          item.segments = currentSegments;
          callbacks.updateSegments(currentSegments);
          throw uploadError;
        }
      }
    } catch (error) {
      scheduler?.cancel();
      flushSegmentBuffer();
      const isStream =
        !downloadedEmbyPath &&
        (item.sourceType === 'url_stream' ||
          /^(https?|rtmp|rtsp):\/\//i.test(item.streamUrl || item.filePath));
      const isExplicitCancel = callbacks.isCancelRequested();
      const isTaskCancelError = extractErrorMessage(error).includes('Task cancelled');
      const isCancelled = isExplicitCancel || isTaskCancelError;
      try {
        if ((!isCancelled || isStream) && currentSegments.length > 0) {
          await ensureHistorySaved();
          await persistHistorySnapshot();
        }
      } catch (historyError) {
        logger.error('[BatchQueue] Failed to persist partial result after error:', historyError);
      }
      if (downloadedEmbyPath) {
        item.filePath = '';
        callbacks.updateFilePath?.('');
      }
      if (currentSegments.length > 0) {
        item.segments = currentSegments;
        callbacks.updateSegments(currentSegments);
      }
      if (!item.errorStage) {
        item.errorStage = 'transcribe';
      }

      throw error;
    } finally {
      if (downloadedEmbyPath) {
        await this.removeTempFile(downloadedEmbyPath);
      }
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
