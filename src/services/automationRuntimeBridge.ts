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

type AutomationTaskSettledListener = (payload: AutomationTaskSettledPayload) => void | Promise<void>;

const automationTaskSettledListeners = new Set<AutomationTaskSettledListener>();

export function subscribeAutomationTaskSettled(listener: AutomationTaskSettledListener): () => void {
  automationTaskSettledListeners.add(listener);

  return () => {
    automationTaskSettledListeners.delete(listener);
  };
}

export async function emitAutomationTaskSettled(payload: AutomationTaskSettledPayload): Promise<void> {
  if (automationTaskSettledListeners.size === 0) {
    return;
  }

  await Promise.all(
    [...automationTaskSettledListeners].map((listener) => listener(payload)),
  );
}
