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
import {
  createBatchTaskLedgerId,
  parseRecoveryIdFromTaskId,
} from '../../services/taskLedgerBuilders';
import type {
  NotificationEntry,
  NotificationPriority,
  NotificationSubItem,
  NotificationTone,
} from '../../types/notification';
import type { RecoveryItemStage } from '../../types/recovery';
import type { TaskLedgerKind, TaskLedgerRecord, TaskLedgerStatus } from '../../types/taskLedger';
import { isTaskLedgerActionableStatus, isTaskLedgerActiveStatus } from '../../types/taskLedger';
import type { TaskCenterActionRegistry } from '../useTaskLedgerActions';
import { formatNotificationTimestamp, type TFunction } from './adapterUtils';

export { formatNotificationTimestamp };

export interface AdaptTaskLedgerOptions {
  ruleNameById?: Record<string, string>;
  now?: number;
}

const TERMINAL_STAGE_NAMES = new Set(['completed', 'cancelled', 'failed']);
const GENERIC_LLM_TITLES = new Set(['LLM Polish', 'Translate', 'AI Summary']);

type GroupTitleVariant = 'active' | 'failed' | 'completed' | 'cancelled' | 'finished';

const GROUP_TITLE_SPECS: Record<
  'automation' | 'batchImport',
  Record<GroupTitleVariant, { key: string; defaultValue: string }>
> = {
  automation: {
    active: {
      key: 'task_center.automation_active_title',
      defaultValue: '{{ruleName}} · Processing {{count}} files',
    },
    failed: {
      key: 'task_center.automation_failed_title',
      defaultValue: '{{ruleName}} · {{count}} files failed',
    },
    completed: {
      key: 'task_center.automation_completed_title',
      defaultValue: '{{ruleName}} · Completed {{count}} files',
    },
    cancelled: {
      key: 'task_center.automation_cancelled_title',
      defaultValue: '{{ruleName}} · {{count}} files cancelled',
    },
    finished: {
      key: 'task_center.automation_finished_title',
      defaultValue: '{{ruleName}} · {{count}} files finished',
    },
  },
  batchImport: {
    active: {
      key: 'task_center.batch_active_title',
      defaultValue: 'Batch transcription · {{count}} files',
    },
    failed: {
      key: 'task_center.batch_failed_title',
      defaultValue: 'Batch transcription · {{count}} files failed',
    },
    completed: {
      key: 'task_center.batch_completed_title',
      defaultValue: 'Batch transcription · {{count}} files completed',
    },
    cancelled: {
      key: 'task_center.batch_cancelled_title',
      defaultValue: 'Batch transcription · {{count}} files cancelled',
    },
    finished: {
      key: 'task_center.batch_finished_title',
      defaultValue: 'Batch transcription · {{count}} files finished',
    },
  },
};

function getFileName(filePath?: string): string | null {
  if (!filePath) {
    return null;
  }
  const filename = filePath.split(/[/\\]/).pop();
  return filename || null;
}

function getStageLabel(
  stage: RecoveryItemStage | string | undefined,
  status: TaskLedgerStatus,
  t: TFunction
): string | null {
  if (!stage || TERMINAL_STAGE_NAMES.has(stage)) {
    return null;
  }
  const shouldShowStage =
    isTaskLedgerActiveStatus(status) || status === 'recoverable' || status === 'interrupted';
  if (!shouldShowStage) {
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

function getKindIcon(kind: TaskLedgerKind, isRunning = false): React.ReactNode {
  switch (kind) {
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
      return isRunning ? <ProcessingIcon /> : <PendingIcon />;
  }
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
  return getKindIcon(task.kind, task.status === 'running');
}

function getGroupIcon(kind: TaskLedgerKind, priority: NotificationPriority): React.ReactNode {
  if (priority === 'action') {
    return kind === 'recovery' ? <RestoreIcon /> : <ErrorIcon />;
  }
  if (priority === 'info') {
    return <CompleteIcon />;
  }
  return getKindIcon(kind, true);
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

function getTaskDisplayTitle(task: TaskLedgerRecord, t: TFunction): string {
  if (isLlmTaskKind(task.kind)) {
    const baseTitle = GENERIC_LLM_TITLES.has(task.title)
      ? getTaskKindLabel(task.kind, t)
      : task.title;
    if (
      task.kind === 'llmTranslate' &&
      task.targetLanguage?.trim() &&
      !baseTitle.includes(task.targetLanguage.trim())
    ) {
      return `${baseTitle} (${task.targetLanguage.trim()})`;
    }
    return baseTitle;
  }
  return task.title;
}

function formatStatusStageLabel(
  statusLabel: string,
  stageLabel: string | null,
  t: TFunction
): string {
  return stageLabel
    ? t('task_center.task_body_status_stage', {
        defaultValue: '{{status}} · {{stage}}',
        status: statusLabel,
        stage: stageLabel,
      })
    : statusLabel;
}

function getTaskBody(
  task: TaskLedgerRecord,
  displayTitle: string,
  t: TFunction,
  ruleName?: string
): string {
  const kindLabel = getTaskKindLabel(task.kind, t);
  const statusLabel = getTaskStatusLabel(task.status, t);
  const stageLabel = getStageLabel(task.stage, task.status, t);
  const fileName = getFileName(task.filePath);

  if (isLlmTaskKind(task.kind) && displayTitle.startsWith(kindLabel)) {
    return formatStatusStageLabel(statusLabel, stageLabel, t);
  }

  const effectiveKindLabel =
    task.kind === 'automation' && ruleName ? `${kindLabel} (${ruleName})` : kindLabel;

  const base = stageLabel
    ? t('task_center.task_body_stage', {
        defaultValue: '{{kind}} · {{status}} · {{stage}}',
        kind: effectiveKindLabel,
        status: statusLabel,
        stage: stageLabel,
      })
    : t('task_center.task_body', {
        defaultValue: '{{kind}} · {{status}}',
        kind: effectiveKindLabel,
        status: statusLabel,
      });

  if (!fileName || fileName === displayTitle) {
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
  if (task.kind !== 'recovery' || task.status !== 'recoverable') {
    return null;
  }
  const recoveryId = parseRecoveryIdFromTaskId(task.id);
  return recoveryId ? createBatchTaskLedgerId(recoveryId) : null;
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

  const stageLabel = getStageLabel(task.stage, task.status, t);
  if (stageLabel && !isTaskLedgerActiveStatus(task.status)) {
    parts.push(t('automation.notifications.stage_detail', { stage: stageLabel }));
  }

  if (task.errorMessage?.trim()) {
    parts.push(task.errorMessage.trim());
  }

  return parts.length > 0 ? parts.join('\n') : null;
}

export function getTaskGroupEntryId(
  kind: TaskLedgerKind,
  priority: NotificationPriority,
  automationRuleId?: string
): string {
  if (kind === 'automation') {
    return `group-automation-${automationRuleId || 'default'}-${priority}`;
  }
  return `group-${kind}-${priority}`;
}

function getGroupBucketKey(task: TaskLedgerRecord): string | null {
  const priority = getPriority(task);
  if (task.kind === 'batchImport') {
    return `batchImport::${priority}`;
  }
  if (task.kind === 'automation') {
    return `automation::${task.automationRuleId || 'default'}::${priority}`;
  }
  if (task.kind === 'recovery') {
    return `recovery::${priority}`;
  }
  return null;
}

function getActiveStatusRank(status: TaskLedgerStatus): number {
  if (status === 'running') return 0;
  if (status === 'cancelRequested') return 1;
  if (status === 'pending') return 2;
  return 3;
}

function buildSingleEntry(
  task: TaskLedgerRecord,
  t: TFunction,
  actionRegistry: TaskCenterActionRegistry,
  ruleNameById: Record<string, string>,
  now: number
): NotificationEntry {
  const ledgerActions = actionRegistry.getLedgerTaskActions(task);
  const displayTitle = getTaskDisplayTitle(task, t);
  const ruleName = task.automationRuleId ? ruleNameById[task.automationRuleId] : undefined;

  return {
    id: task.id,
    source: 'task',
    priority: getPriority(task),
    tone: getTone(task),
    icon: getIcon(task),
    title: displayTitle,
    body: getTaskBody(task, displayTitle, t, ruleName),
    timestamp: task.updatedAt,
    timestampLabel: formatNotificationTimestamp(task.updatedAt, now, t),
    progress: isTaskLedgerActiveStatus(task.status) ? task.progress : undefined,
    actions: ledgerActions.row,
    closeAction: ledgerActions.close,
    detail: buildDetail(task, t),
    itemClassName: 'notification-center-item-task',
  };
}

function buildSubItem(
  task: TaskLedgerRecord,
  t: TFunction,
  actionRegistry: TaskCenterActionRegistry
): NotificationSubItem {
  const statusLabel = getTaskStatusLabel(task.status, t);
  const stageLabel = getStageLabel(task.stage, task.status, t);
  const subtitle = formatStatusStageLabel(statusLabel, stageLabel, t);
  const ledgerActions = actionRegistry.getLedgerTaskActions(task);

  return {
    id: task.id,
    title: getTaskDisplayTitle(task, t) || getFileName(task.filePath) || task.id,
    subtitle,
    tone: getTone(task),
    progress: isTaskLedgerActiveStatus(task.status) ? task.progress : undefined,
    detail: task.errorMessage?.trim() || null,
    actions: ledgerActions.row.filter((action) => action.id !== 'openTarget'),
    closeAction: ledgerActions.close,
  };
}

interface GroupCopyResult {
  title: string;
  body: string;
  tone: NotificationTone;
  progress?: number;
}

function resolveGroupTitle(
  kind: TaskLedgerKind,
  variant: GroupTitleVariant,
  count: number,
  ruleName: string | undefined,
  t: TFunction
): string {
  const groupKind = kind === 'automation' ? 'automation' : 'batchImport';
  const spec = GROUP_TITLE_SPECS[groupKind][variant];
  const automationLabel = ruleName || getTaskKindLabel('automation', t);
  return t(spec.key, {
    ruleName: automationLabel,
    count,
    defaultValue: spec.defaultValue,
  });
}

function buildRecoveryGroupCopy(
  tasks: TaskLedgerRecord[],
  latestTitle: string,
  t: TFunction
): GroupCopyResult {
  const count = tasks.length;
  const missingCount = tasks.filter((item) => !item.recoverable).length;
  const title = t('task_center.recovery_group_title', {
    count,
    defaultValue: '{{count}} interrupted tasks ready to recover',
  });
  const body =
    missingCount > 0
      ? t('task_center.recovery_group_body_missing', {
          count,
          missingCount,
          defaultValue: '{{missingCount}} source file(s) missing · Latest: {{fileName}}',
          fileName: latestTitle,
        })
      : t('task_center.recovery_group_body', {
          count,
          fileName: latestTitle,
          defaultValue: 'Latest: {{fileName}}',
        });
  return { title, body, tone: 'accent' };
}

function buildActiveGroupCopy(
  kind: TaskLedgerKind,
  tasks: TaskLedgerRecord[],
  latestTitle: string,
  ruleName: string | undefined,
  t: TFunction
): GroupCopyResult {
  const count = tasks.length;
  const runningTasks = tasks.filter((item) => item.status === 'running');
  const pendingCount = tasks.filter((item) => item.status === 'pending').length;
  const stoppingCount = tasks.filter((item) => item.status === 'cancelRequested').length;
  const avgProgress = Math.round(
    tasks.reduce((sum, item) => sum + (Number.isFinite(item.progress) ? item.progress : 0), 0) /
      Math.max(count, 1)
  );
  const currentFile = runningTasks[0]?.title || latestTitle;
  const parts: string[] = [];

  if (runningTasks.length > 0) {
    parts.push(
      t('task_center.group_running_count', {
        count: runningTasks.length,
        defaultValue: 'Running {{count}}',
      })
    );
  }
  if (pendingCount > 0) {
    parts.push(
      t('task_center.group_pending_count', {
        count: pendingCount,
        defaultValue: 'Pending {{count}}',
      })
    );
  }
  if (stoppingCount > 0) {
    parts.push(
      t('task_center.group_stopping_count', {
        count: stoppingCount,
        defaultValue: 'Stopping {{count}}',
      })
    );
  }
  if (currentFile) {
    parts.push(currentFile);
  }

  return {
    title: resolveGroupTitle(kind, 'active', count, ruleName, t),
    body: parts.join(' · '),
    tone: 'accent',
    progress: avgProgress,
  };
}

function buildActionGroupCopy(
  kind: TaskLedgerKind,
  tasks: TaskLedgerRecord[],
  latestTitle: string,
  ruleName: string | undefined,
  t: TFunction
): GroupCopyResult {
  const latestError = tasks.find((item) => item.errorMessage?.trim())?.errorMessage?.trim();
  const body = latestError
    ? t('task_center.group_failed_body_error', {
        fileName: latestTitle,
        error: latestError,
        defaultValue: 'Latest: {{fileName}} · {{error}}',
      })
    : t('task_center.group_latest_file', {
        fileName: latestTitle,
        defaultValue: 'Latest: {{fileName}}',
      });

  return {
    title: resolveGroupTitle(kind, 'failed', tasks.length, ruleName, t),
    body,
    tone: 'error',
  };
}

function buildInfoGroupCopy(
  kind: TaskLedgerKind,
  tasks: TaskLedgerRecord[],
  latestTitle: string,
  ruleName: string | undefined,
  t: TFunction
): GroupCopyResult {
  const succeededCount = tasks.filter((item) => item.status === 'succeeded').length;
  const cancelledCount = tasks.filter((item) => item.status === 'cancelled').length;

  const variant: GroupTitleVariant =
    cancelledCount === 0 ? 'completed' : succeededCount === 0 ? 'cancelled' : 'finished';

  const bodyParts: string[] = [];
  if (succeededCount > 0 && cancelledCount > 0) {
    bodyParts.push(
      t('task_center.group_succeeded_count', {
        count: succeededCount,
        defaultValue: '{{count}} succeeded',
      })
    );
    bodyParts.push(
      t('task_center.group_cancelled_count', {
        count: cancelledCount,
        defaultValue: '{{count}} cancelled',
      })
    );
  } else if (latestTitle) {
    bodyParts.push(
      t('task_center.group_latest_file', {
        fileName: latestTitle,
        defaultValue: 'Latest: {{fileName}}',
      })
    );
  }

  return {
    title: resolveGroupTitle(kind, variant, tasks.length, ruleName, t),
    body: bodyParts.join(' · '),
    tone: succeededCount > 0 ? 'success' : 'info',
  };
}

function buildGroupCopy(
  kind: TaskLedgerKind,
  priority: NotificationPriority,
  tasks: TaskLedgerRecord[],
  ruleName: string | undefined,
  t: TFunction
): GroupCopyResult {
  const latestTitle = tasks[0]?.title || getFileName(tasks[0]?.filePath) || '';

  if (kind === 'recovery') {
    return buildRecoveryGroupCopy(tasks, latestTitle, t);
  }
  if (priority === 'active') {
    return buildActiveGroupCopy(kind, tasks, latestTitle, ruleName, t);
  }
  if (priority === 'action') {
    return buildActionGroupCopy(kind, tasks, latestTitle, ruleName, t);
  }
  return buildInfoGroupCopy(kind, tasks, latestTitle, ruleName, t);
}

function buildAggregatedEntry(
  tasks: TaskLedgerRecord[],
  t: TFunction,
  actionRegistry: TaskCenterActionRegistry,
  ruleNameById: Record<string, string>,
  now: number
): NotificationEntry {
  const sortedTasks = [...tasks].sort((a, b) => {
    const rankDiff = getActiveStatusRank(a.status) - getActiveStatusRank(b.status);
    if (rankDiff !== 0) {
      return rankDiff;
    }
    return b.updatedAt - a.updatedAt;
  });

  const lead = sortedTasks[0];
  const kind = lead.kind;
  const priority = getPriority(lead);
  const automationRuleId = lead.automationRuleId;
  const ruleName = automationRuleId ? ruleNameById[automationRuleId] : undefined;
  const maxTimestamp = sortedTasks.reduce((max, item) => Math.max(max, item.updatedAt), 0);

  const { title, body, tone, progress } = buildGroupCopy(kind, priority, sortedTasks, ruleName, t);

  const groupActions = actionRegistry.getLedgerTaskGroupActions
    ? actionRegistry.getLedgerTaskGroupActions(kind, priority, sortedTasks)
    : actionRegistry.getLedgerTaskActions(lead);

  return {
    id: getTaskGroupEntryId(kind, priority, automationRuleId),
    source: 'task',
    priority,
    tone,
    icon: getGroupIcon(kind, priority),
    title,
    body,
    timestamp: maxTimestamp,
    timestampLabel: formatNotificationTimestamp(maxTimestamp, now, t),
    progress,
    actions: groupActions.row,
    closeAction: groupActions.close,
    items: sortedTasks.map((task) => buildSubItem(task, t, actionRegistry)),
    itemsLabel: t('task_center.items_count', {
      count: sortedTasks.length,
      defaultValue: 'Files ({{count}})',
    }),
    itemClassName: 'notification-center-item-task notification-center-item-group',
  };
}

/**
 * Adapts TaskLedgerRecord[] from the ledger store into NotificationEntry[],
 * aggregating batch/automation/recovery records when there are 2 or more in
 * the same group and priority bucket.
 */
export function adaptTaskLedgerEntries(
  tasks: TaskLedgerRecord[],
  isRecoveryLoaded: boolean,
  t: TFunction,
  actionRegistry: TaskCenterActionRegistry,
  options?: AdaptTaskLedgerOptions
): NotificationEntry[] {
  const ruleNameById = options?.ruleNameById ?? {};
  const now = options?.now ?? Date.now();
  const staleQueueTaskIds = new Set(
    tasks.map(getStaleQueueTaskIdForRecovery).filter((id): id is string => Boolean(id))
  );

  const visibleTasks = tasks
    .filter((task) => isRecoveryLoaded || !shouldHideRecoveryRelatedTaskUntilLoaded(task))
    .filter((task) => !staleQueueTaskIds.has(task.id));

  const buckets = new Map<string, TaskLedgerRecord[]>();
  const singleEntries: NotificationEntry[] = [];

  for (const task of visibleTasks) {
    const key = getGroupBucketKey(task);
    if (!key) {
      singleEntries.push(buildSingleEntry(task, t, actionRegistry, ruleNameById, now));
      continue;
    }
    const existing = buckets.get(key);
    if (existing) {
      existing.push(task);
    } else {
      buckets.set(key, [task]);
    }
  }

  for (const groupTasks of buckets.values()) {
    if (groupTasks.length === 1) {
      singleEntries.push(buildSingleEntry(groupTasks[0], t, actionRegistry, ruleNameById, now));
    } else if (groupTasks.length > 1) {
      singleEntries.push(buildAggregatedEntry(groupTasks, t, actionRegistry, ruleNameById, now));
    }
  }

  return singleEntries;
}
