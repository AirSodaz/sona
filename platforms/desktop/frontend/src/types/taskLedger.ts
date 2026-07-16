import type {
  TaskLedgerKind as GeneratedTaskLedgerKind,
  TaskLedgerPatch_Deserialize as GeneratedTaskLedgerPatch,
  TaskLedgerRecord_Serialize as GeneratedTaskLedgerRecord,
  TaskLedgerSnapshot_Serialize as GeneratedTaskLedgerSnapshot,
  TaskLedgerStatus as GeneratedTaskLedgerStatus,
} from '../bindings';

type WithoutNull<T> = { [K in keyof T]: Exclude<T[K], null> };
type NormalizedTaskLedgerRecord = WithoutNull<GeneratedTaskLedgerRecord>;
type NormalizedTaskLedgerPatch = WithoutNull<GeneratedTaskLedgerPatch>;

export type TaskLedgerKind = GeneratedTaskLedgerKind;
export type TaskLedgerStatus = GeneratedTaskLedgerStatus;
export type TaskLedgerRecord = Omit<NormalizedTaskLedgerRecord, 'projectId'> &
  Pick<GeneratedTaskLedgerRecord, 'projectId'>;
export type TaskLedgerSnapshot = Omit<GeneratedTaskLedgerSnapshot, 'tasks'> & {
  tasks: TaskLedgerRecord[];
};
export type TaskLedgerPatch = Omit<
  NormalizedTaskLedgerPatch,
  'errorMessage' | 'projectId'
> &
  Pick<GeneratedTaskLedgerPatch, 'errorMessage' | 'projectId'>;

export function isTaskLedgerActiveStatus(status: TaskLedgerStatus): boolean {
  return status === 'pending' || status === 'running' || status === 'cancelRequested';
}

export function isTaskLedgerActionableStatus(status: TaskLedgerStatus): boolean {
  return status === 'failed' || status === 'recoverable' || status === 'interrupted';
}
