import type React from 'react';
import {
  AutomationIcon,
  CloseIcon,
  CompleteIcon,
  DownloadIcon,
  ErrorIcon,
  FileTextIcon,
  PendingIcon,
  ProcessingIcon,
  RestoreIcon,
  SparklesIcon,
} from '../../components/Icons';
import type {
  NotificationEntry,
  NotificationPriority,
  NotificationTone,
} from '../../types/notification';
import type { RecoveryItemStage } from '../../types/recovery';
import type { TaskLedgerKind, TaskLedgerRecord, TaskLedgerStatus } from '../../types/taskLedger';
import { isTaskLedgerActionableStatus, isTaskLedgerActiveStatus } from '../../types/taskLedger';
import type { TaskCenterActionRegistry } from '../useTaskLedgerActions';
import { type TFunction, toNotificationAction } from './adapterUtils';

function getFileName(filePath?: string): string | null {
  if (!filePath) {
    return null;
  }
  const filename = filePath.split(/[/\\]/).pop();
  return filename || null;
}

function getStageLabel(stage: RecoveryItemStage | string | undefined, t: TFunction): string | null {
  if (!stage) {
    return null;
  }
  return t(`recovery.stage.${stage}`, { defaultValue: stage });
}

function getPriority(task: TaskLedgerRecord): NotificationPriority {
  if (isTaskLedgerActiveStatus(task.status)) {
    return 'active';
  }
  if (isTaskLedgerActionableStatus(task.status)) {
    return 'action';
  }
  return 'info';
}

function getTone(task: TaskLedgerRecord): NotificationTone {
  if (task.kind === 'recovery') {
    return 'accent';
  }
  if (task.status === 'failed' || task.status === 'interrupted') {
    return 'error';
  }
  if (task.status === 'succeeded') {
    return 'success';
  }
  if (task.status === 'cancelled') {
    return 'info';
  }
  return 'accent';
}

function getIcon(task: TaskLedgerRecord): React.ReactNode {
  if (task.status === 'failed' || task.status === 'interrupted') {
    return <ErrorIcon />;
  }
  if (task.status === 'succeeded') {
    return <CompleteIcon />;
  }
  if (task.status === 'cancelled') {
    return <CloseIcon />;
  }
  switch (task.kind) {
    case 'automation':
      return <AutomationIcon />;
    case 'llmPolish':
    case 'llmTranslate':
    case 'llmSummary':
      return <SparklesIcon />;
    case 'recovery':
      return <RestoreIcon />;
    case 'batchImport':
      return <FileTextIcon />;
    case 'update':
      return <DownloadIcon />;
    default:
      return task.status === 'running' ? <ProcessingIcon /> : <PendingIcon />;
  }
}

function isLlmTaskKind(kind: TaskLedgerKind): boolean {
  return kind === 'llmPolish' || kind === 'llmTranslate' || kind === 'llmSummary';
}

function getTaskKindLabel(kind: TaskLedgerKind, t: TFunction): string {
  const labels: Record<TaskLedgerKind, string> = {
    batchImport: 'Batch import',
    automation: 'Automation',
    llmPolish: 'LLM polish',
    llmTranslate: 'Translation',
    llmSummary: 'AI summary',
    recovery: 'Recovery',
    update: 'Update',
  };
  return t(`task_center.kind.${kind}`, { defaultValue: labels[kind] });
}

function getTaskStatusLabel(status: TaskLedgerStatus, t: TFunction): string {
  const labels: Record<TaskLedgerStatus, string> = {
    pending: 'Pending',
    running: 'Running',
    cancelRequested: 'Stopping',
    failed: 'Failed',
    recoverable: 'Recoverable',
    interrupted: 'Interrupted',
    cancelled: 'Cancelled',
    succeeded: 'Succeeded',
  };
  return t(`task_center.status.${status}`, { defaultValue: labels[status] });
}

function getTaskBody(task: TaskLedgerRecord, t: TFunction): string {
  const kindLabel = getTaskKindLabel(task.kind, t);
  const statusLabel = getTaskStatusLabel(task.status, t);
  const stageLabel = getStageLabel(task.stage, t);
  const fileName = getFileName(task.filePath);
  const base = stageLabel
    ? t('task_center.task_body_stage', {
        defaultValue: '{{kind}} · {{status}} · {{stage}}',
        kind: kindLabel,
        status: statusLabel,
        stage: stageLabel,
      })
    : t('task_center.task_body', {
        defaultValue: '{{kind}} · {{status}}',
        kind: kindLabel,
        status: statusLabel,
      });

  if (!fileName || fileName === task.title) {
    return base;
  }

  return t('task_center.task_body_file', {
    defaultValue: '{{base}} · {{fileName}}',
    base,
    fileName,
  });
}

function shouldHideRecoveryRelatedTaskUntilLoaded(task: TaskLedgerRecord): boolean {
  if (task.kind === 'recovery') {
    return true;
  }
  if (task.kind !== 'batchImport' && task.kind !== 'automation') {
    return false;
  }
  return (
    task.status === 'interrupted' ||
    task.status === 'recoverable' ||
    isTaskLedgerActiveStatus(task.status)
  );
}

/**
 * Returns the id of a stale batch-import task that should be hidden when a
 * recoverable recovery entry already covers the same item.
 */
function getStaleQueueTaskIdForRecovery(task: TaskLedgerRecord): string | null {
  if (
    task.kind !== 'recovery' ||
    task.status !== 'recoverable' ||
    !task.id.startsWith('recovery-')
  ) {
    return null;
  }
  return `batch-${task.id.slice('recovery-'.length)}`;
}

function buildDetail(task: TaskLedgerRecord, t: TFunction): string | null {
  const parts: string[] = [];

  const isCancelPendingLlm = task.status === 'cancelRequested' && isLlmTaskKind(task.kind);
  if (isCancelPendingLlm) {
    parts.push(
      t('task_center.cancel_pending_hint', {
        defaultValue: 'Stops after the current step and skips the final writeback.',
      })
    );
  }

  const stageLabel = getStageLabel(task.stage, t);
  if (stageLabel && !isTaskLedgerActiveStatus(task.status)) {
    parts.push(t('automation.notifications.stage_detail', { stage: stageLabel }));
  }

  if (task.errorMessage) {
    parts.push(task.errorMessage);
  }

  return parts.length > 0 ? parts.join('\n') : null;
}

/**
 * Adapts TaskLedgerRecord[] from the ledger store into NotificationEntry[].
 */
export function adaptTaskLedgerEntries(
  tasks: TaskLedgerRecord[],
  isRecoveryLoaded: boolean,
  t: TFunction,
  actionRegistry: TaskCenterActionRegistry
): NotificationEntry[] {
  const staleQueueTaskIds = new Set(
    tasks.map(getStaleQueueTaskIdForRecovery).filter((id): id is string => Boolean(id))
  );

  return tasks
    .filter((task) => isRecoveryLoaded || !shouldHideRecoveryRelatedTaskUntilLoaded(task))
    .filter((task) => !staleQueueTaskIds.has(task.id))
    .map((task): NotificationEntry => {
      const ledgerActions = actionRegistry.getLedgerTaskActions(task);

      return {
        id: task.id,
        source: 'task',
        priority: getPriority(task),
        tone: getTone(task),
        icon: getIcon(task),
        title: task.title,
        body: getTaskBody(task, t),
        timestamp: task.updatedAt,
        progress: isTaskLedgerActiveStatus(task.status) ? task.progress : undefined,
        actions: ledgerActions.map(toNotificationAction),
        detail: buildDetail(task, t),
        itemClassName: 'notification-center-item-task',
      };
    });
}
