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
import { getEffectiveConfigSnapshot } from './effectiveConfigStore';
import { emitAutomationTaskSettled } from '../services/automationRuntimeBridge';
import { isAsrRequestConfigured, resolveAsrTranscriptionRequest } from '../services/asrConfigService';
import { processBatchQueueItem } from '../services/batch/batchItemProcessor';
import { persistQueueRecoverySnapshot, toBatchQueueItem } from '../services/recoveryService';
import {
  buildBatchTaskLedgerRecord,
  createBatchTaskLedgerId,
  isTaskLedgerCancelRequested,
  patchTaskLedgerRecord,
  upsertTaskLedgerRecord,
} from '../services/taskLedgerRuntime';
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
import type { TaskLedgerStatus } from '../types/taskLedger';
import { historyService } from '../services/historyService';
import {
    applySavedBatchHistoryToQueue,
    resolveSavedBatchHistoryMeta,
} from './batchQueueHistorySync';

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

function getQueueRecoveryIds(item: BatchQueueItem): string[] {
    return [item.id, item.recoveryId]
        .filter((id): id is string => typeof id === 'string' && id.trim().length > 0);
}

function scheduleRecoverySnapshotSync(
    queueItems: BatchQueueItem[],
    immediate = false,
    resolvedIds: string[] = [],
) {
    persistQueueRecoverySnapshot(queueItems, { immediate, resolvedIds });
}

function upsertQueueItemTask(item: BatchQueueItem, status?: ReturnType<typeof buildBatchTaskLedgerRecord>['status']) {
    upsertTaskLedgerRecord(buildBatchTaskLedgerRecord(item, status));
}

function patchQueueItemTask(item: BatchQueueItem, patch: Parameters<typeof patchTaskLedgerRecord>[1]) {
    patchTaskLedgerRecord(createBatchTaskLedgerId(item.id), patch);
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

function resolveQueueItemConfig(item: BatchQueueItem): AppConfig {
    if (item.resolvedConfigSnapshot) {
        return item.resolvedConfigSnapshot;
    }

    return getEffectiveConfigSnapshot();
}

async function notifyAutomationResult(
    item: BatchQueueItem,
    status: 'complete' | 'error' | 'discarded',
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
            ?? getEffectiveConfigSnapshot();
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
        newItems.forEach((item) => upsertQueueItemTask(item, 'pending'));

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
        recoveredQueueItems.forEach((item) => upsertQueueItemTask(item, 'pending'));

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

        if (isTaskLedgerCancelRequested(createBatchTaskLedgerId(itemId))) {
            get().updateItemStatus(itemId, 'cancelled', 0);
            patchQueueItemTask(item, {
                status: 'cancelled',
                progress: 0,
                cancelable: false,
                retryable: false,
                errorMessage: undefined,
            });
            await notifyAutomationResult(item, 'discarded');
            return;
        }

        if (!isAsrRequestConfigured(resolveAsrTranscriptionRequest(config, 'batch'))) {
            const message = 'Batch ASR is not configured.';
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
                    onHistorySaved: async (historyItem) => {
                        const savedMeta = await resolveSavedBatchHistoryMeta({
                            historyItem,
                            fallbackAudioUrl: item.audioUrl,
                            fallbackProjectId: item.projectId,
                            getAudioUrl: (audioPath) => historyService.getAudioUrl(audioPath),
                        });
                        let nextQueueItems: BatchQueueItem[] = [];
                        set((currentState) => {
                            nextQueueItems = applySavedBatchHistoryToQueue(currentState.queueItems, itemId, savedMeta);
                            return {
                                queueItems: nextQueueItems,
                            };
                        });
                        scheduleRecoverySnapshotSync(nextQueueItems, true);
                        patchQueueItemTask(item, {
                            historyId: historyItem.id,
                            projectId: historyItem.projectId ?? item.projectId,
                            title: historyItem.title,
                        });

                        if (get().activeItemId === itemId) {
                            syncSavedRecordingMeta(savedMeta.title, savedMeta.historyId, savedMeta.icon, savedMeta.audioUrl ?? null);
                            void useProjectStore.getState().setActiveProjectId(savedMeta.projectId);
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
                    isCancelRequested: () => isTaskLedgerCancelRequested(createBatchTaskLedgerId(itemId)),
                },
            });

            if (isTaskLedgerCancelRequested(createBatchTaskLedgerId(itemId))) {
                get().updateItemStatus(itemId, 'cancelled', 0);
                patchQueueItemTask(item, {
                    status: 'cancelled',
                    progress: 0,
                    cancelable: false,
                    retryable: false,
                    errorMessage: undefined,
                });
                await notifyAutomationResult(item, 'discarded');
            } else {
                get().updateItemStatus(itemId, 'complete', 100);
                patchQueueItemTask(item, {
                    status: 'succeeded',
                    progress: 100,
                    cancelable: false,
                    retryable: false,
                    errorMessage: undefined,
                });
                await notifyAutomationResult(item, 'complete');
            }
        } catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            logger.error(`[BatchQueue] Failed to process ${item.filename}:`, error);

            if (message === 'Task cancelled.') {
                get().updateItemStatus(itemId, 'cancelled', 0);
                patchQueueItemTask(item, {
                    status: 'cancelled',
                    progress: 0,
                    cancelable: false,
                    retryable: false,
                    errorMessage: undefined,
                });
                await notifyAutomationResult(item, 'discarded');
            } else {
                get().setItemError(itemId, message);
                await notifyAutomationResult(item, 'error', message);
            }
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
                patchQueueItemTask(item, {
                    status: toTaskLedgerStatus(status),
                    progress: progress !== undefined ? progress : item.progress,
                    stage: lastKnownStage ?? item.lastKnownStage,
                    cancelable: status === 'pending' || status === 'processing',
                });
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
        const failedItem = nextQueueItems.find((item) => item.id === id);
        if (failedItem) {
            patchQueueItemTask(failedItem, {
                status: 'failed',
                errorMessage: message,
                retryable: true,
                cancelable: false,
            });
        }
    },

    removeItem: (id) => {
        const state = get();
        const removedItem = state.queueItems.find((item) => item.id === id);
        const newItems = state.queueItems.filter((item) => item.id !== id);
        const isActiveItem = state.activeItemId === id;
        const newActiveId = newItems.length > 0 ? newItems[0].id : null;

        set({ queueItems: newItems });
        scheduleRecoverySnapshotSync(newItems, true, removedItem ? getQueueRecoveryIds(removedItem) : []);
        if (removedItem) {
            patchQueueItemTask(removedItem, {
                status: removedItem.status === 'complete' ? 'succeeded' : 'cancelled',
                cancelable: false,
                retryable: false,
            });
        }

        if (isActiveItem) {
            get().setActiveItem(newActiveId);
        }
    },

    clearQueue: () => {
        const state = get();
        set({
            queueItems: [],
            activeItemId: null,
            isQueueProcessing: false,
        });
        scheduleRecoverySnapshotSync([], true, state.queueItems.flatMap(getQueueRecoveryIds));
        state.queueItems.forEach((item) => patchQueueItemTask(item, {
            status: item.status === 'complete' ? 'succeeded' : 'cancelled',
            cancelable: false,
            retryable: false,
        }));
        clearActiveTranscriptSession({ clearAudio: true });
    },
}));

/** Selector for queue items. */
export const useQueueItems = () => useBatchQueueStore((state) => state.queueItems);

/** Selector for active item ID. */
export const useActiveItemId = () => useBatchQueueStore((state) => state.activeItemId);

/** Selector for processing state. */
export const useIsQueueProcessing = () => useBatchQueueStore((state) => state.isQueueProcessing);
