import { create } from 'zustand';
import { v4 as uuidv4 } from 'uuid';
import { convertFileSrc } from '@tauri-apps/api/core';
import { tempDir, join } from '@tauri-apps/api/path';
import { remove } from '@tauri-apps/plugin-fs';
import type { AppConfig } from '../types/config';
import type { AutomationExportConfig, AutomationStageConfig } from '../types/automation';
import {
  BatchQueueItem,
  BatchQueueItemOrigin,
  BatchQueueItemStatus,
} from '../types/batchQueue';
import { TranscriptSegment } from '../types/transcript';
import { transcriptionService } from '../services/transcriptionService';
import { historyService } from '../services/historyService';
import { polishService } from '../services/polishService';
import { translationService } from '../services/translationService';
import { getFeatureLlmConfig, isLlmConfigComplete } from '../services/llm/runtime';
import { summaryService } from '../services/summaryService';
import { exportTranscriptToDirectory } from '../services/exportService';
import { resolveEffectiveConfig } from '../services/effectiveConfigService';
import { emitAutomationTaskSettled } from '../services/automationRuntimeBridge';
import { persistQueueRecoverySnapshot, toBatchQueueItem } from '../services/recoveryService';
import { useTranscriptStore } from './transcriptStore';
import { useConfigStore } from './configStore';
import { useProjectStore } from './projectStore';
import { useHistoryStore } from './historyStore';
import { logger } from '../utils/logger';
import type { RecoveredQueueItem } from '../types/recovery';

interface AddFilesOptions {
    origin?: BatchQueueItemOrigin;
    automationRuleId?: string;
    automationRuleName?: string;
    resolvedConfigSnapshot?: AppConfig;
    exportConfig?: AutomationExportConfig | null;
    stageConfig?: AutomationStageConfig | null;
    sourceFingerprint?: string;
    projectId?: string | null;
    fileStat?: {
        size: number;
        mtimeMs: number;
    };
    exportFileNamePrefix?: string;
}

/** State interface for the batch queue store. */
interface BatchQueueState {
    /** List of queued files. */
    queueItems: BatchQueueItem[];
    /** ID of the currently active/selected item. */
    activeItemId: string | null;
    /** Whether the queue is currently processing. */
    isQueueProcessing: boolean;
    /**
     * Adds files to the queue.
     *
     * @param filePaths Array of file paths to add.
     * @param options Optional queue metadata.
     */
    addFiles: (filePaths: string[], options?: AddFilesOptions) => void;
    /**
     * Re-enqueues items restored by the recovery center.
     *
     * @param items Recovery items to resume.
     */
    enqueueRecoveredItems: (items: RecoveredQueueItem[]) => void;
    /**
     * Starts processing the queue sequentially.
     */
    processQueue: () => Promise<void>;
    /**
     * Sets the active/selected item.
     *
     * @param id Item ID to set as active.
     */
    setActiveItem: (id: string | null) => void;
    /**
     * Updates an item's status and progress.
     *
     * @param id Item ID.
     * @param status New status.
     * @param progress New progress value.
     * @param lastKnownStage Recovery stage metadata.
     */
    updateItemStatus: (id: string, status: BatchQueueItemStatus, progress?: number, lastKnownStage?: RecoveredQueueItem['lastKnownStage']) => void;
    /**
     * Updates an item's segments.
     *
     * @param id Item ID.
     * @param segments New segments array.
     */
    updateItemSegments: (id: string, segments: TranscriptSegment[]) => void;
    /**
     * Sets error state for an item.
     *
     * @param id Item ID.
     * @param message Error message.
     */
    setItemError: (id: string, message: string) => void;
    /**
     * Removes an item from the queue.
     *
     * @param id Item ID to remove.
     */
    removeItem: (id: string) => void;
    /** Clears all items from the queue. */
    clearQueue: () => void;
    /** internal helper */
    _processItem: (itemId: string) => Promise<void>;
}

function scheduleRecoverySnapshotSync(queueItems: BatchQueueItem[], immediate = false) {
    persistQueueRecoverySnapshot(queueItems, { immediate });
}

function resolveQueueItemConfig(item: BatchQueueItem): AppConfig {
    const projectStore = useProjectStore.getState();
    if (item.resolvedConfigSnapshot) {
        return item.resolvedConfigSnapshot;
    }

    const project = item.projectId && typeof projectStore.getProjectById === 'function'
        ? projectStore.getProjectById(item.projectId)
        : null;
    return resolveEffectiveConfig(useConfigStore.getState().config, project);
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

async function notifyAutomationResult(
    item: BatchQueueItem,
    status: 'complete' | 'error',
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

    const latestItem = useBatchQueueStore.getState().queueItems.find((queueItem) => queueItem.id === item.id) || item;
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

/**
 * Zustand store for managing batch transcription queue.
 */
export const useBatchQueueStore = create<BatchQueueState>((set, get) => ({
    queueItems: [],
    activeItemId: null,
    isQueueProcessing: false,

    addFiles: (filePaths, options) => {
        const projectStore = useProjectStore.getState();
        const activeProjectId = options?.projectId ?? projectStore.activeProjectId;
        const activeProject = activeProjectId
            ? (typeof projectStore.getProjectById === 'function' ? projectStore.getProjectById(activeProjectId) : null)
            : (typeof projectStore.getActiveProject === 'function' ? projectStore.getActiveProject() : null);
        const resolvedConfigSnapshot = options?.resolvedConfigSnapshot
            ?? resolveEffectiveConfig(useConfigStore.getState().config, activeProject);
        const exportFileNamePrefix = options?.exportFileNamePrefix
            ?? activeProject?.defaults.exportFileNamePrefix
            ?? '';

        const newItems: BatchQueueItem[] = filePaths.map((filePath) => {
            const filename = filePath.split(/[/\\]/).pop() || filePath;
            return {
                id: uuidv4(),
                filename,
                filePath,
                status: 'pending',
                progress: 0,
                segments: [],
                audioUrl: convertFileSrc(filePath),
                projectId: activeProjectId,
                origin: options?.origin || 'manual',
                automationRuleId: options?.automationRuleId,
                automationRuleName: options?.automationRuleName,
                resolvedConfigSnapshot,
                exportConfig: options?.exportConfig || null,
                stageConfig: options?.stageConfig || null,
                sourceFingerprint: options?.sourceFingerprint,
                fileStat: options?.fileStat,
                exportFileNamePrefix,
                lastKnownStage: 'queued',
            };
        });

        let nextQueueItems: BatchQueueItem[] = [];
        set((state) => {
            nextQueueItems = [...state.queueItems, ...newItems];
            return {
                queueItems: nextQueueItems
            };
        });
        scheduleRecoverySnapshotSync(nextQueueItems, true);

        const state = get();
        if (!state.activeItemId && newItems.length > 0) {
            get().setActiveItem(newItems[0].id);
        }

        if (!state.isQueueProcessing) {
            void get().processQueue();
        }
    },

    enqueueRecoveredItems: (items) => {
        if (items.length === 0) {
            return;
        }

        const recoveredQueueItems = items.map((item) => toBatchQueueItem(item));
        let nextQueueItems: BatchQueueItem[] = [];
        set((state) => {
            nextQueueItems = [...state.queueItems, ...recoveredQueueItems];
            return {
                queueItems: nextQueueItems,
            };
        });
        scheduleRecoverySnapshotSync(nextQueueItems, true);

        const state = get();
        if (!state.activeItemId && recoveredQueueItems.length > 0) {
            get().setActiveItem(recoveredQueueItems[0].id);
        }

        if (!state.isQueueProcessing) {
            void get().processQueue();
        }
    },

    processQueue: async () => {
        const state = get();
        const maxConcurrent = useConfigStore.getState().config.maxConcurrent || 2;
        const processingCount = state.queueItems.filter((item) => item.status === 'processing').length;

        if (processingCount >= maxConcurrent) {
            return;
        }

        const pendingItems = state.queueItems.filter((item) => item.status === 'pending');
        const slotsAvailable = maxConcurrent - processingCount;
        const itemsToStart = pendingItems.slice(0, slotsAvailable);

        if (itemsToStart.length === 0 && processingCount === 0) {
            set({ isQueueProcessing: false });
            return;
        }

        set({ isQueueProcessing: true });
        itemsToStart.forEach((item) => {
            void get()._processItem(item.id);
        });
    },

    _processItem: async (itemId: string) => {
        const state = get();
        const item = state.queueItems.find((queueItem) => queueItem.id === itemId);
        if (!item || item.status !== 'pending') {
            return;
        }

        const config = resolveQueueItemConfig(item);
        const language = config.language;
        const stageConfig = getAutomationStageConfig(item, config);

        if (!config.offlineModelPath) {
            const message = 'No offline model path configured.';
            get().setItemError(itemId, message);
            await notifyAutomationResult(item, 'error', message);
            return;
        }

        get().updateItemStatus(itemId, 'processing', 0, 'transcribing');

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
            let nextQueueItems: BatchQueueItem[] = [];
            set((currentState) => {
                nextQueueItems = currentState.queueItems.map((queueItem) => (
                    queueItem.id === itemId
                        ? {
                            ...queueItem,
                            historyId: historyItem.id,
                            historyTitle: historyItem.title,
                            projectId: historyItem.projectId ?? queueItem.projectId,
                        }
                        : queueItem
                ));
                return {
                    queueItems: nextQueueItems,
                };
            });
            scheduleRecoverySnapshotSync(nextQueueItems, true);

            if (get().activeItemId === itemId) {
                const transcriptStore = useTranscriptStore.getState();
                transcriptStore.setSourceHistoryId(historyItem.id);
                transcriptStore.setTitle(historyItem.title);
                transcriptStore.setIcon(historyItem.icon || null);
                void useProjectStore.getState().setActiveProjectId(historyItem.projectId);
            }
        };

        const setCurrentSegments = (segments: TranscriptSegment[]) => {
            currentSegments = segments;
            get().updateItemSegments(itemId, segments);
        };

        try {
            transcriptionService.setModelPath(config.offlineModelPath);
            transcriptionService.setEnableITN(config.enableITN ?? false);

            const tempDirectory = await tempDir();
            tempWavPath = await join(tempDirectory, `${uuidv4()}.wav`);

            const segments = await transcriptionService.transcribeFile(
                item.filePath,
                (progress) => {
                    get().updateItemStatus(itemId, 'processing', progress);
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

                get().updateItemStatus(itemId, 'processing', 96, 'polishing');
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

                get().updateItemStatus(itemId, 'processing', 98, 'translating');
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
                get().updateItemStatus(itemId, 'processing', 99, 'exporting');
                const exportPath = await exportTranscriptToDirectory({
                    segments: currentSegments,
                    directory: item.exportConfig.directory,
                    baseFileName: buildAutomationExportBaseName(item),
                    format: item.exportConfig.format,
                    mode: item.exportConfig.mode,
                });
                set((currentState) => ({
                    queueItems: currentState.queueItems.map((queueItem) => (
                        queueItem.id === itemId ? { ...queueItem, exportPath } : queueItem
                    )),
                }));
            }

            if (savedHistoryId && get().activeItemId === itemId) {
                await summaryService.persistSummary(savedHistoryId);
            }

            get().updateItemStatus(itemId, 'complete', 100);
            await notifyAutomationResult(item, 'complete');
        } catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            logger.error(`[BatchQueue] Failed to process ${item.filename}:`, error);

            try {
                if (currentSegments.length > 0) {
                    await ensureHistorySaved();
                    await persistHistorySnapshot();
                }
            } catch (historyError) {
                logger.error('[BatchQueue] Failed to persist partial result after error:', historyError);
            }

            get().setItemError(itemId, message);
            await notifyAutomationResult(item, 'error', message);
        } finally {
            if (tempWavPath) {
                try {
                    await remove(tempWavPath);
                } catch (error) {
                    logger.warn('[BatchQueue] Failed to remove temp file:', error);
                }
            }

            void get().processQueue();
        }
    },

    setActiveItem: (id) => {
        set({ activeItemId: id });

        const state = get();
        const item = state.queueItems.find((queueItem) => queueItem.id === id);
        if (item) {
            useTranscriptStore.getState().loadTranscript(
                item.segments,
                item.historyId || null,
                item.historyTitle || item.filename,
            );
            useTranscriptStore.getState().setAudioUrl(item.audioUrl || null);
            void useProjectStore.getState().setActiveProjectId(item.projectId);
        } else if (id === null) {
            useTranscriptStore.getState().loadTranscript([], null);
            useTranscriptStore.getState().setAudioUrl(null);
        }
    },

    updateItemStatus: (id, status, progress, lastKnownStage) => {
        let shouldFlushImmediately = false;
        let nextQueueItems: BatchQueueItem[] = [];
        set((state) => {
            nextQueueItems = state.queueItems.map((item) => {
                if (item.id !== id) {
                    return item;
                }

                shouldFlushImmediately = item.status !== status || (
                    lastKnownStage !== undefined && item.lastKnownStage !== lastKnownStage
                );
                return {
                    ...item,
                    status,
                    progress: progress !== undefined ? progress : item.progress,
                    lastKnownStage: lastKnownStage ?? item.lastKnownStage,
                };
            });

            return {
                queueItems: nextQueueItems,
            };
        });
        scheduleRecoverySnapshotSync(nextQueueItems, shouldFlushImmediately || status !== 'processing');
    },

    updateItemSegments: (id, segments) => {
        let nextQueueItems: BatchQueueItem[] = [];
        set((state) => {
            nextQueueItems = state.queueItems.map((item) => (
                item.id === id ? { ...item, segments } : item
            ));
            return {
                queueItems: nextQueueItems,
            };
        });
        scheduleRecoverySnapshotSync(nextQueueItems);

        const state = get();
        if (state.activeItemId === id) {
            useTranscriptStore.getState().setSegments(segments);
        }
    },

    setItemError: (id, message) => {
        let nextQueueItems: BatchQueueItem[] = [];
        set((state) => {
            nextQueueItems = state.queueItems.map((item) => (
                item.id === id
                    ? { ...item, status: 'error', errorMessage: message }
                    : item
            ));
            return {
                queueItems: nextQueueItems,
            };
        });
        scheduleRecoverySnapshotSync(nextQueueItems, true);
    },

    removeItem: (id) => {
        const state = get();
        const newItems = state.queueItems.filter((item) => item.id !== id);
        const isActiveItem = state.activeItemId === id;
        const newActiveId = newItems.length > 0 ? newItems[0].id : null;

        set({ queueItems: newItems });
        scheduleRecoverySnapshotSync(newItems, true);

        if (isActiveItem) {
            get().setActiveItem(newActiveId);
        }
    },

    clearQueue: () => {
        set({
            queueItems: [],
            activeItemId: null,
            isQueueProcessing: false,
        });
        scheduleRecoverySnapshotSync([], true);
        useTranscriptStore.getState().clearSegments();
        useTranscriptStore.getState().setAudioUrl(null);
    },
}));

/** Selector for queue items. */
export const useQueueItems = () => useBatchQueueStore((state) => state.queueItems);

/** Selector for active item ID. */
export const useActiveItemId = () => useBatchQueueStore((state) => state.activeItemId);

/** Selector for processing state. */
export const useIsQueueProcessing = () => useBatchQueueStore((state) => state.isQueueProcessing);
