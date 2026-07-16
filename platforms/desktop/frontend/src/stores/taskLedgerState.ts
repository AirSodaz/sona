import type {
  TaskLedgerPatch,
  TaskLedgerRecord,
  TaskLedgerSnapshot,
  TaskLedgerStatus,
} from '../types/taskLedger';

export interface TaskLedgerSnapshotState {
  tasks: TaskLedgerRecord[];
  updatedAt: number | null;
  error: null;
  cancelRequestedIds: Set<string>;
}

export function patchTaskRecord(record: TaskLedgerRecord, patch: TaskLedgerPatch): TaskLedgerRecord {
  return {
    ...record,
    ...patch,
    errorMessage: patch.errorMessage === null ? undefined : patch.errorMessage ?? record.errorMessage,
    tagIds: patch.tagIds === null ? [] : patch.tagIds ?? record.tagIds,
  };
}

export function mergeTask(tasks: TaskLedgerRecord[], record: TaskLedgerRecord): TaskLedgerRecord[] {
  return [
    record,
    ...tasks.filter((task) => task.id !== record.id),
  ].sort((a, b) => b.updatedAt - a.updatedAt);
}

export function shouldRetainTaskStatus(status: TaskLedgerStatus): boolean {
  return status !== 'succeeded' && status !== 'cancelled';
}

export function isResolvedTaskStatus(status: TaskLedgerStatus): boolean {
  return !shouldRetainTaskStatus(status);
}

export function isCancelRequestedTask(task: TaskLedgerRecord): boolean {
  return task.status === 'cancelRequested';
}

export function getCancelRequestedIds(
  tasks: TaskLedgerRecord[],
  existingIds: Set<string> = new Set(),
): Set<string> {
  const taskIds = new Set(tasks.map((task) => task.id));
  const resolvedTaskIds = new Set(
    tasks
      .filter((task) => isResolvedTaskStatus(task.status))
      .map((task) => task.id),
  );
  const cancelRequestedIds = new Set(
    Array.from(existingIds).filter((id) => taskIds.has(id) && !resolvedTaskIds.has(id)),
  );
  tasks.filter(isCancelRequestedTask).forEach((task) => {
    cancelRequestedIds.add(task.id);
  });
  return cancelRequestedIds;
}

export function snapshotToTaskLedgerState(snapshot: TaskLedgerSnapshot): TaskLedgerSnapshotState {
  return {
    tasks: snapshot.tasks,
    updatedAt: snapshot.updatedAt,
    error: null,
    cancelRequestedIds: getCancelRequestedIds(snapshot.tasks),
  };
}

function shouldKeepLocalTask(existing: TaskLedgerRecord, incoming: TaskLedgerRecord): boolean {
  return existing.updatedAt > incoming.updatedAt
    || (isResolvedTaskStatus(existing.status) && existing.updatedAt >= incoming.updatedAt);
}

export function mergeSnapshotWithLocalTasks(
  snapshotTasks: TaskLedgerRecord[],
  localTasks: TaskLedgerRecord[],
): TaskLedgerRecord[] {
  const localTasksById = new Map(localTasks.map((task) => [task.id, task]));
  const snapshotTaskIds = new Set(snapshotTasks.map((task) => task.id));
  const mergedSnapshotTasks = snapshotTasks.map((task) => {
    const existing = localTasksById.get(task.id);
    return existing && shouldKeepLocalTask(existing, task) ? existing : task;
  });
  const transientResolvedTasks = localTasks.filter((task) => (
    !snapshotTaskIds.has(task.id) && isResolvedTaskStatus(task.status)
  ));

  return [...mergedSnapshotTasks, ...transientResolvedTasks]
    .sort((a, b) => b.updatedAt - a.updatedAt);
}
