import { create } from 'zustand';
import { v4 as uuidv4 } from 'uuid';
import type { AppConfig } from '../types/config';
import type { AutomationExportConfig, AutomationStageConfig } from '../types/automation';
import {
  BatchQueueItem,
  BatchQueueItemOrigin,
  BatchQueueItemStatus,
} from '../types/batchQueue';
import { TranscriptSegment } from '../types/transcript';
import { getEffectiveConfigSnapshot } from './effectiveConfigStore';
import {
  processBatchQueueItemLifecycle,
  processNextBatchQueueItems,
  toTaskLedgerStatus,
} from '../services/batch/batchQueueCoordinator';
import { persistQueueRecoverySnapshot, toBatchQueueItem } from '../services/recoveryService';
import {
  buildBatchTaskLedgerRecord,
  createBatchTaskLedgerId,
  patchTaskLedgerRecord,
  upsertTaskLedgerRecord,
} from '../services/taskLedgerBuilders';
import { useConfigStore } from './configStore';
import { useProjectStore } from './projectStore';
import {
  clearActiveTranscriptSession,
  openTranscriptSession,
  setTranscriptSegments,
} from './transcriptCoordinator';
import { useTranscriptSessionStore } from './transcriptSessionStore';
import { useTranscriptStore, DEFAULT_SESSION_DATA } from './transcriptStore';
import type { RecoveredQueueItem } from '../types/recovery';
import { historyService } from '../services/historyService';
import {
    applySavedBatchHistoryToQueue,
    resolveSavedBatchHistoryMeta,
} from '../services/batch/batchQueueHistorySync';

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
                audioUrl: null,
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
        processNextBatchQueueItems({
            getQueueItems: () => get().queueItems,
            getMaxConcurrent: () => useConfigStore.getState().config.maxConcurrent || 2,
            setQueueProcessing: (isQueueProcessing) => set({ isQueueProcessing }),
            processItem: (itemId) => get()._processItem(itemId),
        });
    },

    _processItem: async (itemId: string) => {
        await processBatchQueueItemLifecycle(itemId, {
            getQueueItems: () => get().queueItems,
            getQueueItem: (id) => get().queueItems.find((queueItem) => queueItem.id === id),
            getFallbackConfigSnapshot: () => getEffectiveConfigSnapshot(),
            updateItemStatus: (id, status, progress, lastKnownStage) => {
                get().updateItemStatus(id, status, progress, lastKnownStage);
            },
            updateItemSegments: (id, segments) => {
                get().updateItemSegments(id, segments);
            },
            setItemError: (id, message) => {
                get().setItemError(id, message);
            },
            applySavedHistory: async (id, item, historyItem) => {
                const savedMeta = await resolveSavedBatchHistoryMeta({
                    historyItem,
                    fallbackProjectId: item.projectId,
                    getAudioUrl: (audioPath) => historyService.getAudioUrl(audioPath),
                });
                let nextQueueItems: BatchQueueItem[] = [];
                set((currentState) => {
                    nextQueueItems = applySavedBatchHistoryToQueue(currentState.queueItems, id, savedMeta);
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

                const transcriptStore = useTranscriptStore.getState();
                const session = transcriptStore.sessions[id] || DEFAULT_SESSION_DATA;
                useTranscriptStore.setState({
                    sessions: {
                        ...transcriptStore.sessions,
                        [id]: {
                            ...session,
                            sourceHistoryId: savedMeta.historyId,
                            title: savedMeta.title,
                            icon: savedMeta.icon ?? null,
                            audioUrl: savedMeta.audioUrl !== undefined ? savedMeta.audioUrl : session.audioUrl,
                        }
                    }
                });
                if (get().activeItemId === id) {
                    transcriptStore.rekeyCurrentSummaryState(savedMeta.historyId);
                    transcriptStore.setAudioUrl(savedMeta.audioUrl ?? null);
                    void useProjectStore.getState().setActiveProjectId(savedMeta.projectId);
                }
            },
            setItemExportPath: (id, exportPath) => {
                set((currentState) => ({
                    queueItems: currentState.queueItems.map((queueItem) => (
                        queueItem.id === id ? { ...queueItem, exportPath } : queueItem
                    )),
                }));
            },
            isActiveItem: (id) => get().activeItemId === id,
            scheduleNext: () => {
                void get().processQueue();
            },
        });
    },

    setActiveItem: (id) => {
        // Flush current session segments back to the previous queue item
        const prevActiveId = get().activeItemId;
        if (prevActiveId !== null && prevActiveId !== id) {
            const sessionSegments = useTranscriptSessionStore.getState().segments.length > 0
                ? useTranscriptSessionStore.getState().segments
                : (useTranscriptStore.getState().sessions[prevActiveId]?.segments || []);
            set((s) => ({
                queueItems: s.queueItems.map((item) =>
                    item.id === prevActiveId
                        ? { ...item, segments: sessionSegments }
                        : item
                ),
            }));
        }

        set({ activeItemId: id });

        const state = get();
        const item = state.queueItems.find((queueItem) => queueItem.id === id);
        if (id !== null && item) {
            const existingSession = useTranscriptStore.getState().sessions[id];
            if (existingSession && existingSession.segments.length > 0) {
                // Session already has data from updateItemSegments, just activate it
                useTranscriptStore.setState({
                    activeSessionId: id,
                    sessions: {
                        ...useTranscriptStore.getState().sessions,
                        [id]: {
                            ...existingSession,
                            sourceHistoryId: item.historyId || existingSession.sourceHistoryId,
                            title: item.historyTitle || existingSession.title,
                            audioUrl: item.audioUrl || existingSession.audioUrl,
                        }
                    }
                });
                useTranscriptStore.getState().setAudioUrl(item.audioUrl || existingSession.audioUrl);
            } else {
                openTranscriptSession({
                    segments: item.segments,
                    sourceHistoryId: id,
                    title: item.historyTitle || item.filename,
                    audioUrl: item.audioUrl || null,
                });
            }
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

        // Write to dedicated session in transcript store
        useTranscriptStore.getState().setSegmentsForSession(id, segments);

        const state = get();
        if (state.activeItemId === id) {
            if (useTranscriptStore.getState().activeSessionId === id) {
                setTranscriptSegments(segments);
            }
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
        // Flush session segments back to the item before removing it
        if (get().activeItemId === id) {
            const sessionSegments = useTranscriptSessionStore.getState().segments;
            set((s) => ({
                queueItems: s.queueItems.map((item) =>
                    item.id === id ? { ...item, segments: sessionSegments } : item
                ),
            }));
        }

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
        // Flush session segments back to the active item before clearing
        const activeId = get().activeItemId;
        if (activeId !== null) {
            const sessionSegments = useTranscriptSessionStore.getState().segments;
            set((s) => ({
                queueItems: s.queueItems.map((item) =>
                    item.id === activeId ? { ...item, segments: sessionSegments } : item
                ),
            }));
        }

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
