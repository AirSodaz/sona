import { create } from 'zustand';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  TaskLedgerPatch,
  TaskLedgerRecord,
  TaskLedgerSnapshot,
  TaskLedgerStatus,
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

function applyTaskPatch(record: TaskLedgerRecord, patch: TaskLedgerPatch): TaskLedgerRecord {
  return {
    ...record,
    ...patch,
    errorMessage: patch.errorMessage === null ? undefined : patch.errorMessage ?? record.errorMessage,
  };
}

function mergeTask(tasks: TaskLedgerRecord[], record: TaskLedgerRecord): TaskLedgerRecord[] {
  return [
    record,
    ...tasks.filter((task) => task.id !== record.id),
  ].sort((a, b) => b.updatedAt - a.updatedAt);
}

function shouldRetainTaskStatus(status: TaskLedgerStatus): boolean {
  return status !== 'succeeded' && status !== 'cancelled';
}

function isCancelRequestedTask(task: TaskLedgerRecord): boolean {
  return task.status === 'cancelRequested';
}

function getCancelRequestedIds(
  tasks: TaskLedgerRecord[],
  existingIds: Set<string> = new Set(),
): Set<string> {
  const cancelRequestedIds = new Set(existingIds);
  tasks.filter(isCancelRequestedTask).forEach((task) => {
    cancelRequestedIds.add(task.id);
  });
  return cancelRequestedIds;
}

function applySnapshotState(snapshot: TaskLedgerSnapshot): Pick<TaskLedgerState, 'tasks' | 'updatedAt' | 'error' | 'cancelRequestedIds'> {
  return {
    tasks: snapshot.tasks,
    updatedAt: snapshot.updatedAt,
    error: null,
    cancelRequestedIds: getCancelRequestedIds(snapshot.tasks),
  };
}

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
        ...applySnapshotState(snapshot),
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

    const snapshot = await taskLedgerUpsertTask(record);
    get().applySnapshot(snapshot);
  },

  patchTask: async (id, patch, options) => {
    let patchedTask: TaskLedgerRecord | null = null;
    set((state) => ({
      tasks: state.tasks.map((task) => {
        if (task.id !== id) {
          return task;
        }
        patchedTask = applyTaskPatch(task, patch);
        return patchedTask;
      }),
    }));

    if (options?.transient || !patchedTask) {
      return;
    }

    const snapshot = await taskLedgerPatchTask(id, patch);
    get().applySnapshot(snapshot);
  },

  removeTask: async (id) => {
    set((state) => ({
      tasks: state.tasks.filter((task) => task.id !== id),
    }));
    const snapshot = await taskLedgerRemoveTask(id);
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
      const durableIds = new Set(snapshot.tasks.map((task) => task.id));
      const cancelRequestedIds = getCancelRequestedIds(snapshot.tasks, state.cancelRequestedIds);
      const transientTasks = state.tasks.filter((task) => (
        !durableIds.has(task.id) && !shouldRetainTaskStatus(task.status)
      ));
      return {
        tasks: [...snapshot.tasks, ...transientTasks].sort((a, b) => b.updatedAt - a.updatedAt),
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
}
