import { v4 as uuidv4 } from 'uuid';
import { tempDir, join } from '@tauri-apps/api/path';
import { remove } from '@tauri-apps/plugin-fs';
import type { AppConfig } from '../../types/config';
import type { AutomationStageConfig } from '../../types/automation';
import type { BatchQueueItem, BatchQueueItemStatus } from '../../types/batchQueue';
import type { HistoryItem } from '../../types/history';
import type { RecoveryItemStage } from '../../types/recovery';
import type { TranscriptSegment } from '../../types/transcript';
import { transcriptionService } from '../transcriptionService';
import { historyService } from '../historyService';
import { polishService } from '../polishService';
import { translationService } from '../translationService';
import { getFeatureLlmConfig, isLlmConfigComplete } from '../llm/runtime';
import { summaryService } from '../summaryService';
import { exportTranscriptToDirectory } from '../exportService';
import { useHistoryStore } from '../../stores/historyStore';
import { logger } from '../../utils/logger';

interface BatchItemProcessorCallbacks {
    updateStatus: (
        status: BatchQueueItemStatus,
        progress?: number,
        lastKnownStage?: RecoveryItemStage,
    ) => void;
    updateSegments: (segments: TranscriptSegment[]) => void;
    onHistorySaved: (historyItem: HistoryItem) => void | Promise<void>;
    onExportComplete: (exportPath: string) => void;
    isActiveItem: () => boolean;
}

interface ProcessBatchItemOptions {
    item: BatchQueueItem;
    config: AppConfig;
    callbacks: BatchItemProcessorCallbacks;
}

function calculateDuration(segments: TranscriptSegment[]): number {
    return segments.length > 0 ? segments[segments.length - 1].end : 0;
}

function getAutomationStageConfig(item: BatchQueueItem, config: AppConfig): AutomationStageConfig {
    return item.stageConfig || {
        autoPolish: config.autoPolish ?? false,
        autoTranslate: false,
        exportEnabled: false,
    };
}

function buildAutomationExportBaseName(item: BatchQueueItem): string {
    const baseName = item.filename.replace(/\.[^.]+$/, '');
    const prefix = (item.exportFileNamePrefix || '').trim();
    return prefix ? `${prefix} ${baseName}`.trim() : baseName;
}

async function removeTempFile(tempWavPath: string | undefined): Promise<void> {
    if (!tempWavPath) {
        return;
    }

    try {
        await remove(tempWavPath);
    } catch (error) {
        logger.warn('[BatchQueue] Failed to remove temp file:', error);
    }
}

export async function processBatchQueueItem({
    item,
    config,
    callbacks,
}: ProcessBatchItemOptions): Promise<void> {
    const language = config.language;
    const offlineModelPath = config.offlineModelPath;
    const stageConfig = getAutomationStageConfig(item, config);

    if (!offlineModelPath) {
        throw new Error('No offline model path configured.');
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

        await useHistoryStore.getState().updateTranscript(savedHistoryId, currentSegments);
    };

    const ensureHistorySaved = async (): Promise<void> => {
        if (savedHistoryId || currentSegments.length === 0) {
            return;
        }

        const historyItem = await historyService.saveImportedFile(
            item.filePath,
            currentSegments,
            calculateDuration(currentSegments),
            tempWavPath,
            item.projectId,
        );

        if (!historyItem) {
            return;
        }

        savedHistoryId = historyItem.id;
        useHistoryStore.getState().addItem(historyItem);
        await callbacks.onHistorySaved(historyItem);
    };

    const setCurrentSegments = (segments: TranscriptSegment[]): void => {
        currentSegments = segments;
        callbacks.updateSegments(segments);
    };

    try {
        transcriptionService.setModelPath(offlineModelPath);
        transcriptionService.setEnableITN(config.enableITN ?? false);

        const tempDirectory = await tempDir();
        tempWavPath = await join(tempDirectory, `${uuidv4()}.wav`);

        const segments = await transcriptionService.transcribeFile(
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

        setCurrentSegments(segments);
        await ensureHistorySaved();
        await persistHistorySnapshot();

        if (stageConfig.autoPolish && currentSegments.length > 0) {
            const llm = getFeatureLlmConfig(config, 'polish');
            if (!isLlmConfigComplete(llm)) {
                throw new Error('Polish model is not configured.');
            }

            callbacks.updateStatus('processing', 96, 'polishing');
            await polishService.polishSegmentsWithConfig(
                config,
                currentSegments,
                async (polishedChunk) => {
                    const nextSegments = polishService.applyPolishedSegmentsInMemory(currentSegments, polishedChunk);
                    setCurrentSegments(nextSegments);
                },
            );
            await persistHistorySnapshot();
        }

        if (stageConfig.autoTranslate && currentSegments.length > 0) {
            const llm = getFeatureLlmConfig(config, 'translation');
            if (!isLlmConfigComplete(llm)) {
                throw new Error('Translation model is not configured.');
            }

            callbacks.updateStatus('processing', 98, 'translating');
            await translationService.translateSegmentsWithConfig(
                config,
                currentSegments,
                async (translatedChunk) => {
                    const nextSegments = translationService.applyTranslationsInMemory(currentSegments, translatedChunk);
                    setCurrentSegments(nextSegments);
                },
            );
            await persistHistorySnapshot();
        }

        if (item.exportConfig) {
            callbacks.updateStatus('processing', 99, 'exporting');
            const exportPath = await exportTranscriptToDirectory({
                segments: currentSegments,
                directory: item.exportConfig.directory,
                baseFileName: buildAutomationExportBaseName(item),
                format: item.exportConfig.format,
                mode: item.exportConfig.mode,
            });
            callbacks.onExportComplete(exportPath);
        }

        if (savedHistoryId && callbacks.isActiveItem()) {
            await summaryService.persistSummary(savedHistoryId);
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
        await removeTempFile(tempWavPath);
    }
}
