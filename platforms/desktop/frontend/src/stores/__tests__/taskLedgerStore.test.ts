import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { TaskLedgerRecord, TaskLedgerSnapshot } from '../../types/taskLedger';
import { useTaskLedgerStore } from '../taskLedgerStore';

const loadSnapshotMock = vi.fn();
const patchTaskMock = vi.fn();
const upsertTaskMock = vi.fn();
const listenMock = vi.fn();

vi.mock('@tauri-apps/api/event', () => ({
  listen: (...args: unknown[]) => listenMock(...args),
}));

const removeTaskMock = vi.fn();
const clearResolvedMock = vi.fn();

vi.mock('../../services/tauri/taskLedger', () => ({
  taskLedgerLoadSnapshot: (...args: unknown[]) => loadSnapshotMock(...args),
  taskLedgerPatchTask: (...args: unknown[]) => patchTaskMock(...args),
  taskLedgerUpsertTask: (...args: unknown[]) => upsertTaskMock(...args),
  taskLedgerRemoveTask: (...args: unknown[]) => removeTaskMock(...args),
  taskLedgerClearResolved: (...args: unknown[]) => clearResolvedMock(...args),
}));

function makeTask(overrides: Partial<TaskLedgerRecord> = {}): TaskLedgerRecord {
  return {
    id: 'task-1',
    kind: 'batchImport',
    status: 'running',
    title: 'meeting.wav',
    progress: 20,
    createdAt: 100,
    updatedAt: 100,
    retryable: true,
    cancelable: true,
    recoverable: false,
    ...overrides,
  };
}

function makeSnapshot(tasks: TaskLedgerRecord[]): TaskLedgerSnapshot {
  return {
    version: 1,
    updatedAt: 100,
    tasks,
  };
}

function resetTaskLedgerStore() {
  useTaskLedgerStore.setState({
    tasks: [],
    updatedAt: null,
    isLoaded: false,
    isBusy: false,
    error: null,
    cancelRequestedIds: new Set(),
  });
}

describe('taskLedgerStore', () => {
  beforeEach(() => {
    loadSnapshotMock.mockReset();
    patchTaskMock.mockReset();
    upsertTaskMock.mockReset();
    listenMock.mockReset();
    resetTaskLedgerStore();
    listenMock.mockResolvedValue(vi.fn());
    removeTaskMock.mockReset();
    clearResolvedMock.mockReset();
    removeTaskMock.mockResolvedValue(makeSnapshot([]));
    clearResolvedMock.mockResolvedValue(makeSnapshot([]));
  });

  it('loads the persisted task ledger snapshot', async () => {
    const task = makeTask();
    loadSnapshotMock.mockResolvedValueOnce(makeSnapshot([task]));

    await useTaskLedgerStore.getState().loadTasks();

    expect(useTaskLedgerStore.getState().tasks).toEqual([task]);
    expect(useTaskLedgerStore.getState().isLoaded).toBe(true);
    expect(loadSnapshotMock).toHaveBeenCalledTimes(1);
  });

  it('marks the ledger loaded and stores the error when loading fails', async () => {
    loadSnapshotMock.mockRejectedValueOnce(new Error('Ledger unavailable.'));

    await useTaskLedgerStore.getState().loadTasks();

    expect(useTaskLedgerStore.getState()).toEqual(
      expect.objectContaining({
        tasks: [],
        updatedAt: null,
        isLoaded: true,
        isBusy: false,
        error: 'Ledger unavailable.',
      })
    );
  });

  it('treats cancelRequested tasks from the backend snapshot as soft-cancelled', async () => {
    const task = makeTask({ id: 'task-cancelled-late', status: 'cancelRequested' });
    loadSnapshotMock.mockResolvedValueOnce(makeSnapshot([task]));

    await useTaskLedgerStore.getState().loadTasks();

    expect(useTaskLedgerStore.getState().isCancelRequested('task-cancelled-late')).toBe(true);
  });

  it('requests soft cancellation for cancelable running tasks', async () => {
    const runningTask = makeTask({ id: 'task-running', status: 'running', cancelable: true });
    useTaskLedgerStore.setState({ tasks: [runningTask] });
    patchTaskMock.mockResolvedValueOnce(
      makeSnapshot([
        {
          ...runningTask,
          status: 'cancelRequested',
          cancelable: false,
        },
      ])
    );

    await useTaskLedgerStore.getState().requestCancel('task-running');

    expect(patchTaskMock).toHaveBeenCalledWith('task-running', {
      status: 'cancelRequested',
      cancelable: false,
    });
    expect(useTaskLedgerStore.getState().isCancelRequested('task-running')).toBe(true);
    expect(useTaskLedgerStore.getState().tasks[0].status).toBe('cancelRequested');
  });

  it('upserts transient succeeded tasks without requiring durable persistence', async () => {
    const succeededTask = makeTask({ id: 'task-succeeded', status: 'succeeded', progress: 100 });
    upsertTaskMock.mockResolvedValueOnce(makeSnapshot([]));

    await useTaskLedgerStore.getState().upsertTask(succeededTask, { transient: true });

    expect(upsertTaskMock).not.toHaveBeenCalled();
    expect(useTaskLedgerStore.getState().tasks[0]).toEqual(succeededTask);
  });

  it('sends resolved task statuses to the backend so durable records are cleared', async () => {
    const runningTask = makeTask({ id: 'task-succeeded', status: 'running' });
    useTaskLedgerStore.setState({ tasks: [runningTask] });
    patchTaskMock.mockResolvedValueOnce(makeSnapshot([]));

    await useTaskLedgerStore.getState().patchTask('task-succeeded', {
      status: 'succeeded',
      progress: 100,
      cancelable: false,
    });

    expect(patchTaskMock).toHaveBeenCalledWith('task-succeeded', {
      status: 'succeeded',
      progress: 100,
      cancelable: false,
    });
    expect(useTaskLedgerStore.getState().tasks[0]).toEqual(
      expect.objectContaining({
        id: 'task-succeeded',
        status: 'succeeded',
        progress: 100,
      })
    );
  });

  it('keeps a locally completed task when an older durable snapshot arrives late', () => {
    const pendingTask = makeTask({
      id: 'task-race',
      status: 'pending',
      progress: 0,
      updatedAt: 100,
      cancelable: true,
    });
    const succeededTask = makeTask({
      id: 'task-race',
      status: 'succeeded',
      progress: 100,
      updatedAt: 200,
      cancelable: false,
    });

    useTaskLedgerStore.setState({
      tasks: [succeededTask],
      cancelRequestedIds: new Set(['task-race']),
    });

    useTaskLedgerStore.getState().applySnapshot(makeSnapshot([pendingTask]));

    expect(useTaskLedgerStore.getState().tasks[0]).toEqual(
      expect.objectContaining({
        id: 'task-race',
        status: 'succeeded',
        progress: 100,
        cancelable: false,
      })
    );
    expect(useTaskLedgerStore.getState().isCancelRequested('task-race')).toBe(false);
  });

  it('serializes durable writes for the same task id', async () => {
    const pendingTask = makeTask({
      id: 'task-serial',
      status: 'pending',
      progress: 0,
      updatedAt: 100,
    });
    let resolveUpsert!: (snapshot: TaskLedgerSnapshot) => void;
    upsertTaskMock.mockImplementationOnce(
      () =>
        new Promise<TaskLedgerSnapshot>((resolve) => {
          resolveUpsert = resolve;
        })
    );
    patchTaskMock.mockResolvedValueOnce(makeSnapshot([]));

    const upsertPromise = useTaskLedgerStore.getState().upsertTask(pendingTask);
    const patchPromise = useTaskLedgerStore.getState().patchTask('task-serial', {
      status: 'succeeded',
      progress: 100,
      cancelable: false,
      updatedAt: 200,
    });

    await Promise.resolve();

    expect(upsertTaskMock).toHaveBeenCalledTimes(1);
    expect(patchTaskMock).not.toHaveBeenCalled();

    resolveUpsert(makeSnapshot([pendingTask]));
    await upsertPromise;
    await patchPromise;

    expect(patchTaskMock).toHaveBeenCalledWith('task-serial', {
      status: 'succeeded',
      progress: 100,
      cancelable: false,
      updatedAt: 200,
    });
    expect(useTaskLedgerStore.getState().tasks[0]).toEqual(
      expect.objectContaining({
        id: 'task-serial',
        status: 'succeeded',
        progress: 100,
      })
    );
  });

  it('clears succeeded tasks only, retaining cancelled and active tasks', async () => {
    const activeTask = makeTask({ id: 'task-active', status: 'running' });
    const succeededTask = makeTask({ id: 'task-succeeded', status: 'succeeded' });
    const cancelledTask = makeTask({ id: 'task-cancelled', status: 'cancelled' });

    useTaskLedgerStore.setState({
      tasks: [activeTask, succeededTask, cancelledTask],
    });

    removeTaskMock.mockResolvedValueOnce(makeSnapshot([activeTask, cancelledTask]));

    await useTaskLedgerStore.getState().clearSucceeded();

    expect(removeTaskMock).toHaveBeenCalledWith('task-succeeded');
    expect(clearResolvedMock).not.toHaveBeenCalled();
    expect(useTaskLedgerStore.getState().tasks).toEqual([activeTask, cancelledTask]);
  });

  it('delegates to clearResolved when all resolved tasks are succeeded', async () => {
    const activeTask = makeTask({ id: 'task-active', status: 'running' });
    const succeededTask = makeTask({ id: 'task-succeeded', status: 'succeeded' });

    useTaskLedgerStore.setState({
      tasks: [activeTask, succeededTask],
    });

    clearResolvedMock.mockResolvedValueOnce(makeSnapshot([activeTask]));

    await useTaskLedgerStore.getState().clearSucceeded();

    expect(clearResolvedMock).toHaveBeenCalledTimes(1);
    expect(useTaskLedgerStore.getState().tasks).toEqual([activeTask]);
  });

  it('clears all non-active tasks while strictly protecting active tasks', async () => {
    const runningTask = makeTask({ id: 'task-running', status: 'running' });
    const pendingTask = makeTask({ id: 'task-pending', status: 'pending' });
    const succeededTask = makeTask({ id: 'task-succeeded', status: 'succeeded' });
    const cancelledTask = makeTask({ id: 'task-cancelled', status: 'cancelled' });
    const failedTask = makeTask({ id: 'task-failed', status: 'failed' });

    useTaskLedgerStore.setState({
      tasks: [runningTask, pendingTask, succeededTask, cancelledTask, failedTask],
    });

    clearResolvedMock.mockResolvedValueOnce(makeSnapshot([runningTask, pendingTask, failedTask]));
    removeTaskMock.mockResolvedValueOnce(makeSnapshot([runningTask, pendingTask]));

    await useTaskLedgerStore.getState().clearAllNonActive();

    expect(clearResolvedMock).toHaveBeenCalledTimes(1);
    expect(removeTaskMock).toHaveBeenCalledWith('task-failed');
    expect(useTaskLedgerStore.getState().tasks).toEqual([runningTask, pendingTask]);
  });
});
