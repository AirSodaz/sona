import type { RecoveryItemStage } from '../types/recovery';

export interface AutomationTaskSettledPayload {
  ruleId: string;
  filePath: string;
  sourceFingerprint: string;
  size: number;
  mtimeMs: number;
  status: 'complete' | 'error';
  processedAt: number;
  historyId?: string;
  exportPath?: string;
  errorMessage?: string;
  stage?: RecoveryItemStage;
}

type AutomationTaskSettledHandler = (payload: AutomationTaskSettledPayload) => void | Promise<void>;

let automationTaskSettledHandler: AutomationTaskSettledHandler | null = null;

export function registerAutomationTaskSettledHandler(handler: AutomationTaskSettledHandler | null) {
  automationTaskSettledHandler = handler;
}

export async function notifyAutomationTaskSettled(payload: AutomationTaskSettledPayload): Promise<void> {
  if (!automationTaskSettledHandler) {
    return;
  }

  await automationTaskSettledHandler(payload);
}
