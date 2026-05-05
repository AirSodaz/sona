import type { TaskLedgerPatch, TaskLedgerRecord, TaskLedgerSnapshot } from '../../types/taskLedger';
import { TauriCommand } from './commands';
import { invokeTauri } from './invoke';

export async function taskLedgerLoadSnapshot(): Promise<TaskLedgerSnapshot> {
  return invokeTauri(TauriCommand.taskLedger.loadSnapshot);
}

export async function taskLedgerUpsertTask(record: TaskLedgerRecord): Promise<TaskLedgerSnapshot> {
  return invokeTauri(TauriCommand.taskLedger.upsertTask, { record });
}

export async function taskLedgerPatchTask(
  id: string,
  patch: TaskLedgerPatch,
): Promise<TaskLedgerSnapshot> {
  return invokeTauri(TauriCommand.taskLedger.patchTask, { id, patch });
}

export async function taskLedgerRemoveTask(id: string): Promise<TaskLedgerSnapshot> {
  return invokeTauri(TauriCommand.taskLedger.removeTask, { id });
}

export async function taskLedgerClearResolved(): Promise<TaskLedgerSnapshot> {
  return invokeTauri(TauriCommand.taskLedger.clearResolved);
}
