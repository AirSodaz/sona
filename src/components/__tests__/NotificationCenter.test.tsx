import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { NotificationCenter } from '../NotificationCenter';
import { useAppUpdaterStore } from '../../stores/appUpdaterStore';

const recoveryState = {
  items: [] as any[],
  isLoaded: true,
};

const automationState = {
  notifications: [] as any[],
  dismissNotification: vi.fn(),
  retryNotification: vi.fn(),
};

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

vi.mock('../../stores/errorDialogStore', () => ({
  useErrorDialogStore: {
    getState: () => ({
      showError: showErrorMock,
    }),
  },
}));

vi.mock('../../i18n', () => ({
  default: {
    t: (key: string, options?: Record<string, unknown>) => options?.version ? `${key}:${options.version}` : key,
  },
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) => {
      if (key === 'header.notifications') return 'Notifications';
      if (key === 'header.notifications_panel') return 'Notifications';
      if (key === 'header.notifications_empty') return 'No notifications right now.';
      if (key === 'recovery.banner.title') return 'Interrupted work is ready to recover';
      if (key === 'recovery.banner.body') {
        return `${options?.count} file(s) waiting. Batch: ${options?.batchCount} · Automation: ${options?.automationCount}`;
      }
      if (key === 'recovery.actions.open_center') return 'Open Recovery Center';
      if (key === 'recovery.stage.transcribing') return 'Transcribing';
      if (key === 'recovery.stage.translating') return 'Translating';
      if (key === 'recovery.stage.exporting') return 'Exporting';
      if (key === 'settings.update_available') return `settings.update_available:${options?.version}`;
      if (key === 'settings.update_desc_default') return 'A new version of Sona is available.';
      if (key === 'settings.update_downloading') return 'Downloading update...';
      if (key === 'settings.update_installing') return 'Installing update...';
      if (key === 'settings.update_relaunch') return 'Relaunch to update';
      if (key === 'settings.update_btn_install') return 'Install Update';
      if (key === 'settings.update_btn_relaunch') return 'Relaunch';
      if (key === 'automation.retry_failed') return 'Retry Failed';
      if (key === 'automation.open_settings') return 'Open Automation';
      if (key === 'automation.notifications.failure_title') return `${options?.ruleName} needs attention`;
      if (key === 'automation.notifications.failure_body') {
        return `${options?.count} failed file(s). Latest: ${options?.fileName}`;
      }
      if (key === 'automation.notifications.success_title') return `${options?.ruleName} completed`;
      if (key === 'automation.notifications.success_body') {
        return `${options?.count} completed file(s). Latest: ${options?.fileName}`;
      }
      if (key === 'automation.notifications.stage_detail') return `Latest stage: ${options?.stage}`;
      if (key === 'automation.notifications.file_unknown') return 'Latest item unavailable';
      if (key === 'common.close') return 'Close';
      return key;
    },
  }),
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
}));

vi.mock('../../stores/recoveryStore', () => ({
  useRecoveryStore: (selector: any) => selector(recoveryState),
}));

vi.mock('../../stores/automationStore', () => ({
  useAutomationStore: (selector: any) => selector(automationState),
}));

function makeUpdate(version: string, overrides: Record<string, unknown> = {}) {
  return {
    currentVersion: '0.6.0',
    version,
    body: 'Release notes',
    downloadAndInstall: vi.fn().mockResolvedValue(undefined),
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

function resetAutomationState() {
  automationState.notifications = [];
  automationState.dismissNotification.mockImplementation((notificationId: string) => {
    automationState.notifications = automationState.notifications.filter((item) => item.id !== notificationId);
  });
  automationState.retryNotification.mockImplementation(async (notificationId: string) => {
    automationState.notifications = automationState.notifications.filter((item) => item.id !== notificationId);
  });
}

describe('NotificationCenter', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    recoveryState.items = [];
    recoveryState.isLoaded = true;
    resetAutomationState();
    resetUpdaterStore();
    runGuardedQuitMock.mockImplementation(async (onExit: () => Promise<void>) => {
      await onExit();
      return true;
    });
  });

  it('shows the empty state and no badge when there are no notifications', () => {
    const { container } = render(
      <NotificationCenter
        onOpenRecoveryCenter={vi.fn()}
        onOpenAutomationSettings={vi.fn()}
      />
    );

    expect(container.querySelector('.notification-center-trigger-badge')).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));

    expect(screen.getByRole('dialog', { name: 'Notifications' })).toBeDefined();
    expect(screen.getByText('No notifications right now.')).toBeDefined();
    expect(screen.queryByText('Interrupted work is ready to recover')).toBeNull();
    expect(screen.queryByText('settings.update_available:1.2.3')).toBeNull();
  });

  it('renders one update notification when the automatic update reminder is visible', () => {
    useAppUpdaterStore.setState({
      status: 'available',
      updateInfo: makeUpdate('1.2.3') as any,
      notificationVisible: true,
    });

    const { container } = render(
      <NotificationCenter
        onOpenRecoveryCenter={vi.fn()}
        onOpenAutomationSettings={vi.fn()}
      />
    );

    expect(container.querySelector('.notification-center-trigger-badge')?.textContent).toBe('1');

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));

    expect(screen.getByText('settings.update_available:1.2.3')).toBeDefined();
    expect(screen.getByText('Release notes')).toBeDefined();
    expect(screen.getByRole('button', { name: 'Install Update' })).toBeDefined();
  });

  it('renders one recovery notification with the aggregated counts', () => {
    recoveryState.items = [
      { id: 'recovery-1', source: 'batch_import', resolution: 'pending' },
      { id: 'recovery-2', source: 'batch_import', resolution: 'pending' },
      { id: 'recovery-3', source: 'automation', resolution: 'pending' },
    ];

    const { container } = render(
      <NotificationCenter
        onOpenRecoveryCenter={vi.fn()}
        onOpenAutomationSettings={vi.fn()}
      />
    );

    expect(container.querySelector('.notification-center-trigger-badge')?.textContent).toBe('1');

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));

    expect(screen.getByText('Interrupted work is ready to recover')).toBeDefined();
    expect(screen.getByText('3 file(s) waiting. Batch: 2 · Automation: 1')).toBeDefined();
    expect(screen.getByRole('button', { name: 'Open Recovery Center' })).toBeDefined();
  });

  it('shows update, automation failure, recovery, and automation success notifications in order', () => {
    recoveryState.items = [
      { id: 'recovery-1', source: 'batch_import', resolution: 'pending' },
    ];
    automationState.notifications = [
      {
        id: 'automation-success-rule-1-1',
        kind: 'success',
        ruleId: 'rule-1',
        ruleName: 'Success Rule',
        count: 2,
        latestFilePath: 'C:\\watch\\done.wav',
        latestStage: 'exporting',
        createdAt: 10,
        updatedAt: 12,
        retryable: false,
      },
      {
        id: 'automation-failure-rule-2',
        kind: 'failure',
        ruleId: 'rule-2',
        ruleName: 'Failure Rule',
        count: 1,
        latestFilePath: 'C:\\watch\\failed.wav',
        latestStage: 'translating',
        latestMessage: 'Translation failed',
        createdAt: 9,
        updatedAt: 11,
        retryable: true,
      },
    ];
    useAppUpdaterStore.setState({
      status: 'available',
      updateInfo: makeUpdate('1.2.3') as any,
      notificationVisible: true,
    });

    const { container } = render(
      <NotificationCenter
        onOpenRecoveryCenter={vi.fn()}
        onOpenAutomationSettings={vi.fn()}
      />
    );

    expect(container.querySelector('.notification-center-trigger-badge')?.textContent).toBe('4');

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));

    const titles = Array.from(container.querySelectorAll('.notification-center-item strong')).map((node) => node.textContent);
    expect(titles).toEqual([
      'settings.update_available:1.2.3',
      'Failure Rule needs attention',
      'Interrupted work is ready to recover',
      'Success Rule completed',
    ]);
    expect(container.querySelectorAll('.notification-center-item-header')).toHaveLength(4);
    expect(container.querySelectorAll('.notification-center-item-actions')).toHaveLength(4);
  });

  it('caps the trigger badge at 9+ while keeping the full notification list', () => {
    automationState.notifications = Array.from({ length: 10 }, (_, index) => ({
      id: `automation-success-rule-${index}`,
      kind: 'success',
      ruleId: `rule-${index}`,
      ruleName: `Success Rule ${index}`,
      count: 1,
      latestFilePath: `C:\\watch\\done-${index}.wav`,
      latestStage: 'exporting',
      createdAt: index,
      updatedAt: index,
      retryable: false,
    }));

    const { container } = render(
      <NotificationCenter
        onOpenRecoveryCenter={vi.fn()}
        onOpenAutomationSettings={vi.fn()}
      />
    );

    expect(container.querySelector('.notification-center-trigger-badge')?.textContent).toBe('9+');

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));
    expect(container.querySelectorAll('.notification-center-item')).toHaveLength(10);
  });

  it('opens the recovery center from either the notification body or the CTA', () => {
    recoveryState.items = [
      { id: 'recovery-1', source: 'batch_import', resolution: 'pending' },
    ];
    const onOpenRecoveryCenter = vi.fn();

    render(
      <NotificationCenter
        onOpenRecoveryCenter={onOpenRecoveryCenter}
        onOpenAutomationSettings={vi.fn()}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));
    fireEvent.click(screen.getByRole('button', { name: /Interrupted work is ready to recover/i }));

    expect(onOpenRecoveryCenter).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole('dialog', { name: 'Notifications' })).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));
    fireEvent.click(screen.getByRole('button', { name: 'Open Recovery Center' }));

    expect(onOpenRecoveryCenter).toHaveBeenCalledTimes(2);
    expect(screen.queryByRole('dialog', { name: 'Notifications' })).toBeNull();
  });

  it('opens automation settings from the notification body and CTA', () => {
    automationState.notifications = [
      {
        id: 'automation-success-rule-1-1',
        kind: 'success',
        ruleId: 'rule-1',
        ruleName: 'Success Rule',
        count: 2,
        latestFilePath: 'C:\\watch\\done.wav',
        latestStage: 'exporting',
        createdAt: 10,
        updatedAt: 12,
        retryable: false,
      },
    ];
    const onOpenAutomationSettings = vi.fn();

    render(
      <NotificationCenter
        onOpenRecoveryCenter={vi.fn()}
        onOpenAutomationSettings={onOpenAutomationSettings}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));
    fireEvent.click(screen.getByRole('button', { name: /Success Rule completed/i }));

    expect(onOpenAutomationSettings).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole('dialog', { name: 'Notifications' })).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));
    fireEvent.click(screen.getByRole('button', { name: 'Open Automation' }));

    expect(onOpenAutomationSettings).toHaveBeenCalledTimes(2);
  });

  it('retries failed automation notifications and removes the failure entry', async () => {
    automationState.notifications = [
      {
        id: 'automation-failure-rule-2',
        kind: 'failure',
        ruleId: 'rule-2',
        ruleName: 'Failure Rule',
        count: 1,
        latestFilePath: 'C:\\watch\\failed.wav',
        latestStage: 'translating',
        latestMessage: 'Translation failed',
        createdAt: 9,
        updatedAt: 11,
        retryable: true,
      },
    ];

    const view = render(
      <NotificationCenter
        onOpenRecoveryCenter={vi.fn()}
        onOpenAutomationSettings={vi.fn()}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Retry Failed' }));
      await Promise.resolve();
    });

    expect(automationState.retryNotification).toHaveBeenCalledWith('automation-failure-rule-2');

    view.rerender(
      <NotificationCenter
        onOpenRecoveryCenter={vi.fn()}
        onOpenAutomationSettings={vi.fn()}
      />
    );

    expect(screen.queryByText('Failure Rule needs attention')).toBeNull();
  });

  it('dismisses automation notifications and updates the badge count', () => {
    automationState.notifications = [
      {
        id: 'automation-success-rule-1-1',
        kind: 'success',
        ruleId: 'rule-1',
        ruleName: 'Success Rule',
        count: 1,
        latestFilePath: 'C:\\watch\\done.wav',
        latestStage: 'exporting',
        createdAt: 10,
        updatedAt: 12,
        retryable: false,
      },
    ];

    const view = render(
      <NotificationCenter
        onOpenRecoveryCenter={vi.fn()}
        onOpenAutomationSettings={vi.fn()}
      />
    );

    expect(view.container.querySelector('.notification-center-trigger-badge')?.textContent).toBe('1');

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));

    view.rerender(
      <NotificationCenter
        onOpenRecoveryCenter={vi.fn()}
        onOpenAutomationSettings={vi.fn()}
      />
    );

    expect(view.container.querySelector('.notification-center-trigger-badge')).toBeNull();
    expect(screen.queryByText('Success Rule completed')).toBeNull();
  });

  it('dismisses the update notification and records the dismissed version', () => {
    useAppUpdaterStore.setState({
      status: 'available',
      updateInfo: makeUpdate('1.2.3') as any,
      notificationVisible: true,
    });

    render(
      <NotificationCenter
        onOpenRecoveryCenter={vi.fn()}
        onOpenAutomationSettings={vi.fn()}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));

    expect(useAppUpdaterStore.getState().notificationVisible).toBe(false);
    expect(useAppUpdaterStore.getState().dismissedVersion).toBe('1.2.3');
  });

  it('updates the notification in place while installing and then offers relaunch', async () => {
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
      />
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

  it('does not surface a manual update result through the notification center when the reminder is hidden', () => {
    useAppUpdaterStore.setState({
      status: 'available',
      updateInfo: makeUpdate('1.2.3') as any,
      notificationVisible: false,
    });

    const { container } = render(
      <NotificationCenter
        onOpenRecoveryCenter={vi.fn()}
        onOpenAutomationSettings={vi.fn()}
      />
    );

    expect(container.querySelector('.notification-center-trigger-badge')).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));

    expect(screen.queryByText('settings.update_available:1.2.3')).toBeNull();
    expect(screen.getByText('No notifications right now.')).toBeDefined();
  });

  it('closes the open panel on outside click and Escape', () => {
    render(
      <NotificationCenter
        onOpenRecoveryCenter={vi.fn()}
        onOpenAutomationSettings={vi.fn()}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));
    expect(screen.getByRole('dialog', { name: 'Notifications' })).toBeDefined();

    fireEvent.mouseDown(document.body);
    expect(screen.queryByRole('dialog', { name: 'Notifications' })).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));
    expect(screen.getByRole('dialog', { name: 'Notifications' })).toBeDefined();

    fireEvent.keyDown(document, { key: 'Escape' });
    expect(screen.queryByRole('dialog', { name: 'Notifications' })).toBeNull();
  });
});
