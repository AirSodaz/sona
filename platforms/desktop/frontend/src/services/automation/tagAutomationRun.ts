import type { AutomationActions } from '../../types/automation';

export interface TagAutomationRunRequest {
  ruleId: string;
  historyId: string;
  inputVersion: string;
  actions: AutomationActions;
  force?: boolean;
}

export interface TagAutomationRunFinishRequest {
  ruleId: string;
  historyId: string;
  inputVersion: string;
  status: 'complete' | 'error';
  errorMessage?: string;
}

export interface TagAutomationRunPorts {
  begin: (request: TagAutomationRunRequest) => Promise<boolean>;
  finish: (request: TagAutomationRunFinishRequest) => Promise<void>;
}

let ports: TagAutomationRunPorts | null = null;

export function registerTagAutomationRunPorts(nextPorts: TagAutomationRunPorts): void {
  ports = nextPorts;
}

function requirePorts(): TagAutomationRunPorts {
  if (!ports) {
    throw new Error('Tag automation run ports are not registered.');
  }
  return ports;
}

export async function beginTagAutomationRun(
  request: TagAutomationRunRequest,
): Promise<boolean> {
  return requirePorts().begin(request);
}

export async function finishTagAutomationRun(
  request: TagAutomationRunFinishRequest,
): Promise<void> {
  await requirePorts().finish(request);
}
