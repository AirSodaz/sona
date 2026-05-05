import { useAutomationStore } from '../stores/automationStore';
import type { TaskLedgerRecord } from '../types/taskLedger';
import { normalizeError } from '../utils/errorUtils';
import { patchTaskLedgerRecord } from './taskLedgerRuntime';

function patchRetryPreflightFailure(task: TaskLedgerRecord, error: unknown): void {
  patchTaskLedgerRecord(task.id, {
    status: 'failed',
    progress: 0,
    cancelable: false,
    retryable: true,
    errorMessage: normalizeError(error).message,
  });
}

export async function retryAutomationTaskFromLedger(task: TaskLedgerRecord): Promise<void> {
  try {
    if (task.kind !== 'automation') {
      throw new Error('Unsupported automation task type.');
    }

    if (!task.automationRuleId || !task.filePath) {
      throw new Error('Automation task is missing retry metadata.');
    }

    await useAutomationStore.getState().retryFailedFile(task.automationRuleId, task.filePath);
  } catch (error) {
    patchRetryPreflightFailure(task, error);
    throw Object.assign(new Error(normalizeError(error).message), { cause: error });
  }
}
