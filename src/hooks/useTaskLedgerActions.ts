import { useMemo } from 'react';
import type { UpdateStatus } from '../stores/appUpdaterStore';
import { useBatchQueueStore } from '../stores/batchQueueStore';
import { useRecoveryStore } from '../stores/recoveryStore';
import { useTaskLedgerStore } from '../stores/taskLedgerStore';
import type { TaskLedgerRecord } from '../types/taskLedger';
import {
  isTaskLedgerActionableStatus,
  isTaskLedgerActiveStatus,
} from '../types/taskLedger';
import { retryAutomationTaskFromLedger } from '../services/automationTaskRetryService';
import { retryLlmTaskFromLedger } from '../services/llmTaskRetryService';

export type TaskCenterActionId =
  | 'retry'
  | 'cancel'
  | 'resume'
  | 'discard'
  | 'openTarget'
  | 'dismiss'
  | 'clear'
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
  projectId?: string | null;
}

export interface TaskCenterActionDependencies {
  t: TaskCenterTranslate;
  requestTaskCancel: (id: string) => Promise<void>;
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
  getLedgerTaskActions: (task: TaskLedgerRecord) => TaskCenterAction[];
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

function createDismissTaskAction(
  deps: TaskCenterActionDependencies,
  task: TaskLedgerRecord,
): TaskCenterAction {
  return {
    id: 'dismiss',
    label: deps.t('task_center.dismiss', { defaultValue: 'Dismiss' }),
    variant: 'secondarySoft',
    run: () => deps.removeTask(task.id),
  };
}

function createClearTaskAction(
  deps: TaskCenterActionDependencies,
  task: TaskLedgerRecord,
): TaskCenterAction {
  return {
    id: 'clear',
    label: deps.t('task_center.clear', { defaultValue: 'Clear' }),
    variant: 'secondarySoft',
    run: () => deps.removeTask(task.id),
  };
}

function isLlmTask(task: TaskLedgerRecord): boolean {
  return task.kind === 'llmPolish' || task.kind === 'llmTranslate' || task.kind === 'llmSummary';
}

export function createTaskCenterActionRegistry(
  deps: TaskCenterActionDependencies,
): TaskCenterActionRegistry {
  return {
    getLedgerTaskActions: (task) => {
      if (task.kind === 'recovery' && task.status === 'recoverable') {
        const recoveryId = getRecoveryIdFromTask(task.id);
        return [
          {
            id: 'resume',
            label: deps.t('common.resume', { defaultValue: 'Resume' }),
            variant: 'primary',
            disabled: !task.recoverable,
            run: () => deps.resumeRecoveryItem(recoveryId),
          },
          {
            id: 'discard',
            label: deps.t('task_center.discard', { defaultValue: 'Discard' }),
            variant: 'secondarySoft',
            run: () => deps.discardRecoveryItem(recoveryId),
          },
          createOpenRecoveryAction(deps),
        ];
      }

      if (isTaskLedgerActiveStatus(task.status)) {
        return [
          {
            id: 'cancel',
            label: task.status === 'cancelRequested'
              ? deps.t('task_center.stopping', { defaultValue: 'Stopping' })
              : deps.t('common.cancel'),
            variant: 'secondarySoft',
            disabled: !task.cancelable || task.status === 'cancelRequested',
            run: () => deps.requestTaskCancel(task.id),
          },
        ];
      }

      if (isTaskLedgerActionableStatus(task.status)) {
        const canRetryAutomationFile = task.kind === 'automation' && Boolean(task.automationRuleId) && Boolean(task.filePath);
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
              deps.addBatchFiles([task.filePath as string], { projectId: task.projectId ?? null });
              await deps.removeTask(task.id);
            },
          });
        } else if (isLlmTask(task)) {
          actions.push({
            id: 'retry',
            label: deps.t('task_center.retry', { defaultValue: 'Retry' }),
            variant: 'primary',
            run: async () => {
              await deps.retryLlmTask(task);
              await deps.removeTask(task.id);
            },
          });
        } else if (task.kind === 'automation') {
          actions.push(createOpenAutomationAction(deps));
        }

        actions.push(createDismissTaskAction(deps, task));
        return actions;
      }

      return [createClearTaskAction(deps, task)];
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
        const label = status === 'downloading'
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
          id: 'dismiss',
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
          id: 'dismiss',
          label: deps.t('first_run.banner.dismiss_aria_label'),
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

  return useMemo(() => createTaskCenterActionRegistry({
    t,
    requestTaskCancel,
    removeTask,
    resumeRecoveryItem,
    discardRecoveryItem,
    retryAutomationTask: retryAutomationTaskFromLedger,
    addBatchFiles,
    retryLlmTask: retryLlmTaskFromLedger,
    installUpdate: updater.installUpdate,
    dismissUpdateNotification: updater.dismissNotification,
    relaunchToUpdate: updater.relaunchToUpdate,
    onOpenRecoveryCenter,
    onOpenAutomationSettings,
    closePanel,
    onboard,
  }), [
    addBatchFiles,
    closePanel,
    discardRecoveryItem,
    onOpenAutomationSettings,
    onOpenRecoveryCenter,
    removeTask,
    requestTaskCancel,
    resumeRecoveryItem,
    t,
    updater.dismissNotification,
    updater.installUpdate,
    updater.relaunchToUpdate,
    onboard,
  ]);
}
