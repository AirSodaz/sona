import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { NotificationCenter } from '../NotificationCenter';
import { useAppUpdaterStore } from '../../stores/appUpdaterStore';
import type { TaskLedgerRecord } from '../../types/taskLedger';

const taskLedgerState = {
  tasks: [] as TaskLedgerRecord[],
  requestCancel: vi.fn(),
  removeTask: vi.fn(),
  clearResolved: vi.fn(),
};

const recoveryState = {
  resumeItem: vi.fn(),
  discardItem: vi.fn(),
};

const automationState = {
  notifications: [] as any[],
  dismissNotification: vi.fn(),
  retryNotification: vi.fn(),
  retryFailed: vi.fn(),
};

const batchQueueState = {
  addFiles: vi.fn(),
};

const retryAutomationTaskFromLedgerMock = vi.fn();
const relaunchMock = vi.fn();
const showErrorMock = vi.fn();
const runGuardedQuitMock = vi.fn();

vi.mock('@tauri-apps/plugin-updater', () => ({}));

vi.mock('@tauri-apps/plugin-process', () => ({
  relaunch: (...args: unknown[]) => relaunchMock(...args),
}));

vi.mock('@tauri-apps/plugin-opener', () => ({
  openUrl: vi.fn(),
}));

vi.mock('../../services/quitGuard', () => ({
  runGuardedQuit: (...args: unknown[]) => runGuardedQuitMock(...args),
}));

vi.mock('../../services/automationTaskRetryService', () => ({
  retryAutomationTaskFromLedger: (...args: unknown[]) => retryAutomationTaskFromLedgerMock(...args),
}));

vi.mock('../../stores/errorDialogStore', () => ({
  useErrorDialogStore: {
    getState: () => ({
      showError: showErrorMock,
    }),
  },
}));

vi.mock('../../i18n', () => ({
  default: {
    t: (key: string, options?: Record<string, unknown>) => translate(key, options),
  },
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) => translate(key, options),
  }),
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
}));

vi.mock('../../stores/taskLedgerStore', () => ({
  useTaskLedgerStore: (selector: any) => selector(taskLedgerState),
}));

vi.mock('../../stores/recoveryStore', () => ({
  useRecoveryStore: (selector: any) => selector(recoveryState),
}));

vi.mock('../../stores/automationStore', () => ({
  useAutomationStore: (selector: any) => selector(automationState),
}));

vi.mock('../../stores/batchQueueStore', () => ({
  useBatchQueueStore: (selector: any) => selector(batchQueueState),
}));

function interpolate(template: string, options?: Record<string, unknown>): string {
  return template.replace(/\{\{(\w+)\}\}/g, (_, key) => String(options?.[key] ?? ''));
}

function translate(key: string, options?: Record<string, unknown>): string {
  if (options?.defaultValue && typeof options.defaultValue === 'string') {
    return interpolate(options.defaultValue, options);
  }

  if (key === 'header.notifications') return 'Notifications';
  if (key === 'task_center.panel_title') return 'Task Center';
  if (key === 'task_center.empty') return 'No active tasks right now.';
  if (key === 'task_center.needs_action') return 'Needs action';
  if (key === 'task_center.active') return 'Active';
  if (key === 'task_center.recent') return 'Recent';
  if (key === 'task_center.clear_recent') return 'Clear recent';
  if (key === 'task_center.cancel_pending_hint') return 'Stops after the current step and skips the final writeback.';
  if (key === 'settings.update_available') return `Update ${options?.version}`;
  if (key === 'settings.update_desc_default') return 'A new version of Sona is available.';
  if (key === 'settings.update_downloading') return 'Downloading update...';
  if (key === 'settings.update_installing') return 'Installing update...';
  if (key === 'settings.update_relaunch') return 'Relaunch to update';
  if (key === 'settings.update_btn_install') return 'Install Update';
  if (key === 'settings.update_btn_relaunch') return 'Relaunch';
  if (key === 'common.cancel') return 'Cancel';
  if (key === 'common.close') return 'Close';
  if (key === 'common.resume') return 'Resume';
  if (key === 'recovery.actions.open_center') return 'Open Recovery Center';
  if (key === 'automation.retry_failed') return 'Retry Failed';
  if (key === 'automation.open_settings') return 'Open Automation';
  if (key === 'automation.notifications.failure_title') return `${options?.ruleName} needs attention`;
  if (key === 'automation.notifications.failure_body') return `${options?.count} failed file(s). Latest: ${options?.fileName}`;
  if (key === 'automation.notifications.stage_detail') return `Latest stage: ${options?.stage}`;
  if (key === 'automation.notifications.file_unknown') return 'Latest item unavailable';
  if (key === 'recovery.stage.transcribing') return 'Transcribing';
  return key;
}

function makeUpdate(version: string, overrides: Record<string, unknown> = {}) {
  return {
    currentVersion: '0.6.0',
    version,
    body: 'Release notes',
    downloadAndInstall: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

function makeTask(overrides: Partial<TaskLedgerRecord> = {}): TaskLedgerRecord {
  return {
    id: 'task-1',
    kind: 'batchImport',
    status: 'running',
    title: 'meeting.wav',
    progress: 25,
    createdAt: 100,
    updatedAt: 100,
    retryable: true,
    cancelable: true,
    recoverable: false,
    filePath: 'C:\\audio\\meeting.wav',
    ...overrides,
  };
}

function resetUpdaterStore() {
  useAppUpdaterStore.setState({
    status: 'idle',
    error: null,
    updateInfo: null,
    progress: 0,
    dismissedVersion: null,
    notificationVisible: false,
    hasAutoCheckedThisSession: false,
  });
}

describe('NotificationCenter task center', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    taskLedgerState.tasks = [];
    automationState.notifications = [];
    retryAutomationTaskFromLedgerMock.mockResolvedValue(undefined);
    resetUpdaterStore();
    runGuardedQuitMock.mockImplementation(async (onExit: () => Promise<void>) => {
      await onExit();
      return true;
    });
  });

  it('shows an empty task center with no badge when there is no active or actionable task', () => {
    const { container } = render(
      <NotificationCenter
        onOpenRecoveryCenter={vi.fn()}
        onOpenAutomationSettings={vi.fn()}
      />,
    );

    expect(container.querySelector('.notification-center-trigger-badge')).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));

    expect(screen.getByRole('dialog', { name: 'Task Center' })).toBeDefined();
    expect(screen.getByText('No active tasks right now.')).toBeDefined();
  });

  it('groups task ledger records into Needs action, Active, and Recent sections', () => {
    taskLedgerState.tasks = [
      makeTask({ id: 'active-task', status: 'running', title: 'active.wav', updatedAt: 300 }),
      makeTask({
        id: 'failed-task',
        status: 'failed',
        title: 'failed.wav',
        updatedAt: 200,
        errorMessage: 'Transcription failed',
      }),
      makeTask({ id: 'recent-task', status: 'succeeded', title: 'done.wav', progress: 100, updatedAt: 100 }),
    ];

    const { container } = render(
      <NotificationCenter
        onOpenRecoveryCenter={vi.fn()}
        onOpenAutomationSettings={vi.fn()}
      />,
    );

    expect(container.querySelector('.notification-center-trigger-badge')?.textContent).toBe('2');

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));

    expect(screen.getByText('Needs action')).toBeDefined();
    expect(screen.getByText('Active')).toBeDefined();
    expect(screen.getByText('Recent')).toBeDefined();
    expect(screen.getByText('failed.wav')).toBeDefined();
    expect(screen.getByText('active.wav')).toBeDefined();
    expect(screen.getByText('done.wav')).toBeDefined();
    expect(screen.getByText('Transcription failed')).toBeDefined();
  });

  it('requests soft cancellation for active ledger tasks', () => {
    taskLedgerState.tasks = [
      makeTask({ id: 'active-task', status: 'running', title: 'active.wav' }),
    ];

    render(
      <NotificationCenter
        onOpenRecoveryCenter={vi.fn()}
        onOpenAutomationSettings={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));

    expect(taskLedgerState.requestCancel).toHaveBeenCalledWith('active-task');
  });

  it('explains that cancel-requested LLM tasks stop after the current step', () => {
    taskLedgerState.tasks = [
      makeTask({
        id: 'llm-stopping',
        kind: 'llmSummary',
        status: 'cancelRequested',
        title: 'AI Summary',
        cancelable: false,
      }),
    ];

    render(
      <NotificationCenter
        onOpenRecoveryCenter={vi.fn()}
        onOpenAutomationSettings={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));

    expect(screen.getByText('Stops after the current step and skips the final writeback.')).toBeDefined();
  });

  it('resumes and discards individual recovery tasks from the task center', () => {
    taskLedgerState.tasks = [
      makeTask({
        id: 'recovery-recovery-1',
        kind: 'recovery',
        status: 'recoverable',
        title: 'recover.wav',
        recoverable: true,
        cancelable: false,
      }),
    ];

    render(
      <NotificationCenter
        onOpenRecoveryCenter={vi.fn()}
        onOpenAutomationSettings={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));
    fireEvent.click(screen.getByRole('button', { name: 'Resume' }));
    fireEvent.click(screen.getByRole('button', { name: 'Discard' }));

    expect(recoveryState.resumeItem).toHaveBeenCalledWith('recovery-1');
    expect(recoveryState.discardItem).toHaveBeenCalledWith('recovery-1');
  });

  it('opens the recovery center from individual recovery ledger tasks', () => {
    taskLedgerState.tasks = [
      makeTask({
        id: 'recovery-recovery-1',
        kind: 'recovery',
        status: 'recoverable',
        title: 'recover.wav',
        recoverable: true,
        cancelable: false,
      }),
    ];
    const onOpenRecoveryCenter = vi.fn();

    render(
      <NotificationCenter
        onOpenRecoveryCenter={onOpenRecoveryCenter}
        onOpenAutomationSettings={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));
    fireEvent.click(screen.getByRole('button', { name: 'Open Recovery Center' }));

    expect(onOpenRecoveryCenter).toHaveBeenCalled();
    expect(screen.queryByRole('dialog', { name: 'Task Center' })).toBeNull();
  });

  it('shows only the recoverable entry when a stale batch task exists for the same recovery item', () => {
    taskLedgerState.tasks = [
      makeTask({
        id: 'batch-recovery-1',
        kind: 'batchImport',
        status: 'pending',
        title: 'recover.wav',
        progress: 0,
        cancelable: true,
        recoverable: false,
        stage: 'transcribing',
      }),
      makeTask({
        id: 'recovery-recovery-1',
        kind: 'recovery',
        status: 'recoverable',
        title: 'recover.wav',
        progress: 25,
        recoverable: true,
        cancelable: false,
        stage: 'transcribing',
      }),
    ];

    render(
      <NotificationCenter
        onOpenRecoveryCenter={vi.fn()}
        onOpenAutomationSettings={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));

    expect(screen.getAllByText('recover.wav')).toHaveLength(1);
    expect(screen.getByText(/Recovery .* Recoverable/)).toBeDefined();
    expect(screen.queryByText(/Batch import .* Pending/)).toBeNull();
  });

  it('retries failed batch ledger tasks through the batch queue', async () => {
    taskLedgerState.tasks = [
      makeTask({
        id: 'batch-failed',
        status: 'failed',
        title: 'failed.wav',
        filePath: 'C:\\audio\\failed.wav',
        projectId: 'project-2',
      }),
    ];

    render(
      <NotificationCenter
        onOpenRecoveryCenter={vi.fn()}
        onOpenAutomationSettings={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
      await Promise.resolve();
    });

    expect(batchQueueState.addFiles).toHaveBeenCalledWith(['C:\\audio\\failed.wav'], {
      projectId: 'project-2',
    });
    expect(taskLedgerState.removeTask).toHaveBeenCalledWith('batch-failed');
  });

  it('opens automation settings for rule-level automation failures without retry metadata', async () => {
    const onOpenAutomationSettings = vi.fn();
    taskLedgerState.tasks = [
      makeTask({
        id: 'automation-failed',
        kind: 'automation',
        status: 'failed',
        title: 'watch.wav',
        automationRuleId: 'rule-1',
        filePath: undefined,
      }),
    ];

    render(
      <NotificationCenter
        onOpenRecoveryCenter={vi.fn()}
        onOpenAutomationSettings={onOpenAutomationSettings}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Open Automation' }));
      await Promise.resolve();
    });

    expect(onOpenAutomationSettings).toHaveBeenCalled();
    expect(taskLedgerState.removeTask).not.toHaveBeenCalledWith('automation-failed');
  });

  it('keeps visible updates as transient task center entries', () => {
    useAppUpdaterStore.setState({
      status: 'available',
      updateInfo: makeUpdate('1.2.3') as any,
      notificationVisible: true,
    });

    const { container } = render(
      <NotificationCenter
        onOpenRecoveryCenter={vi.fn()}
        onOpenAutomationSettings={vi.fn()}
      />,
    );

    expect(container.querySelector('.notification-center-trigger-badge')?.textContent).toBe('1');

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));

    expect(screen.getByText('Update 1.2.3')).toBeDefined();
    expect(screen.getByText('Release notes')).toBeDefined();
    expect(screen.getByRole('button', { name: 'Install Update' })).toBeDefined();
  });

  it('disables update actions while an update is downloading', () => {
    useAppUpdaterStore.setState({
      status: 'downloading',
      updateInfo: makeUpdate('1.2.3') as any,
      progress: 35,
      notificationVisible: true,
    });

    render(
      <NotificationCenter
        onOpenRecoveryCenter={vi.fn()}
        onOpenAutomationSettings={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));

    const downloadingButton = screen.getByRole('button', { name: 'Downloading update...' }) as HTMLButtonElement;
    const closeButton = screen.getByRole('button', { name: 'Close' }) as HTMLButtonElement;

    expect(downloadingButton.disabled).toBe(true);
    expect(closeButton.disabled).toBe(true);
  });

  it('does not render legacy automation session notifications in the task center', async () => {
    automationState.notifications = [
      {
        id: 'automation-failure-rule-2',
        kind: 'failure',
        ruleId: 'rule-2',
        ruleName: 'Failure Rule',
        count: 1,
        latestFilePath: 'C:\\watch\\failed.wav',
        latestStage: 'transcribing',
        latestMessage: 'Transcription failed',
        createdAt: 9,
        updatedAt: 11,
        retryable: true,
      },
    ];

    render(
      <NotificationCenter
        onOpenRecoveryCenter={vi.fn()}
        onOpenAutomationSettings={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));

    expect(screen.queryByText('Failure Rule needs attention')).toBeNull();
    expect(screen.queryByRole('button', { name: 'Retry Failed' })).toBeNull();
    expect(screen.getByText('No active tasks right now.')).toBeDefined();
  });

  it('retries failed automation ledger file tasks through the task retry service', async () => {
    const task = makeTask({
      id: 'automation-failed-file',
      kind: 'automation',
      status: 'failed',
      title: 'failed.wav',
      automationRuleId: 'rule-2',
      filePath: 'C:\\watch\\failed.wav',
    });
    taskLedgerState.tasks = [task];

    render(
      <NotificationCenter
        onOpenRecoveryCenter={vi.fn()}
        onOpenAutomationSettings={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
      await Promise.resolve();
    });

    expect(retryAutomationTaskFromLedgerMock).toHaveBeenCalledWith(task);
    expect(taskLedgerState.removeTask).toHaveBeenCalledWith('automation-failed-file');
  });

  it('updates the transient update task while installing and then offers relaunch', async () => {
    const downloadAndInstall = vi.fn().mockImplementation(async (onEvent: any) => {
      onEvent({ event: 'Started', data: { contentLength: 100 } });
      onEvent({ event: 'Progress', data: { chunkLength: 40 } });
      onEvent({ event: 'Progress', data: { chunkLength: 60 } });
      onEvent({ event: 'Finished' });
    });

    useAppUpdaterStore.setState({
      status: 'available',
      updateInfo: makeUpdate('1.2.3', { downloadAndInstall }) as any,
      notificationVisible: true,
    });

    render(
      <NotificationCenter
        onOpenRecoveryCenter={vi.fn()}
        onOpenAutomationSettings={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Install Update' }));
      await Promise.resolve();
    });

    await waitFor(() => {
      expect(downloadAndInstall).toHaveBeenCalled();
      expect(useAppUpdaterStore.getState().status).toBe('downloaded');
      expect(screen.getByText('Relaunch to update')).toBeDefined();
      expect(screen.getByRole('button', { name: 'Relaunch' })).toBeDefined();
    });
  });
});
