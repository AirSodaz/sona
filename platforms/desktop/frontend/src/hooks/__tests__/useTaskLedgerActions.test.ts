import { describe, expect, it, vi } from 'vitest';
import type { TaskLedgerRecord } from '../../types/taskLedger';
import {
  createTaskCenterActionRegistry,
  type TaskCenterAction,
  type TaskCenterActionDependencies,
} from '../useTaskLedgerActions';

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
  overrides: Partial<TaskCenterActionDependencies> = {}
): TaskCenterActionDependencies {
  return {
    t: translate,
    requestTaskCancel: vi.fn().mockResolvedValue(undefined),
    removeTask: vi.fn().mockResolvedValue(undefined),
    resumeRecoveryItem: vi.fn().mockResolvedValue(undefined),
    discardRecoveryItem: vi.fn().mockResolvedValue(undefined),
    retryAutomationTask: vi.fn().mockResolvedValue(undefined),
    addBatchFiles: vi.fn(),
    retryLlmTask: vi.fn().mockResolvedValue(undefined),
    installUpdate: vi.fn().mockResolvedValue(undefined),
    dismissUpdateNotification: vi.fn(),
    relaunchToUpdate: vi.fn().mockResolvedValue(undefined),
    onOpenRecoveryCenter: vi.fn(),
    onOpenAutomationSettings: vi.fn(),
    closePanel: vi.fn(),
    onboard: {
      reopen: vi.fn(),
      dismiss: vi.fn(),
    },
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
      tagIds: ['project-1'],
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

  it('retries failed automation file tasks and clears the old ledger record', async () => {
    const deps = makeDeps();
    const registry = createTaskCenterActionRegistry(deps);
    const task = makeTask({
      id: 'automation-failed-file',
      kind: 'automation',
      status: 'failed',
      title: 'failed.wav',
      automationRuleId: 'rule-1',
      filePath: 'C:\\watch\\failed.wav',
    });

    const actions = registry.getLedgerTaskActions(task);

    expect(actions.map((action) => action.id)).toEqual(['retry', 'dismiss']);

    await getAction(actions, 'retry').run();

    expect(deps.retryAutomationTask).toHaveBeenCalledWith(task);
    expect(deps.removeTask).toHaveBeenCalledWith('automation-failed-file');
  });

  it('keeps failed automation file tasks when retry preflight fails', async () => {
    const deps = makeDeps({
      retryAutomationTask: vi
        .fn()
        .mockRejectedValue(new Error('Source file is no longer available for retry.')),
    });
    const registry = createTaskCenterActionRegistry(deps);

    await expect(
      getAction(
        registry.getLedgerTaskActions(
          makeTask({
            id: 'automation-failed-file',
            kind: 'automation',
            status: 'failed',
            automationRuleId: 'rule-1',
            filePath: 'C:\\watch\\failed.wav',
          })
        ),
        'retry'
      ).run()
    ).rejects.toThrow('Source file is no longer available for retry.');

    expect(deps.removeTask).not.toHaveBeenCalled();
  });

  it('opens automation settings for rule-level automation failures without a retry file', async () => {
    const deps = makeDeps();
    const registry = createTaskCenterActionRegistry(deps);

    const actions = registry.getLedgerTaskActions(
      makeTask({
        id: 'automation-rule-failed',
        kind: 'automation',
        status: 'failed',
        title: 'Meeting Inbox',
        filePath: undefined,
        automationRuleId: 'rule-1',
      })
    );

    expect(actions.map((action) => action.id)).toEqual(['openTarget', 'dismiss']);

    await getAction(actions, 'openTarget').run();

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
      retryLlmTask: vi
        .fn()
        .mockRejectedValue(new Error('Transcript is no longer available for retry.')),
    });
    const registry = createTaskCenterActionRegistry(deps);

    await expect(
      getAction(
        registry.getLedgerTaskActions(
          makeTask({
            id: 'llm-failed',
            kind: 'llmTranslate',
            status: 'failed',
            filePath: undefined,
          })
        ),
        'retry'
      ).run()
    ).rejects.toThrow('Transcript is no longer available for retry.');

    expect(deps.removeTask).not.toHaveBeenCalled();
  });
  it('cancels active batch task and calls cancelBatchTask when activeInstanceId exists', async () => {
    const cancelBatchTask = vi.fn().mockResolvedValue(undefined);
    const deps = makeDeps({
      cancelBatchTask,
      getBatchQueueItems: () => [
        {
          id: 'item-123',
          filename: 'recording.mp3',
          filePath: '/recording.mp3',
          status: 'processing',
          progress: 50,
          segments: [],
          projectId: null,
          activeInstanceId: 'inst-abc',
        },
      ],
    });
    const registry = createTaskCenterActionRegistry(deps);

    const task = makeTask({
      id: 'batch-item-123',
      kind: 'batchImport',
      status: 'running',
      cancelable: true,
    });

    const actions = registry.getLedgerTaskActions(task);
    expect(actions.map((a) => a.id)).toEqual(['cancel']);

    await getAction(actions, 'cancel').run();

    expect(cancelBatchTask).toHaveBeenCalledWith('inst-abc');
    expect(deps.requestTaskCancel).toHaveBeenCalledWith('batch-item-123');
  });

  it('cancels active batch task without calling cancelBatchTask when no activeInstanceId exists', async () => {
    const cancelBatchTask = vi.fn().mockResolvedValue(undefined);
    const deps = makeDeps({
      cancelBatchTask,
      getBatchQueueItems: () => [],
    });
    const registry = createTaskCenterActionRegistry(deps);

    const task = makeTask({
      id: 'batch-pending-item',
      kind: 'batchImport',
      status: 'running',
      cancelable: true,
    });

    const actions = registry.getLedgerTaskActions(task);
    await getAction(actions, 'cancel').run();

    expect(cancelBatchTask).not.toHaveBeenCalled();
    expect(deps.requestTaskCancel).toHaveBeenCalledWith('batch-pending-item');
  });

  it('cancels active LLM task without calling cancelBatchTask', async () => {
    const cancelBatchTask = vi.fn().mockResolvedValue(undefined);
    const deps = makeDeps({
      cancelBatchTask,
    });
    const registry = createTaskCenterActionRegistry(deps);

    const task = makeTask({
      id: 'llm-running',
      kind: 'llmSummary',
      status: 'running',
      cancelable: true,
    });

    const actions = registry.getLedgerTaskActions(task);
    await getAction(actions, 'cancel').run();

    expect(cancelBatchTask).not.toHaveBeenCalled();
    expect(deps.requestTaskCancel).toHaveBeenCalledWith('llm-running');
  });

  it('maps onboarding reminder state to onboard and dismiss actions', () => {
    const deps = makeDeps();
    const registry = createTaskCenterActionRegistry(deps);

    const actions = registry.getOnboardingReminderActions();
    expect(actions.row.map((action) => action.id)).toEqual(['onboard']);
    expect(actions.close?.id).toBe('dismiss');

    getAction(actions.row, 'onboard').run();
    expect(deps.closePanel).toHaveBeenCalled();
    expect(deps.onboard.reopen).toHaveBeenCalled();

    actions.close?.run();
    expect(deps.onboard.dismiss).toHaveBeenCalled();
  });
});
