import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useTaskLedgerStore } from '../taskLedgerStore';
import type { TaskLedgerRecord, TaskLedgerSnapshot } from '../../types/taskLedger';

const loadSnapshotMock = vi.fn();
const patchTaskMock = vi.fn();
const upsertTaskMock = vi.fn();
const listenMock = vi.fn();

vi.mock('@tauri-apps/api/event', () => ({
  listen: (...args: unknown[]) => listenMock(...args),
}));

vi.mock('../../services/tauri/taskLedger', () => ({
  taskLedgerLoadSnapshot: (...args: unknown[]) => loadSnapshotMock(...args),
  taskLedgerPatchTask: (...args: unknown[]) => patchTaskMock(...args),
  taskLedgerUpsertTask: (...args: unknown[]) => upsertTaskMock(...args),
  taskLedgerRemoveTask: vi.fn(),
  taskLedgerClearResolved: vi.fn(),
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

    expect(useTaskLedgerStore.getState()).toEqual(expect.objectContaining({
      tasks: [],
      updatedAt: null,
      isLoaded: true,
      isBusy: false,
      error: 'Ledger unavailable.',
    }));
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
    patchTaskMock.mockResolvedValueOnce(makeSnapshot([
      {
        ...runningTask,
        status: 'cancelRequested',
        cancelable: false,
      },
    ]));

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
    expect(useTaskLedgerStore.getState().tasks[0]).toEqual(expect.objectContaining({
      id: 'task-succeeded',
      status: 'succeeded',
      progress: 100,
    }));
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

    expect(useTaskLedgerStore.getState().tasks[0]).toEqual(expect.objectContaining({
      id: 'task-race',
      status: 'succeeded',
      progress: 100,
      cancelable: false,
    }));
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
    upsertTaskMock.mockImplementationOnce(() => new Promise<TaskLedgerSnapshot>((resolve) => {
      resolveUpsert = resolve;
    }));
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
    expect(useTaskLedgerStore.getState().tasks[0]).toEqual(expect.objectContaining({
      id: 'task-serial',
      status: 'succeeded',
      progress: 100,
    }));
  });
});
