import { v4 as uuidv4 } from 'uuid';
import { create } from 'zustand';
import {
  processBatchQueueItemLifecycle,
  processNextBatchQueueItems,
  toTaskLedgerStatus,
} from '../services/batch/batchQueueCoordinator';
import {
  applySavedBatchHistoryToQueue,
  resolveSavedBatchHistoryMeta,
} from '../services/batch/batchQueueHistorySync';
import { historyService } from '../services/historyService';
import { type EffectivePipelineSnapshot, resolveItemPipeline } from '../services/projectPipeline';
import { persistQueueRecoverySnapshot, toBatchQueueItem } from '../services/recoveryService';
import {
  buildBatchTaskLedgerRecord,
  createBatchTaskLedgerId,
  patchTaskLedgerRecord,
  upsertTaskLedgerRecord,
} from '../services/taskLedgerBuilders';
import { cancelBatchTask } from '../services/tauri/recognizer';
import type {
  AutomationExportConfig,
  AutomationResolutionSnapshot,
  AutomationStageConfig,
} from '../types/automation';
import type {
  BatchQueueItem,
  BatchQueueItemOrigin,
  BatchQueueItemStatus,
} from '../types/batchQueue';
import type { AppConfig } from '../types/config';
import type { RecoveredQueueItem } from '../types/recovery';
import type { TranscriptSegment } from '../types/transcript';
import { useConfigStore } from './configStore';
import { getEffectiveConfigSnapshot } from './effectiveConfigStore';
import { useProjectStore } from './projectStore';
import { useTaskLedgerStore } from './taskLedgerStore';
import { clearActiveTranscriptSession, setTranscriptSegments } from './transcriptCoordinator';
import { useTranscriptSessionStore } from './transcriptSessionStore';
import { DEFAULT_SESSION_DATA, useTranscriptStore } from './transcriptStore';

interface AddFilesOptions {
  origin?: BatchQueueItemOrigin;
  automationRuleId?: string;
  automationRuleName?: string;
  resolvedConfigSnapshot?: AppConfig;
  exportConfig?: AutomationExportConfig | null;
  stageConfig?: AutomationStageConfig | null;
  automationResolutionSnapshot?: AutomationResolutionSnapshot;
  sourceFingerprint?: string;
  projectId?: string | null;
  pipelineSnapshot?: EffectivePipelineSnapshot;
  tagIds?: string[];
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
  /** Whether queue processing is paused by user. */
  isQueuePaused: boolean;
  /** Pauses processing of pending items. */
  pauseQueue: () => void;
  /** Resumes processing of pending items. */
  resumeQueue: () => void;
  /** Retries a failed or cancelled queue item. */
  retryItem: (id: string) => void;
  /** Retries all failed and cancelled queue items. */
  retryAllFailed: () => void;
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
  updateItemStatus: (
    id: string,
    status: BatchQueueItemStatus,
    progress?: number,
    lastKnownStage?: RecoveredQueueItem['lastKnownStage']
  ) => void;
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
  /** Sets an item's target project and refreshes its pipeline snapshot. */
  setItemProjectId: (id: string, projectId: string | null) => void;
  /**
   * Removes an item from the queue.
   *
   * @param id Item ID to remove.
   */
  removeItem: (id: string) => void;
  /** Clears all items from the queue. */
  clearQueue: () => void;
  /** Clears only completed items from the queue. */
  clearCompleted: () => void;
  /** internal helper */
  _processItem: (itemId: string) => Promise<void>;
  /**
   * Records the active Rust `process_batch_file` instance ID on the queue
   * item so that `removeItem` / `clearQueue` can cancel it in real time.
   *
   * @param id Queue item ID.
   * @param instanceId Rust instance ID, or null to clear after completion.
   */
  setItemActiveInstanceId: (id: string, instanceId: string | null) => void;
}

function getQueueRecoveryIds(item: BatchQueueItem): string[] {
  return [item.id, item.recoveryId].filter(
    (id): id is string => typeof id === 'string' && id.trim().length > 0
  );
}

function scheduleRecoverySnapshotSync(
  queueItems: BatchQueueItem[],
  immediate = false,
  resolvedIds: string[] = []
) {
  persistQueueRecoverySnapshot(queueItems, { immediate, resolvedIds });
}

function upsertQueueItemTask(
  item: BatchQueueItem,
  status?: ReturnType<typeof buildBatchTaskLedgerRecord>['status']
) {
  upsertTaskLedgerRecord(buildBatchTaskLedgerRecord(item, status));
}

function patchQueueItemTask(
  item: BatchQueueItem,
  patch: Parameters<typeof patchTaskLedgerRecord>[1]
) {
  patchTaskLedgerRecord(createBatchTaskLedgerId(item.id), patch);
}

/**
 * Zustand store for managing batch transcription queue.
 */
export const useBatchQueueStore = create<BatchQueueState>((set, get) => ({
  queueItems: [],
  activeItemId: null,
  isQueueProcessing: false,
  isQueuePaused: false,

  pauseQueue: () => {
    set({ isQueuePaused: true, isQueueProcessing: false });
  },

  resumeQueue: () => {
    set({ isQueuePaused: false });
    void get().processQueue();
  },

  retryItem: (id: string) => {
    let nextQueueItems: BatchQueueItem[] = [];
    set((state) => {
      nextQueueItems = state.queueItems.map((item) => {
        if (item.id !== id) return item;
        return {
          ...item,
          status: 'pending' as BatchQueueItemStatus,
          progress: 0,
          errorMessage: undefined,
        };
      });
      return { queueItems: nextQueueItems };
    });
    scheduleRecoverySnapshotSync(nextQueueItems, true);
    const retriedItem = nextQueueItems.find((item) => item.id === id);
    if (retriedItem) {
      upsertQueueItemTask(retriedItem, 'pending');
    }
    if (!get().isQueueProcessing && !get().isQueuePaused) {
      void get().processQueue();
    }
  },

  retryAllFailed: () => {
    let nextQueueItems: BatchQueueItem[] = [];
    const retriedIds: string[] = [];
    set((state) => {
      nextQueueItems = state.queueItems.map((item) => {
        if (item.status === 'error' || item.status === 'cancelled') {
          retriedIds.push(item.id);
          return {
            ...item,
            status: 'pending' as BatchQueueItemStatus,
            progress: 0,
            errorMessage: undefined,
          };
        }
        return item;
      });
      return { queueItems: nextQueueItems };
    });
    if (retriedIds.length > 0) {
      scheduleRecoverySnapshotSync(nextQueueItems, true);
      nextQueueItems
        .filter((item) => retriedIds.includes(item.id))
        .forEach((item) => upsertQueueItemTask(item, 'pending'));
      if (!get().isQueueProcessing && !get().isQueuePaused) {
        void get().processQueue();
      }
    }
  },
  addFiles: (filePaths, options) => {
    const projectStore = useProjectStore.getState();
    const activeProjectId = options?.projectId ?? projectStore.activeProjectId ?? null;
    const resolvedConfigSnapshot = options?.resolvedConfigSnapshot ?? getEffectiveConfigSnapshot();
    const exportFileNamePrefix = options?.exportFileNamePrefix ?? '';

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
        pipelineSnapshot: options?.pipelineSnapshot,
        // New queue entries have a single canonical project assignment.
        // Legacy tagIds are only read when restoring pre-migration entries.
        tagIds: undefined,
        origin: options?.origin || 'manual',
        automationRuleId: options?.automationRuleId,
        automationRuleName: options?.automationRuleName,
        resolvedConfigSnapshot,
        exportConfig: options?.exportConfig || null,
        stageConfig: options?.stageConfig || null,
        automationResolutionSnapshot: options?.automationResolutionSnapshot,
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
        queueItems: nextQueueItems,
      };
    });
    scheduleRecoverySnapshotSync(nextQueueItems, true);
    newItems.forEach((item) => upsertQueueItemTask(item, 'pending'));

    const state = get();
    if (!state.activeItemId && newItems.length > 0) {
      get().setActiveItem(newItems[0].id);
    }

    if (!state.isQueueProcessing && !state.isQueuePaused) {
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

    if (!state.isQueueProcessing && !state.isQueuePaused) {
      void get().processQueue();
    }
  },

  processQueue: async () => {
    if (get().isQueuePaused) {
      return;
    }
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
          getAudioUrl: (historyId) => historyService.getAudioUrl(historyId),
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
            },
          },
        });
        if (get().activeItemId === id) {
          transcriptStore.rekeyCurrentSummaryState(savedMeta.historyId);
          transcriptStore.setAudioUrl(savedMeta.audioUrl ?? null);
          // Only update the active project when still in batch mode,
          // to avoid overriding the browse scope in the projects view.
          const currentMode = useTranscriptStore.getState().mode;
          if (currentMode === 'batch') {
            void useProjectStore.getState().setActiveProjectId(savedMeta.projectId ?? null);
          }
        }
      },
      setItemExportPath: (id, exportPath) => {
        set((currentState) => ({
          queueItems: currentState.queueItems.map((queueItem) =>
            queueItem.id === id ? { ...queueItem, exportPath } : queueItem
          ),
        }));
      },
      setItemActiveInstanceId: (id, instanceId) => {
        set((currentState) => ({
          queueItems: currentState.queueItems.map((queueItem) =>
            queueItem.id === id ? { ...queueItem, activeInstanceId: instanceId } : queueItem
          ),
        }));
      },
      isActiveItem: (id) => get().activeItemId === id,
      scheduleNext: () => {
        if (!get().isQueuePaused) {
          void get().processQueue();
        }
      },
    });
  },

  setActiveItem: (id) => {
    // Flush current session segments back to the previous queue item
    const prevActiveId = get().activeItemId;
    if (prevActiveId !== null && prevActiveId !== id) {
      const sessionSegments =
        useTranscriptSessionStore.getState().segments.length > 0
          ? useTranscriptSessionStore.getState().segments
          : useTranscriptStore.getState().sessions[prevActiveId]?.segments || [];
      set((s) => ({
        queueItems: s.queueItems.map((item) =>
          item.id === prevActiveId ? { ...item, segments: sessionSegments } : item
        ),
      }));
    }

    set({ activeItemId: id });

    const state = get();
    const item = state.queueItems.find((queueItem) => queueItem.id === id);
    if (id !== null && item) {
      let transcriptState = useTranscriptStore.getState();
      if (!transcriptState.sessions[id]) {
        transcriptState.setSegmentsForSession(id, item.segments);
        transcriptState = useTranscriptStore.getState();
      }
      const session = transcriptState.sessions[id] || DEFAULT_SESSION_DATA;
      const audioUrl = item.audioUrl || session.audioUrl;
      useTranscriptStore.setState({
        activeSessionId: id,
        sessions: {
          ...transcriptState.sessions,
          [id]: {
            ...session,
            sourceHistoryId: item.historyId || session.sourceHistoryId,
            title: item.historyTitle || session.title || item.filename,
            audioUrl,
          },
        },
      });
      useTranscriptStore.getState().setAudioUrl(audioUrl);
      void useProjectStore.getState().setActiveProjectId(item.projectId);
    } else if (id === null) {
      clearActiveTranscriptSession({ clearAudio: true, title: '' });
    }
  },
  setItemProjectId: (id, projectId) => {
    set((state) => {
      const item = state.queueItems.find((queueItem) => queueItem.id === id);
      if (!item || item.status === 'processing' || item.status === 'complete') {
        return state;
      }
      const projects = useProjectStore.getState().projects;
      const config = item.resolvedConfigSnapshot ?? getEffectiveConfigSnapshot();
      const pipelineSnapshot = resolveItemPipeline(projectId, projects, config);
      return {
        queueItems: state.queueItems.map((queueItem) =>
          queueItem.id === id ? { ...queueItem, projectId, pipelineSnapshot } : queueItem
        ),
      };
    });
  },

  updateItemStatus: (id, status, progress, lastKnownStage) => {
    let shouldFlushImmediately = false;
    let nextQueueItems: BatchQueueItem[] = [];
    set((state) => {
      nextQueueItems = state.queueItems.map((item) => {
        if (item.id !== id) {
          return item;
        }

        shouldFlushImmediately =
          item.status !== status ||
          (lastKnownStage !== undefined && item.lastKnownStage !== lastKnownStage);
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
      nextQueueItems = state.queueItems.map((item) =>
        item.id === id ? { ...item, segments } : item
      );
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
      nextQueueItems = state.queueItems.map((item) =>
        item.id === id ? { ...item, status: 'error', errorMessage: message } : item
      );
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

    // Signal cancellation to the task ledger so isCancelRequested() fires
    // inside the running pipeline, and tell Rust to abort the transcription.
    if (removedItem && (removedItem.status === 'processing' || removedItem.status === 'pending')) {
      void useTaskLedgerStore.getState().requestCancel(createBatchTaskLedgerId(id));
      if (removedItem.activeInstanceId) {
        void cancelBatchTask(removedItem.activeInstanceId);
      }
    }

    const newItems = state.queueItems.filter((item) => item.id !== id);
    const isActiveItem = state.activeItemId === id;
    const newActiveId = newItems.length > 0 ? newItems[0].id : null;

    set({ queueItems: newItems });
    scheduleRecoverySnapshotSync(
      newItems,
      true,
      removedItem ? getQueueRecoveryIds(removedItem) : []
    );
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

    // Signal cancellation for every active/pending item so in-flight
    // tasks are interrupted both at the JS pipeline level and in Rust.
    state.queueItems.forEach((item) => {
      if (item.status === 'processing' || item.status === 'pending') {
        void useTaskLedgerStore.getState().requestCancel(createBatchTaskLedgerId(item.id));
        if (item.activeInstanceId) {
          void cancelBatchTask(item.activeInstanceId);
        }
      }
    });

    set({
      queueItems: [],
      activeItemId: null,
      isQueueProcessing: false,
      isQueuePaused: false,
    });
    scheduleRecoverySnapshotSync([], true, state.queueItems.flatMap(getQueueRecoveryIds));
    state.queueItems.forEach((item) =>
      patchQueueItemTask(item, {
        status: item.status === 'complete' ? 'succeeded' : 'cancelled',
        cancelable: false,
        retryable: false,
      })
    );
    clearActiveTranscriptSession({ clearAudio: true });
  },

  clearCompleted: () => {
    const state = get();
    const completedItems = state.queueItems.filter((item) => item.status === 'complete');
    if (completedItems.length === 0) return;

    const activeId = state.activeItemId;
    const isActiveItemCompleted = completedItems.some((item) => item.id === activeId);
    if (activeId !== null && isActiveItemCompleted) {
      const sessionSegments = useTranscriptSessionStore.getState().segments;
      set((s) => ({
        queueItems: s.queueItems.map((item) =>
          item.id === activeId ? { ...item, segments: sessionSegments } : item
        ),
      }));
    }

    const remainingItems = state.queueItems.filter((item) => item.status !== 'complete');
    const newActiveId = isActiveItemCompleted
      ? remainingItems.length > 0
        ? remainingItems[0].id
        : null
      : activeId;

    set({
      queueItems: remainingItems,
      activeItemId: newActiveId,
    });

    scheduleRecoverySnapshotSync(remainingItems, true, completedItems.flatMap(getQueueRecoveryIds));

    completedItems.forEach((item) =>
      patchQueueItemTask(item, {
        status: 'succeeded',
        cancelable: false,
        retryable: false,
      })
    );

    if (isActiveItemCompleted) {
      get().setActiveItem(newActiveId);
    }
  },

  setItemActiveInstanceId: (id, instanceId) => {
    set((currentState) => ({
      queueItems: currentState.queueItems.map((queueItem) =>
        queueItem.id === id ? { ...queueItem, activeInstanceId: instanceId } : queueItem
      ),
    }));
  },
}));

/** Selector for queue items. */
export const useQueueItems = () => useBatchQueueStore((state) => state.queueItems);

/** Selector for active item ID. */
export const useActiveItemId = () => useBatchQueueStore((state) => state.activeItemId);

/** Selector for processing state. */
export const useIsQueueProcessing = () => useBatchQueueStore((state) => state.isQueueProcessing);

/** Selector for paused state. */
export const useIsQueuePaused = () => useBatchQueueStore((state) => state.isQueuePaused);
