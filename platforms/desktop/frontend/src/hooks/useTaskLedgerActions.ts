import { useMemo } from 'react';
import { retryAutomationTaskFromLedger } from '../services/automationTaskRetryService';
import { retryLlmTaskFromLedger } from '../services/llmTaskRetryService';
import {
  createBatchTaskLedgerId,
  getAutomationFailureNotificationId,
  getRecoveryIdForTask,
  parseBatchQueueIdFromTaskId,
} from '../services/taskLedgerBuilders';
import { cancelBatchTask } from '../services/tauri/recognizer';
import type { UpdateStatus } from '../stores/appUpdaterStore';
import { useAutomationStore } from '../stores/automationStore';
import { useBatchQueueStore } from '../stores/batchQueueStore';
import { useRecoveryStore } from '../stores/recoveryStore';
import { useTaskLedgerStore } from '../stores/taskLedgerStore';
import type { BatchQueueItem } from '../types/batchQueue';
import type {
  NotificationAction,
  NotificationActionVariant,
  NotificationPriority,
} from '../types/notification';
import type { TaskLedgerKind, TaskLedgerRecord } from '../types/taskLedger';
import { isTaskLedgerActionableStatus, isTaskLedgerActiveStatus } from '../types/taskLedger';

export type TaskCenterActionId =
  | 'retry'
  | 'retryAll'
  | 'cancel'
  | 'cancelAll'
  | 'resume'
  | 'resumeAll'
  | 'discard'
  | 'openTarget'
  | 'dismiss'
  | 'clear'
  | 'close'
  | 'installUpdate'
  | 'relaunchUpdate'
  | 'onboard';

export interface TaskCenterAction extends NotificationAction {
  id: TaskCenterActionId;
  variant: NotificationActionVariant;
}

export interface TaskCenterResolvedActions {
  row: TaskCenterAction[];
  close?: TaskCenterAction;
}

export interface TaskCenterUpdateActionInput {
  status: UpdateStatus;
  isBusy: boolean;
}

type TaskCenterTranslate = (key: string, options?: Record<string, unknown>) => string;

interface BatchAddOptions {
  tagIds?: string[];
  /** @deprecated */
  projectId?: string | null;
}

export interface LedgerTaskCleanupPorts {
  discardRecoveryItem?: (id: string) => Promise<void> | void;
  removeBatchQueueItem?: (id: string) => void;
  dismissAutomationNotification?: (id: string) => void;
}

export interface TaskCenterActionDependencies extends LedgerTaskCleanupPorts {
  t: TaskCenterTranslate;
  requestTaskCancel: (id: string) => Promise<void>;
  cancelBatchTask?: (instanceId: string) => Promise<void>;
  getBatchQueueItems?: () => BatchQueueItem[];
  removeTask: (id: string) => Promise<void>;
  resumeRecoveryItem: (id: string) => Promise<void>;
  discardRecoveryItem: (id: string) => Promise<void>;
  retryAutomationTask: (task: TaskLedgerRecord) => Promise<void>;
  addBatchFiles: (filePaths: string[], options?: BatchAddOptions) => void;
  retryLlmTask: (task: TaskLedgerRecord) => Promise<void>;
  installUpdate: () => Promise<void>;
  dismissUpdateNotification: () => void;
  relaunchToUpdate: () => Promise<void>;
  onOpenRecoveryCenter: () => void;
  onOpenAutomationSettings: () => void;
  closePanel: () => void;
  onboard: {
    reopen: () => void;
    dismiss: () => void;
  };
}

export interface TaskCenterActionRegistry {
  getLedgerTaskActions: (task: TaskLedgerRecord) => TaskCenterResolvedActions;
  getLedgerTaskGroupActions?: (
    kind: TaskLedgerKind,
    priority: NotificationPriority,
    tasks: TaskLedgerRecord[]
  ) => TaskCenterResolvedActions;
  getUpdateTaskActions: (entry: TaskCenterUpdateActionInput) => TaskCenterResolvedActions;
  getOnboardingReminderActions: () => TaskCenterResolvedActions;
}

export interface UseTaskLedgerActionsInput {
  t: TaskCenterTranslate;
  onOpenRecoveryCenter: () => void;
  onOpenAutomationSettings: () => void;
  closePanel: () => void;
  updater: {
    installUpdate: () => Promise<void>;
    dismissNotification: () => void;
    relaunchToUpdate: () => Promise<void>;
  };
  onboard: {
    reopen: () => void;
    dismiss: () => void;
  };
}

/**
 * Cleans up side-effect stores (recovery, batch queue, automation notices) associated
 * with a ledger task. Returns a promise when recovery discarding handles ledger removal,
 * or undefined when the caller should remove the ledger record directly.
 */
export function cleanupLedgerTaskSideEffects(
  task: TaskLedgerRecord,
  ports: LedgerTaskCleanupPorts
): Promise<void> | undefined {
  if (task.kind === 'recovery' || task.id.startsWith('recovery-')) {
    const recoveryId = getRecoveryIdForTask(task.id);
    return Promise.resolve(ports.discardRecoveryItem?.(recoveryId));
  }

  const batchQueueId = parseBatchQueueIdFromTaskId(task.id);

  if (task.kind === 'automation' || task.id.startsWith('automation-')) {
    if (batchQueueId) {
      ports.removeBatchQueueItem?.(batchQueueId);
    }
    if (task.automationRuleId) {
      ports.dismissAutomationNotification?.(
        getAutomationFailureNotificationId(task.automationRuleId)
      );
    }
    return undefined;
  }

  if (task.kind === 'batchImport' || batchQueueId !== null) {
    ports.removeBatchQueueItem?.(batchQueueId ?? task.id);
  }
  return undefined;
}

async function closeLedgerTask(
  deps: TaskCenterActionDependencies,
  task: TaskLedgerRecord
): Promise<void> {
  const recoveryCleanup = cleanupLedgerTaskSideEffects(task, deps);
  if (recoveryCleanup) {
    await recoveryCleanup;
    return;
  }
  await deps.removeTask(task.id);
}

function createCloseTaskAction(
  deps: TaskCenterActionDependencies,
  task: TaskLedgerRecord
): TaskCenterAction {
  return {
    id: 'close',
    label: deps.t('common.close', { defaultValue: 'Close' }),
    variant: 'soft',
    run: () => closeLedgerTask(deps, task),
  };
}

function createOpenRecoveryAction(deps: TaskCenterActionDependencies): TaskCenterAction {
  return {
    id: 'openTarget',
    label: deps.t('recovery.actions.open_center'),
    variant: 'secondary',
    run: () => {
      deps.closePanel();
      deps.onOpenRecoveryCenter();
    },
  };
}

function createOpenAutomationAction(deps: TaskCenterActionDependencies): TaskCenterAction {
  return {
    id: 'openTarget',
    label: deps.t('automation.open_settings', { defaultValue: 'Open Automation' }),
    variant: 'secondary',
    run: () => {
      deps.closePanel();
      deps.onOpenAutomationSettings();
    },
  };
}

function isLlmTask(task: TaskLedgerRecord): boolean {
  return task.kind === 'llmPolish' || task.kind === 'llmTranslate' || task.kind === 'llmSummary';
}

function isTaskRetryable(task: TaskLedgerRecord): boolean {
  if (task.kind === 'automation') {
    return Boolean(task.automationRuleId && task.filePath);
  }
  if (task.kind === 'batchImport') {
    return Boolean(task.filePath);
  }
  if (isLlmTask(task)) {
    return task.retryable !== false;
  }
  return false;
}

async function cancelSingleTask(
  deps: TaskCenterActionDependencies,
  task: TaskLedgerRecord
): Promise<void> {
  if (task.kind === 'batchImport' || task.kind === 'automation') {
    const items = deps.getBatchQueueItems?.() ?? [];
    const queueItem = items.find(
      (item) => createBatchTaskLedgerId(item.id) === task.id || item.id === task.id
    );
    if (queueItem) {
      if (queueItem.activeInstanceId) {
        void (deps.cancelBatchTask ?? cancelBatchTask)(queueItem.activeInstanceId);
      }
    } else if (!parseBatchQueueIdFromTaskId(task.id)) {
      void (deps.cancelBatchTask ?? cancelBatchTask)(task.id);
    }
  }
  await deps.requestTaskCancel(task.id);
}

async function retrySingleTask(
  deps: TaskCenterActionDependencies,
  task: TaskLedgerRecord
): Promise<void> {
  if (task.kind === 'automation' && task.automationRuleId && task.filePath) {
    await deps.retryAutomationTask(task);
    await deps.removeTask(task.id);
    return;
  }
  if (task.kind === 'batchImport' && task.filePath) {
    deps.addBatchFiles([task.filePath], {
      projectId: task.projectId ?? task.tagIds?.[0] ?? null,
    });
    await deps.removeTask(task.id);
    return;
  }
  if (isLlmTask(task) && task.retryable !== false) {
    await deps.retryLlmTask(task);
    await deps.removeTask(task.id);
  }
}

async function runSequentiallyBestEffort(
  tasks: TaskLedgerRecord[],
  runner: (task: TaskLedgerRecord) => Promise<void>
): Promise<void> {
  for (const task of tasks) {
    try {
      await runner(task);
    } catch {
      // Individual task errors are logged/recorded by the underlying task service or store.
    }
  }
}

async function closeTaskGroup(
  deps: TaskCenterActionDependencies,
  tasks: TaskLedgerRecord[]
): Promise<void> {
  await runSequentiallyBestEffort(tasks, (task) => closeLedgerTask(deps, task));
}

export function createTaskCenterActionRegistry(
  deps: TaskCenterActionDependencies
): TaskCenterActionRegistry {
  return {
    getLedgerTaskActions: (task) => {
      if (task.kind === 'recovery' && task.status === 'recoverable') {
        const recoveryId = getRecoveryIdForTask(task.id);
        return {
          row: [
            {
              id: 'resume',
              label: deps.t('common.resume', { defaultValue: 'Resume' }),
              variant: 'primary',
              disabled: !task.recoverable,
              run: () => deps.resumeRecoveryItem(recoveryId),
            },
            createOpenRecoveryAction(deps),
          ],
          close: createCloseTaskAction(deps, task),
        };
      }

      if (isTaskLedgerActiveStatus(task.status)) {
        return {
          row: [
            {
              id: 'cancel',
              label:
                task.status === 'cancelRequested'
                  ? deps.t('task_center.stopping', { defaultValue: 'Stopping' })
                  : deps.t('common.cancel'),
              variant: 'soft',
              disabled: !task.cancelable || task.status === 'cancelRequested',
              run: () => cancelSingleTask(deps, task),
            },
          ],
          close: undefined,
        };
      }

      if (isTaskLedgerActionableStatus(task.status)) {
        const actions: TaskCenterAction[] = [];

        if (isTaskRetryable(task)) {
          actions.push({
            id: 'retry',
            label: deps.t('task_center.retry', { defaultValue: 'Retry' }),
            variant: 'primary',
            run: () => retrySingleTask(deps, task),
          });
        } else if (task.kind === 'automation') {
          actions.push(createOpenAutomationAction(deps));
        }

        return {
          row: actions,
          close: createCloseTaskAction(deps, task),
        };
      }

      return {
        row: [],
        close: createCloseTaskAction(deps, task),
      };
    },

    getLedgerTaskGroupActions: (kind, priority, tasks) => {
      if (kind === 'recovery' && priority === 'action') {
        const recoverableTasks = tasks.filter(
          (task) => task.kind === 'recovery' && task.status === 'recoverable' && task.recoverable
        );
        return {
          row: [
            {
              id: 'resumeAll',
              label: deps.t('task_center.resume_all', { defaultValue: 'Resume All' }),
              variant: 'primary',
              disabled: recoverableTasks.length === 0,
              run: () =>
                runSequentiallyBestEffort(recoverableTasks, (task) =>
                  deps.resumeRecoveryItem(getRecoveryIdForTask(task.id))
                ),
            },
            createOpenRecoveryAction(deps),
          ],
          close: {
            id: 'close',
            label: deps.t('common.close', { defaultValue: 'Close' }),
            variant: 'soft',
            run: () => closeTaskGroup(deps, tasks),
          },
        };
      }

      if (priority === 'active') {
        const cancelableTasks = tasks.filter(
          (task) => task.cancelable && task.status !== 'cancelRequested'
        );
        return {
          row: [
            {
              id: 'cancelAll',
              label:
                cancelableTasks.length === 0
                  ? deps.t('task_center.stopping', { defaultValue: 'Stopping' })
                  : deps.t('task_center.cancel_all', { defaultValue: 'Cancel All' }),
              variant: 'soft',
              disabled: cancelableTasks.length === 0,
              run: () =>
                runSequentiallyBestEffort(cancelableTasks, (task) => cancelSingleTask(deps, task)),
            },
          ],
          close: undefined,
        };
      }

      if (priority === 'action') {
        const retryableTasks = tasks.filter(isTaskRetryable);
        const hasRuleLevelAutomationFailure =
          kind === 'automation' &&
          tasks.some((task) => task.kind === 'automation' && !isTaskRetryable(task));
        const actions: TaskCenterAction[] = [];

        if (retryableTasks.length > 0) {
          actions.push({
            id: 'retryAll',
            label: deps.t('task_center.retry_all', { defaultValue: 'Retry All' }),
            variant: 'primary',
            run: () =>
              runSequentiallyBestEffort(retryableTasks, (task) => retrySingleTask(deps, task)),
          });
        }
        if (hasRuleLevelAutomationFailure) {
          actions.push(createOpenAutomationAction(deps));
        }

        return {
          row: actions,
          close: {
            id: 'close',
            label: deps.t('common.close', { defaultValue: 'Close' }),
            variant: 'soft',
            run: () => closeTaskGroup(deps, tasks),
          },
        };
      }

      return {
        row: [],
        close: {
          id: 'close',
          label: deps.t('common.close', { defaultValue: 'Close' }),
          variant: 'soft',
          run: () => closeTaskGroup(deps, tasks),
        },
      };
    },

    getUpdateTaskActions: ({ status, isBusy }) => {
      let rowAction: TaskCenterAction;
      if (status === 'downloaded') {
        rowAction = {
          id: 'relaunchUpdate',
          label: deps.t('settings.update_btn_relaunch'),
          variant: 'primary',
          disabled: isBusy,
          run: () => deps.relaunchToUpdate(),
        };
      } else {
        const label =
          status === 'downloading'
            ? deps.t('settings.update_downloading')
            : status === 'installing'
              ? deps.t('settings.update_installing')
              : deps.t('settings.update_btn_install');

        rowAction = {
          id: 'installUpdate',
          label,
          variant: 'primary',
          disabled: isBusy,
          run: () => deps.installUpdate(),
        };
      }

      return {
        row: [rowAction],
        close: {
          id: 'close',
          label: deps.t('common.close'),
          variant: 'soft',
          disabled: isBusy,
          run: deps.dismissUpdateNotification,
        },
      };
    },

    getOnboardingReminderActions: () => {
      return {
        row: [
          {
            id: 'onboard',
            label: deps.t('first_run.banner.cta'),
            variant: 'primary',
            run: () => {
              deps.closePanel();
              deps.onboard.reopen();
            },
          },
        ],
        close: {
          id: 'close',
          label: deps.t('common.close'),
          variant: 'soft',
          run: deps.onboard.dismiss,
        },
      };
    },
  };
}

export function useTaskLedgerActions({
  t,
  onOpenRecoveryCenter,
  onOpenAutomationSettings,
  closePanel,
  updater,
  onboard,
}: UseTaskLedgerActionsInput): TaskCenterActionRegistry {
  const requestTaskCancel = useTaskLedgerStore((state) => state.requestCancel);
  const removeTask = useTaskLedgerStore((state) => state.removeTask);
  const resumeRecoveryItem = useRecoveryStore((state) => state.resumeItem);
  const discardRecoveryItem = useRecoveryStore((state) => state.discardItem);
  const addBatchFiles = useBatchQueueStore((state) => state.addFiles);
  const removeBatchQueueItem = useBatchQueueStore((state) => state.removeItem);
  const dismissAutomationNotification = useAutomationStore((state) => state.dismissNotification);

  return useMemo(
    () =>
      createTaskCenterActionRegistry({
        t,
        requestTaskCancel,
        removeTask,
        resumeRecoveryItem,
        discardRecoveryItem,
        removeBatchQueueItem,
        dismissAutomationNotification,
        retryAutomationTask: retryAutomationTaskFromLedger,
        addBatchFiles,
        retryLlmTask: retryLlmTaskFromLedger,
        installUpdate: updater.installUpdate,
        cancelBatchTask,
        getBatchQueueItems: () =>
          (typeof useBatchQueueStore.getState === 'function'
            ? useBatchQueueStore.getState().queueItems
            : []) ?? [],
        dismissUpdateNotification: updater.dismissNotification,
        relaunchToUpdate: updater.relaunchToUpdate,
        onOpenRecoveryCenter,
        onOpenAutomationSettings,
        closePanel,
        onboard,
      }),
    [
      addBatchFiles,
      closePanel,
      discardRecoveryItem,
      dismissAutomationNotification,
      onOpenAutomationSettings,
      onOpenRecoveryCenter,
      removeBatchQueueItem,
      removeTask,
      requestTaskCancel,
      resumeRecoveryItem,
      t,
      updater.dismissNotification,
      updater.installUpdate,
      updater.relaunchToUpdate,
      onboard,
    ]
  );
}
