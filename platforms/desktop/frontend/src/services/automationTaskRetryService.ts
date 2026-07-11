import { useAutomationStore } from '../stores/automationStore';
import type { TaskLedgerRecord } from '../types/taskLedger';
import { handleTaskRetryPreflightFailure } from './taskRetryFailure';

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
    handleTaskRetryPreflightFailure(task, error);
  }
}
