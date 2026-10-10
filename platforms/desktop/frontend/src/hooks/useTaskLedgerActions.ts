import { useMemo } from 'react';
import { retryAutomationTaskFromLedger } from '../services/automationTaskRetryService';
import { retryLlmTaskFromLedger } from '../services/llmTaskRetryService';
import { createBatchTaskLedgerId } from '../services/taskLedgerBuilders';
import { cancelBatchTask } from '../services/tauri/recognizer';
import type { UpdateStatus } from '../stores/appUpdaterStore';
import { useAutomationStore } from '../stores/automationStore';
import { useBatchQueueStore } from '../stores/batchQueueStore';
import { useRecoveryStore } from '../stores/recoveryStore';
import { useTaskLedgerStore } from '../stores/taskLedgerStore';
import type { BatchQueueItem } from '../types/batchQueue';
import type { TaskLedgerRecord } from '../types/taskLedger';
import { isTaskLedgerActionableStatus, isTaskLedgerActiveStatus } from '../types/taskLedger';

export type TaskCenterActionId =
  | 'retry'
  | 'cancel'
  | 'resume'
  | 'discard'
  | 'openTarget'
  | 'dismiss'
  | 'clear'
  | 'close'
  | 'installUpdate'
  | 'relaunchUpdate'
  | 'onboard';

export type TaskCenterActionVariant = 'primary' | 'secondary' | 'secondarySoft';

export interface TaskCenterAction {
  id: TaskCenterActionId;
  label: string;
  variant: TaskCenterActionVariant;
  disabled?: boolean;
  run: () => void | Promise<void>;
}

export interface TaskCenterResolvedActions {
  row: TaskCenterAction[];
  close?: TaskCenterAction;
  open?: TaskCenterAction;
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

export interface TaskCenterActionDependencies {
  t: TaskCenterTranslate;
  requestTaskCancel: (id: string) => Promise<void>;
  cancelBatchTask?: (instanceId: string) => Promise<void>;
  getBatchQueueItems?: () => BatchQueueItem[];
  removeTask: (id: string) => Promise<void>;
  resumeRecoveryItem: (id: string) => Promise<void>;
  discardRecoveryItem: (id: string) => Promise<void>;
  removeBatchQueueItem?: (id: string) => void;
  dismissAutomationNotification?: (id: string) => void;
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

function getRecoveryIdFromTask(taskId: string): string {
  return taskId.startsWith('recovery-') ? taskId.slice('recovery-'.length) : taskId;
}
async function closeLedgerTask(
  deps: TaskCenterActionDependencies,
  task: TaskLedgerRecord
): Promise<void> {
  if (task.kind === 'recovery' || task.id.startsWith('recovery-')) {
    const recoveryId = getRecoveryIdFromTask(task.id);
    await deps.discardRecoveryItem(recoveryId);
    return;
  }
  if (task.kind === 'automation' || task.id.startsWith('automation-')) {
    if (task.id.startsWith('batch-')) {
      deps.removeBatchQueueItem?.(task.id.slice(6));
    }
    if (task.automationRuleId) {
      deps.dismissAutomationNotification?.(`automation-failure-${task.automationRuleId}`);
    }
    await deps.removeTask(task.id);
    return;
  }

  if (task.kind === 'batchImport' || task.id.startsWith('batch-')) {
    const queueId = task.id.startsWith('batch-') ? task.id.slice(6) : task.id;
    deps.removeBatchQueueItem?.(queueId);
    await deps.removeTask(task.id);
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
    variant: 'secondarySoft',
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

export function createTaskCenterActionRegistry(
  deps: TaskCenterActionDependencies
): TaskCenterActionRegistry {
  return {
    getLedgerTaskActions: (task) => {
      if (task.kind === 'recovery' && task.status === 'recoverable') {
        const recoveryId = getRecoveryIdFromTask(task.id);
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
              variant: 'secondarySoft',
              disabled: !task.cancelable || task.status === 'cancelRequested',
              run: async () => {
                if (task.kind === 'batchImport' || task.kind === 'automation') {
                  const items = deps.getBatchQueueItems?.() ?? [];
                  const queueItem = items.find(
                    (item) => createBatchTaskLedgerId(item.id) === task.id || item.id === task.id
                  );
                  if (queueItem) {
                    if (queueItem.activeInstanceId) {
                      void (deps.cancelBatchTask ?? cancelBatchTask)(queueItem.activeInstanceId);
                    }
                  } else if (!task.id.startsWith('batch-')) {
                    void (deps.cancelBatchTask ?? cancelBatchTask)(task.id);
                  }
                }
                await deps.requestTaskCancel(task.id);
              },
            },
          ],
          close: undefined,
        };
      }

      if (isTaskLedgerActionableStatus(task.status)) {
        const canRetryAutomationFile =
          task.kind === 'automation' && Boolean(task.automationRuleId) && Boolean(task.filePath);
        const canRetryBatch = task.kind === 'batchImport' && Boolean(task.filePath);
        const actions: TaskCenterAction[] = [];

        if (canRetryAutomationFile) {
          actions.push({
            id: 'retry',
            label: deps.t('task_center.retry', { defaultValue: 'Retry' }),
            variant: 'primary',
            run: async () => {
              await deps.retryAutomationTask(task);
              await deps.removeTask(task.id);
            },
          });
        } else if (canRetryBatch && task.filePath) {
          actions.push({
            id: 'retry',
            label: deps.t('task_center.retry', { defaultValue: 'Retry' }),
            variant: 'primary',
            run: async () => {
              deps.addBatchFiles([task.filePath as string], {
                tagIds: task.tagIds ?? (task.projectId ? [task.projectId] : []),
              });
              await deps.removeTask(task.id);
            },
          });
        } else if (isLlmTask(task)) {
          if (task.retryable !== false) {
            actions.push({
              id: 'retry',
              label: deps.t('task_center.retry', { defaultValue: 'Retry' }),
              variant: 'primary',
              run: async () => {
                await deps.retryLlmTask(task);
                await deps.removeTask(task.id);
              },
            });
          }
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
          variant: 'secondarySoft',
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
          variant: 'secondarySoft',
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
  const queueItems = useBatchQueueStore((state) => state.queueItems);
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
            : queueItems) ?? [],
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
      queueItems,
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
