import { create } from 'zustand';
import { v4 as uuidv4 } from 'uuid';
import { convertFileSrc } from '@tauri-apps/api/core';
import type { AppConfig } from '../types/config';
import type { AutomationExportConfig, AutomationStageConfig } from '../types/automation';
import {
  BatchQueueItem,
  BatchQueueItemOrigin,
  BatchQueueItemStatus,
} from '../types/batchQueue';
import { TranscriptSegment } from '../types/transcript';
import { resolveEffectiveConfig } from '../services/effectiveConfigService';
import { emitAutomationTaskSettled } from '../services/automationRuntimeBridge';
import { processBatchQueueItem } from '../services/batch/batchItemProcessor';
import { persistQueueRecoverySnapshot, toBatchQueueItem } from '../services/recoveryService';
import { useConfigStore } from './configStore';
import { useProjectStore } from './projectStore';
import {
  clearActiveTranscriptSession,
  openTranscriptSession,
  setTranscriptSegments,
  syncSavedRecordingMeta,
} from './transcriptCoordinator';
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

        if (!config.offlineModelPath) {
            const message = 'No offline model path configured.';
            get().setItemError(itemId, message);
            await notifyAutomationResult(item, 'error', message);
            return;
        }

        try {
            await processBatchQueueItem({
                item,
                config,
                callbacks: {
                    updateStatus: (status, progress, lastKnownStage) => {
                        get().updateItemStatus(itemId, status, progress, lastKnownStage);
                    },
                    updateSegments: (segments) => {
                        get().updateItemSegments(itemId, segments);
                    },
                    onHistorySaved: (historyItem) => {
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
                            syncSavedRecordingMeta(historyItem.title, historyItem.id, historyItem.icon || null);
                            void useProjectStore.getState().setActiveProjectId(historyItem.projectId);
                        }
                    },
                    onExportComplete: (exportPath) => {
                        set((currentState) => ({
                            queueItems: currentState.queueItems.map((queueItem) => (
                                queueItem.id === itemId ? { ...queueItem, exportPath } : queueItem
                            )),
                        }));
                    },
                    isActiveItem: () => get().activeItemId === itemId,
                },
            });

            get().updateItemStatus(itemId, 'complete', 100);
            await notifyAutomationResult(item, 'complete');
        } catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            logger.error(`[BatchQueue] Failed to process ${item.filename}:`, error);

            get().setItemError(itemId, message);
            await notifyAutomationResult(item, 'error', message);
        } finally {
            void get().processQueue();
        }
    },

    setActiveItem: (id) => {
        set({ activeItemId: id });

        const state = get();
        const item = state.queueItems.find((queueItem) => queueItem.id === id);
        if (item) {
            openTranscriptSession({
                segments: item.segments,
                sourceHistoryId: item.historyId || null,
                title: item.historyTitle || item.filename,
                audioUrl: item.audioUrl || null,
            });
            void useProjectStore.getState().setActiveProjectId(item.projectId);
        } else if (id === null) {
            clearActiveTranscriptSession({ clearAudio: true, title: '' });
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
            setTranscriptSegments(segments);
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
        clearActiveTranscriptSession({ clearAudio: true });
    },
}));

/** Selector for queue items. */
export const useQueueItems = () => useBatchQueueStore((state) => state.queueItems);

/** Selector for active item ID. */
export const useActiveItemId = () => useBatchQueueStore((state) => state.activeItemId);

/** Selector for processing state. */
export const useIsQueueProcessing = () => useBatchQueueStore((state) => state.isQueueProcessing);
