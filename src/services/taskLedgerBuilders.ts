import type { BatchQueueItem } from '../types/batchQueue';
import type { RecoveryItemStage, RecoveredQueueItem } from '../types/recovery';
import type { TaskLedgerKind, TaskLedgerPatch, TaskLedgerRecord } from '../types/taskLedger';
import type { LlmTaskType } from './llmTaskService';
import { useTaskLedgerStore } from '../stores/taskLedgerStore';
import { logger } from '../utils/logger';

export function createBatchTaskLedgerId(queueItemId: string): string {
  return `batch-${queueItemId}`;
}

export function createLlmTaskLedgerId(taskId: string): string {
  return `llm-${taskId}`;
}

export function createRecoveryTaskLedgerId(recoveryId: string): string {
  return `recovery-${recoveryId}`;
}

export function nowTaskLedgerTimestamp(): number {
  return Date.now();
}

export function buildBatchTaskLedgerRecord(
  item: BatchQueueItem,
  status: TaskLedgerRecord['status'] = 'pending',
): TaskLedgerRecord {
  const now = nowTaskLedgerTimestamp();
  const taskKind: TaskLedgerKind = item.origin === 'automation' ? 'automation' : 'batchImport';
  return {
    id: createBatchTaskLedgerId(item.id),
    kind: taskKind,
    status,
    title: item.filename,
    progress: item.progress ?? 0,
    createdAt: now,
    updatedAt: now,
    retryable: true,
    cancelable: status === 'pending' || status === 'running',
    recoverable: Boolean(item.recoveryId),
    stage: item.lastKnownStage,
    historyId: item.historyId,
    projectId: item.projectId,
    filePath: item.filePath,
    automationRuleId: item.automationRuleId,
    sourceFingerprint: item.sourceFingerprint,
    errorMessage: item.errorMessage,
  };
}

export function buildRecoveryTaskLedgerRecord(item: RecoveredQueueItem): TaskLedgerRecord {
  return {
    id: createRecoveryTaskLedgerId(item.id),
    kind: 'recovery',
    status: 'recoverable',
    title: item.filename,
    progress: item.progress,
    createdAt: item.updatedAt,
    updatedAt: item.updatedAt,
    retryable: item.canResume,
    cancelable: false,
    recoverable: item.canResume,
    stage: item.lastKnownStage,
    historyId: item.historyId,
    projectId: item.projectId,
    filePath: item.filePath,
    automationRuleId: item.automationRuleId,
    sourceFingerprint: item.sourceFingerprint,
    errorMessage: item.canResume ? undefined : 'Source file is missing.',
  };
}

export function buildLlmTaskLedgerRecord({
  taskId,
  taskType,
  jobHistoryId,
  templateId,
  targetLanguage,
}: {
  taskId: string;
  taskType: LlmTaskType;
  jobHistoryId: string;
  templateId?: string;
  targetLanguage?: string;
}): TaskLedgerRecord {
  const now = nowTaskLedgerTimestamp();
  let kind: TaskLedgerKind;
  let title: string;
  if (taskType === 'polish') {
    kind = 'llmPolish';
    title = 'LLM Polish';
  } else if (taskType === 'translate') {
    kind = 'llmTranslate';
    title = 'Translate';
  } else {
    kind = 'llmSummary';
    title = 'AI Summary';
  }

  return {
    id: createLlmTaskLedgerId(taskId),
    kind,
    status: 'running',
    title,
    progress: 0,
    createdAt: now,
    updatedAt: now,
    retryable: true,
    cancelable: true,
    recoverable: false,
    historyId: jobHistoryId === 'current' ? undefined : jobHistoryId,
    templateId,
    targetLanguage,
  };
}

export function upsertTaskLedgerRecord(record: TaskLedgerRecord, options?: { transient?: boolean }): void {
  void useTaskLedgerStore.getState().upsertTask(record, options).catch((error) => {
    logger.error('[TaskLedger] Failed to upsert task:', error);
  });
}

export function patchTaskLedgerRecord(id: string, patch: TaskLedgerPatch, options?: { transient?: boolean }): void {
  void useTaskLedgerStore.getState().patchTask(id, {
    ...patch,
    updatedAt: patch.updatedAt ?? nowTaskLedgerTimestamp(),
  }, options).catch((error) => {
    logger.error('[TaskLedger] Failed to patch task:', error);
  });
}

export function removeTaskLedgerRecord(id: string): void {
  void useTaskLedgerStore.getState().removeTask(id).catch((error) => {
    logger.error('[TaskLedger] Failed to remove task:', error);
  });
}

export function requestTaskLedgerCancel(id: string): void {
  void useTaskLedgerStore.getState().requestCancel(id).catch((error) => {
    logger.error('[TaskLedger] Failed to request task cancellation:', error);
  });
}

export function isTaskLedgerCancelRequested(id: string): boolean {
  return useTaskLedgerStore.getState().isCancelRequested(id);
}

export function recoveryStageToTaskStage(stage?: RecoveryItemStage): string | undefined {
  return stage;
}
