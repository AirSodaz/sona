import { create } from 'zustand';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  TaskLedgerPatch,
  TaskLedgerRecord,
  TaskLedgerSnapshot,
} from '../types/taskLedger';
import {
  taskLedgerClearResolved,
  taskLedgerLoadSnapshot,
  taskLedgerPatchTask,
  taskLedgerRemoveTask,
  taskLedgerUpsertTask,
} from '../services/tauri/taskLedger';
import { TauriEvent } from '../services/tauri/events';
import { extractErrorMessage } from '../utils/errorUtils';
import { logger } from '../utils/logger';
import {
  getCancelRequestedIds,
  isCancelRequestedTask,
  mergeSnapshotWithLocalTasks,
  mergeTask,
  patchTaskRecord,
  shouldRetainTaskStatus,
  snapshotToTaskLedgerState,
} from './taskLedgerState';

interface UpsertTaskOptions {
  transient?: boolean;
}

interface TaskLedgerState {
  tasks: TaskLedgerRecord[];
  updatedAt: number | null;
  isLoaded: boolean;
  isBusy: boolean;
  error: string | null;
  cancelRequestedIds: Set<string>;
  loadTasks: () => Promise<void>;
  upsertTask: (record: TaskLedgerRecord, options?: UpsertTaskOptions) => Promise<void>;
  patchTask: (id: string, patch: TaskLedgerPatch, options?: UpsertTaskOptions) => Promise<void>;
  removeTask: (id: string) => Promise<void>;
  clearResolved: () => Promise<void>;
  requestCancel: (id: string) => Promise<void>;
  isCancelRequested: (id: string) => boolean;
  applySnapshot: (snapshot: TaskLedgerSnapshot) => void;
}

let taskLedgerUnlisten: UnlistenFn | null = null;
const durableWriteChains = new Map<string, Promise<void>>();

async function ensureTaskLedgerListener() {
  if (taskLedgerUnlisten) {
    return;
  }

  taskLedgerUnlisten = await listen<TaskLedgerSnapshot>(
    TauriEvent.taskLedger.updated,
    ({ payload }) => {
      useTaskLedgerStore.getState().applySnapshot(payload);
    },
  );
}

async function enqueueDurableWrite<T>(
  taskId: string,
  write: () => Promise<T>,
): Promise<T> {
  const previousWrite = durableWriteChains.get(taskId);
  const nextWrite = previousWrite
    ? previousWrite
      .catch(() => undefined)
      .then(write)
    : write();
  const trackedWrite = nextWrite.then(
    () => undefined,
    () => undefined,
  );

  durableWriteChains.set(taskId, trackedWrite);
  trackedWrite.finally(() => {
    if (durableWriteChains.get(taskId) === trackedWrite) {
      durableWriteChains.delete(taskId);
    }
  });

  return nextWrite;
}

export const useTaskLedgerStore = create<TaskLedgerState>((set, get) => ({
  tasks: [],
  updatedAt: null,
  isLoaded: false,
  isBusy: false,
  error: null,
  cancelRequestedIds: new Set(),

  loadTasks: async () => {
    set({ isBusy: true, error: null });
    try {
      await ensureTaskLedgerListener();
      const snapshot = await taskLedgerLoadSnapshot();
      set({
        ...snapshotToTaskLedgerState(snapshot),
        isLoaded: true,
        isBusy: false,
      });
    } catch (error) {
      const errorMessage = extractErrorMessage(error) || 'Failed to load task ledger.';
      logger.error('[TaskLedger] Failed to load task ledger:', error);
      set({
        tasks: [],
        updatedAt: null,
        isLoaded: true,
        isBusy: false,
        error: errorMessage,
      });
    }
  },

  upsertTask: async (record, options) => {
    set((state) => ({
      tasks: mergeTask(state.tasks, record),
      updatedAt: record.updatedAt,
      error: null,
    }));

    if (options?.transient) {
      return;
    }

    const snapshot = await enqueueDurableWrite(record.id, () => taskLedgerUpsertTask(record));
    get().applySnapshot(snapshot);
  },

  patchTask: async (id, patch, options) => {
    let patchedTask: TaskLedgerRecord | null = null;
    set((state) => ({
      tasks: state.tasks.map((task) => {
        if (task.id !== id) {
          return task;
        }
        patchedTask = patchTaskRecord(task, patch);
        return patchedTask;
      }),
    }));

    if (options?.transient || !patchedTask) {
      return;
    }

    const snapshot = await enqueueDurableWrite(id, () => taskLedgerPatchTask(id, patch));
    get().applySnapshot(snapshot);
  },

  removeTask: async (id) => {
    set((state) => ({
      tasks: state.tasks.filter((task) => task.id !== id),
    }));
    const snapshot = await enqueueDurableWrite(id, () => taskLedgerRemoveTask(id));
    get().applySnapshot(snapshot);
  },

  clearResolved: async () => {
    set((state) => ({
      tasks: state.tasks.filter((task) => shouldRetainTaskStatus(task.status)),
    }));
    const snapshot = await taskLedgerClearResolved();
    get().applySnapshot(snapshot);
  },

  requestCancel: async (id) => {
    const task = get().tasks.find((item) => item.id === id);
    if (!task || !task.cancelable || task.status === 'cancelRequested') {
      return;
    }

    set((state) => {
      const cancelRequestedIds = new Set(state.cancelRequestedIds);
      cancelRequestedIds.add(id);
      return { cancelRequestedIds };
    });

    await get().patchTask(id, {
      status: 'cancelRequested',
      cancelable: false,
    });
  },

  isCancelRequested: (id) => {
    if (get().cancelRequestedIds.has(id)) {
      return true;
    }

    return get().tasks.some((task) => task.id === id && isCancelRequestedTask(task));
  },

  applySnapshot: (snapshot) => {
    set((state) => {
      const tasks = mergeSnapshotWithLocalTasks(snapshot.tasks, state.tasks);
      const cancelRequestedIds = getCancelRequestedIds(tasks, state.cancelRequestedIds);
      return {
        tasks,
        updatedAt: snapshot.updatedAt,
        error: null,
        cancelRequestedIds,
      };
    });
  },
}));

export function resetTaskLedgerRuntimeForTests(): void {
  taskLedgerUnlisten?.();
  taskLedgerUnlisten = null;
  durableWriteChains.clear();
}
