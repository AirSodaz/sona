export type TaskLedgerKind =
  | 'batchImport'
  | 'automation'
  | 'llmPolish'
  | 'llmTranslate'
  | 'llmSummary'
  | 'recovery'
  | 'update';

export type TaskLedgerStatus =
  | 'pending'
  | 'running'
  | 'cancelRequested'
  | 'failed'
  | 'recoverable'
  | 'interrupted'
  | 'cancelled'
  | 'succeeded';

export interface TaskLedgerRecord {
  id: string;
  kind: TaskLedgerKind;
  status: TaskLedgerStatus;
  title: string;
  progress: number;
  createdAt: number;
  updatedAt: number;
  retryable: boolean;
  cancelable: boolean;
  recoverable: boolean;
  stage?: string;
  historyId?: string;
  projectId?: string | null;
  filePath?: string;
  automationRuleId?: string;
  sourceFingerprint?: string;
  errorMessage?: string;
  templateId?: string;
  targetLanguage?: string;
}

export interface TaskLedgerSnapshot {
  version: number;
  updatedAt: number | null;
  tasks: TaskLedgerRecord[];
}

export type TaskLedgerPatch = Partial<Omit<TaskLedgerRecord, 'id' | 'errorMessage'>> & {
  errorMessage?: string | null;
};

export function isTaskLedgerActiveStatus(status: TaskLedgerStatus): boolean {
  return status === 'pending' || status === 'running' || status === 'cancelRequested';
}

export function isTaskLedgerActionableStatus(status: TaskLedgerStatus): boolean {
  return status === 'failed' || status === 'recoverable' || status === 'interrupted';
}
