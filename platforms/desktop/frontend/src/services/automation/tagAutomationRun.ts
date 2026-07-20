import type { AutomationActions } from '../../types/automation';

export interface TagAutomationRunRequest {
  ruleId: string;
  historyId: string;
  inputVersion: string;
  actions: AutomationActions;
  force?: boolean;
}

export async function beginTagAutomationRun(
  request: TagAutomationRunRequest,
): Promise<boolean> {
  const { useAutomationStore } = await import('../../stores/automationStore');
  return useAutomationStore.getState().beginTagAutomationRun(request);
}

export async function finishTagAutomationRun(args: {
  ruleId: string;
  historyId: string;
  inputVersion: string;
  status: 'complete' | 'error';
  errorMessage?: string;
}): Promise<void> {
  const { useAutomationStore } = await import('../../stores/automationStore');
  await useAutomationStore.getState().finishTagAutomationRun(args);
}
