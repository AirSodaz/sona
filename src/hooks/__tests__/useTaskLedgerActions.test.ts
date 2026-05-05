import { describe, expect, it, vi } from 'vitest';
import {
  createTaskCenterActionRegistry,
  type TaskCenterAction,
  type TaskCenterActionDependencies,
} from '../useTaskLedgerActions';
import type { TaskLedgerRecord } from '../../types/taskLedger';

function translate(key: string, options?: Record<string, unknown>): string {
  if (key === 'task_center.retry') return 'Retry';
  if (key === 'task_center.dismiss') return 'Dismiss';
  if (key === 'task_center.clear') return 'Clear';
  if (key === 'task_center.discard') return 'Discard';
  if (key === 'task_center.stopping') return 'Stopping';
  if (key === 'common.cancel') return 'Cancel';
  if (key === 'common.resume') return 'Resume';
  if (key === 'common.close') return 'Close';
  if (key === 'recovery.actions.open_center') return 'Open Recovery Center';
  if (key === 'automation.retry_failed') return 'Retry Failed';
  if (key === 'automation.open_settings') return 'Open Automation';
  if (key === 'settings.update_btn_install') return 'Install Update';
  if (key === 'settings.update_btn_relaunch') return 'Relaunch';
  if (key === 'settings.update_downloading') return 'Downloading update...';
  if (key === 'settings.update_installing') return 'Installing update...';
  return String(options?.defaultValue ?? key);
}

function makeDeps(
  overrides: Partial<TaskCenterActionDependencies> = {},
): TaskCenterActionDependencies {
  return {
    t: translate,
    requestTaskCancel: vi.fn().mockResolvedValue(undefined),
    removeTask: vi.fn().mockResolvedValue(undefined),
    resumeRecoveryItem: vi.fn().mockResolvedValue(undefined),
    discardRecoveryItem: vi.fn().mockResolvedValue(undefined),
    retryAutomationRule: vi.fn().mockResolvedValue(undefined),
    retryAutomationNotification: vi.fn().mockResolvedValue(undefined),
    dismissAutomationNotification: vi.fn(),
    addBatchFiles: vi.fn(),
    retryLlmTask: vi.fn().mockResolvedValue(undefined),
    installUpdate: vi.fn().mockResolvedValue(undefined),
    dismissUpdateNotification: vi.fn(),
    relaunchToUpdate: vi.fn().mockResolvedValue(undefined),
    onOpenRecoveryCenter: vi.fn(),
    onOpenAutomationSettings: vi.fn(),
    closePanel: vi.fn(),
    ...overrides,
  };
}

function makeTask(overrides: Partial<TaskLedgerRecord> = {}): TaskLedgerRecord {
  return {
    id: 'task-1',
    kind: 'batchImport',
    status: 'failed',
    title: 'meeting.wav',
    progress: 0,
    createdAt: 100,
    updatedAt: 100,
    retryable: true,
    cancelable: false,
    recoverable: false,
    filePath: 'C:\\audio\\meeting.wav',
    projectId: 'project-1',
    ...overrides,
  };
}

function getAction(actions: TaskCenterAction[], id: TaskCenterAction['id']): TaskCenterAction {
  const action = actions.find((item) => item.id === id);
  expect(action).toBeDefined();
  return action as TaskCenterAction;
}

describe('createTaskCenterActionRegistry', () => {
  it('retries failed batch tasks through the batch queue and clears the ledger record', async () => {
    const deps = makeDeps();
    const registry = createTaskCenterActionRegistry(deps);

    const actions = registry.getLedgerTaskActions(makeTask());

    expect(actions.map((action) => action.id)).toEqual(['retry', 'dismiss']);

    await getAction(actions, 'retry').run();

    expect(deps.addBatchFiles).toHaveBeenCalledWith(['C:\\audio\\meeting.wav'], {
      projectId: 'project-1',
    });
    expect(deps.removeTask).toHaveBeenCalledWith('task-1');
  });

  it('maps update states to install, busy, relaunch, and dismiss actions', async () => {
    const deps = makeDeps();
    const registry = createTaskCenterActionRegistry(deps);

    const available = registry.getUpdateTaskActions({ status: 'available', isBusy: false });
    expect(available.row.map((action) => action.id)).toEqual(['installUpdate']);
    expect(available.close?.id).toBe('dismiss');
    await getAction(available.row, 'installUpdate').run();
    available.close?.run();
    expect(deps.installUpdate).toHaveBeenCalled();
    expect(deps.dismissUpdateNotification).toHaveBeenCalled();

    const busy = registry.getUpdateTaskActions({ status: 'downloading', isBusy: true });
    expect(busy.row).toMatchObject([
      { id: 'installUpdate', label: 'Downloading update...', disabled: true },
    ]);
    expect(busy.close).toMatchObject({ id: 'dismiss', disabled: true });

    const downloaded = registry.getUpdateTaskActions({ status: 'downloaded', isBusy: false });
    expect(downloaded.row.map((action) => action.id)).toEqual(['relaunchUpdate']);
    await getAction(downloaded.row, 'relaunchUpdate').run();
    expect(deps.relaunchToUpdate).toHaveBeenCalled();
  });

  it('routes automation notification actions through the automation store and open target callback', async () => {
    const deps = makeDeps();
    const registry = createTaskCenterActionRegistry(deps);

    const retryable = registry.getAutomationNotificationActions({
      notificationId: 'automation-failure-rule-1',
      kind: 'automationFailure',
      retryable: true,
    });
    expect(retryable.row.map((action) => action.id)).toEqual(['retry']);
    expect(retryable.close?.id).toBe('dismiss');
    await getAction(retryable.row, 'retry').run();
    retryable.close?.run();
    expect(deps.retryAutomationNotification).toHaveBeenCalledWith('automation-failure-rule-1');
    expect(deps.dismissAutomationNotification).toHaveBeenCalledWith('automation-failure-rule-1');

    const success = registry.getAutomationNotificationActions({
      notificationId: 'automation-success-rule-1',
      kind: 'automationSuccess',
      retryable: false,
    });
    expect(success.row.map((action) => action.id)).toEqual(['openTarget']);
    await getAction(success.row, 'openTarget').run();
    expect(deps.closePanel).toHaveBeenCalled();
    expect(deps.onOpenAutomationSettings).toHaveBeenCalled();
  });

  it('retries failed LLM ledger tasks through the LLM retry service and clears the old task', async () => {
    const deps = makeDeps();
    const registry = createTaskCenterActionRegistry(deps);
    const task = makeTask({
      id: 'llm-failed',
      kind: 'llmSummary',
      status: 'failed',
      filePath: undefined,
      templateId: 'meeting',
    });

    const actions = registry.getLedgerTaskActions(task);

    expect(actions.map((action) => action.id)).toEqual(['retry', 'dismiss']);

    await getAction(actions, 'retry').run();

    expect(deps.retryLlmTask).toHaveBeenCalledWith(task);
    expect(deps.removeTask).toHaveBeenCalledWith('llm-failed');
  });

  it('keeps failed LLM ledger tasks when retry preflight fails', async () => {
    const deps = makeDeps({
      retryLlmTask: vi.fn().mockRejectedValue(new Error('Transcript is no longer available for retry.')),
    });
    const registry = createTaskCenterActionRegistry(deps);

    await expect(getAction(registry.getLedgerTaskActions(makeTask({
      id: 'llm-failed',
      kind: 'llmTranslate',
      status: 'failed',
      filePath: undefined,
    })), 'retry').run()).rejects.toThrow('Transcript is no longer available for retry.');

    expect(deps.removeTask).not.toHaveBeenCalled();
  });
});
