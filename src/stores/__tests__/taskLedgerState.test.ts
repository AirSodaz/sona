import { describe, expect, it } from 'vitest';
import {
  getCancelRequestedIds,
  mergeSnapshotWithLocalTasks,
  mergeTask,
  patchTaskRecord,
  snapshotToTaskLedgerState,
} from '../taskLedgerState';
import type { TaskLedgerPatch, TaskLedgerRecord, TaskLedgerSnapshot } from '../../types/taskLedger';

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

describe('taskLedgerState', () => {
  it('keeps newer local resolved task state when merging an older snapshot', () => {
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

    expect(mergeSnapshotWithLocalTasks([pendingTask], [succeededTask])).toEqual([
      succeededTask,
    ]);
  });

  it('drops stale cancel requests for resolved tasks', () => {
    const succeededTask = makeTask({
      id: 'task-done',
      status: 'succeeded',
      progress: 100,
    });

    expect(getCancelRequestedIds([succeededTask], new Set(['task-done']))).toEqual(new Set());
  });

  it('retains transient resolved local tasks missing from durable snapshots', () => {
    const succeededTask = makeTask({
      id: 'task-session-complete',
      status: 'succeeded',
      progress: 100,
      updatedAt: 200,
    });

    expect(mergeSnapshotWithLocalTasks([], [succeededTask])).toEqual([succeededTask]);
  });

  it('normalizes null error patches without leaking null into records', () => {
    const failedTask = makeTask({
      status: 'failed',
      errorMessage: 'Previous failure',
    });

    const clearErrorPatch: TaskLedgerPatch = { errorMessage: null };

    expect(patchTaskRecord(failedTask, clearErrorPatch)).toEqual(expect.objectContaining({
      errorMessage: undefined,
    }));
  });

  it('creates store-ready state from a backend snapshot', () => {
    const cancelRequestedTask = makeTask({
      id: 'task-cancel',
      status: 'cancelRequested',
    });

    expect(snapshotToTaskLedgerState(makeSnapshot([cancelRequestedTask]))).toEqual({
      tasks: [cancelRequestedTask],
      updatedAt: 100,
      error: null,
      cancelRequestedIds: new Set(['task-cancel']),
    });
  });

  it('merges upserted tasks in newest-first order', () => {
    const olderTask = makeTask({ id: 'older', updatedAt: 100 });
    const newerTask = makeTask({ id: 'newer', updatedAt: 200 });

    expect(mergeTask([olderTask], newerTask)).toEqual([newerTask, olderTask]);
  });
});
