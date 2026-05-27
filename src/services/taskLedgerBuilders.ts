import type { BatchQueueItem } from '../types/batchQueue';
import type { RecoveryItemStage, RecoveredQueueItem } from '../types/recovery';
import type { TaskLedgerKind, TaskLedgerPatch, TaskLedgerRecord } from '../types/taskLedger';
import type { LlmTaskType } from './llmTaskTypes';
import { useTaskLedgerStore } from '../stores/taskLedgerStore';
import { logger } from '../utils/logger';

export interface TaskLedgerBuildersPorts {
  useTaskLedgerStore: typeof useTaskLedgerStore;
}

export class TaskLedgerBuilders {
  constructor(private readonly ports: TaskLedgerBuildersPorts) {}

  createBatchTaskLedgerId = (queueItemId: string): string => {
    return `batch-${queueItemId}`;
  }

  createLlmTaskLedgerId = (taskId: string): string => {
    return `llm-${taskId}`;
  }

  createRecoveryTaskLedgerId = (recoveryId: string): string => {
    return `recovery-${recoveryId}`;
  }

  nowTaskLedgerTimestamp = (): number => {
    return Date.now();
  }

  buildBatchTaskLedgerRecord = (
    item: BatchQueueItem,
    status: TaskLedgerRecord['status'] = 'pending',
  ): TaskLedgerRecord => {
    const now = this.nowTaskLedgerTimestamp();
    const taskKind: TaskLedgerKind = item.origin === 'automation' ? 'automation' : 'batchImport';
    return {
      id: this.createBatchTaskLedgerId(item.id),
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

  buildRecoveryTaskLedgerRecord = (item: RecoveredQueueItem): TaskLedgerRecord => {
    return {
      id: this.createRecoveryTaskLedgerId(item.id),
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

  buildLlmTaskLedgerRecord = ({
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
  }): TaskLedgerRecord => {
    const now = this.nowTaskLedgerTimestamp();
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
      id: this.createLlmTaskLedgerId(taskId),
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

  upsertTaskLedgerRecord = (record: TaskLedgerRecord, options?: { transient?: boolean }): void => {
    void this.ports.useTaskLedgerStore.getState().upsertTask(record, options).catch((error) => {
      logger.error('[TaskLedger] Failed to upsert task:', error);
    });
  }

  patchTaskLedgerRecord = (id: string, patch: TaskLedgerPatch, options?: { transient?: boolean }): void => {
    void this.ports.useTaskLedgerStore.getState().patchTask(id, {
      ...patch,
      updatedAt: patch.updatedAt ?? this.nowTaskLedgerTimestamp(),
    }, options).catch((error) => {
      logger.error('[TaskLedger] Failed to patch task:', error);
    });
  }

  removeTaskLedgerRecord = (id: string): void => {
    void this.ports.useTaskLedgerStore.getState().removeTask(id).catch((error) => {
      logger.error('[TaskLedger] Failed to remove task:', error);
    });
  }

  requestTaskLedgerCancel = (id: string): void => {
    void this.ports.useTaskLedgerStore.getState().requestCancel(id).catch((error) => {
      logger.error('[TaskLedger] Failed to request task cancellation:', error);
    });
  }

  isTaskLedgerCancelRequested = (id: string): boolean => {
    return this.ports.useTaskLedgerStore.getState().isCancelRequested(id);
  }

  recoveryStageToTaskStage = (stage?: RecoveryItemStage): string | undefined => {
    return stage;
  }
}

export function createTaskLedgerBuilders(ports: TaskLedgerBuildersPorts): TaskLedgerBuilders {
  return new TaskLedgerBuilders(ports);
}

export const taskLedgerBuilders = createTaskLedgerBuilders({
  useTaskLedgerStore,
});

export const {
  createBatchTaskLedgerId,
  createLlmTaskLedgerId,
  createRecoveryTaskLedgerId,
  nowTaskLedgerTimestamp,
  buildBatchTaskLedgerRecord,
  buildRecoveryTaskLedgerRecord,
  buildLlmTaskLedgerRecord,
  upsertTaskLedgerRecord,
  patchTaskLedgerRecord,
  removeTaskLedgerRecord,
  requestTaskLedgerCancel,
  isTaskLedgerCancelRequested,
  recoveryStageToTaskStage,
} = taskLedgerBuilders;
