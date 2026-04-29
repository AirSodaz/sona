import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { NotificationCenter } from '../NotificationCenter';
import { useAppUpdaterStore } from '../../stores/appUpdaterStore';

const recoveryState = {
  items: [] as any[],
  isLoaded: true,
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
      if (key === 'settings.update_available') return `settings.update_available:${options?.version}`;
      if (key === 'settings.update_desc_default') return 'A new version of Sona is available.';
      if (key === 'settings.update_downloading') return 'Downloading update...';
      if (key === 'settings.update_installing') return 'Installing update...';
      if (key === 'settings.update_relaunch') return 'Relaunch to update';
      if (key === 'settings.update_btn_install') return 'Install Update';
      if (key === 'settings.update_btn_relaunch') return 'Relaunch';
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

describe('NotificationCenter', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    recoveryState.items = [];
    recoveryState.isLoaded = true;
    resetUpdaterStore();
    runGuardedQuitMock.mockImplementation(async (onExit: () => Promise<void>) => {
      await onExit();
      return true;
    });
  });

  it('shows the empty state and no badge when there are no notifications', () => {
    const { container } = render(<NotificationCenter onOpenRecoveryCenter={vi.fn()} />);

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

    const { container } = render(<NotificationCenter onOpenRecoveryCenter={vi.fn()} />);

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

    const { container } = render(<NotificationCenter onOpenRecoveryCenter={vi.fn()} />);

    expect(container.querySelector('.notification-center-trigger-badge')?.textContent).toBe('1');

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));

    expect(screen.getByText('Interrupted work is ready to recover')).toBeDefined();
    expect(screen.getByText('3 file(s) waiting. Batch: 2 · Automation: 1')).toBeDefined();
    expect(screen.getByRole('button', { name: 'Open Recovery Center' })).toBeDefined();
  });

  it('shows the update notification before the recovery notification when both are present', () => {
    recoveryState.items = [
      { id: 'recovery-1', source: 'batch_import', resolution: 'pending' },
    ];
    useAppUpdaterStore.setState({
      status: 'available',
      updateInfo: makeUpdate('1.2.3') as any,
      notificationVisible: true,
    });

    const { container } = render(<NotificationCenter onOpenRecoveryCenter={vi.fn()} />);

    expect(container.querySelector('.notification-center-trigger-badge')?.textContent).toBe('2');

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));

    const titles = Array.from(container.querySelectorAll('.notification-center-item strong')).map((node) => node.textContent);
    expect(titles).toEqual([
      'settings.update_available:1.2.3',
      'Interrupted work is ready to recover',
    ]);
  });

  it('opens the recovery center from either the notification body or the CTA', () => {
    recoveryState.items = [
      { id: 'recovery-1', source: 'batch_import', resolution: 'pending' },
    ];
    const onOpenRecoveryCenter = vi.fn();

    render(<NotificationCenter onOpenRecoveryCenter={onOpenRecoveryCenter} />);

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));
    fireEvent.click(screen.getByRole('button', { name: /Interrupted work is ready to recover/i }));

    expect(onOpenRecoveryCenter).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole('dialog', { name: 'Notifications' })).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));
    fireEvent.click(screen.getByRole('button', { name: 'Open Recovery Center' }));

    expect(onOpenRecoveryCenter).toHaveBeenCalledTimes(2);
    expect(screen.queryByRole('dialog', { name: 'Notifications' })).toBeNull();
  });

  it('dismisses the update notification and records the dismissed version', () => {
    useAppUpdaterStore.setState({
      status: 'available',
      updateInfo: makeUpdate('1.2.3') as any,
      notificationVisible: true,
    });

    render(<NotificationCenter onOpenRecoveryCenter={vi.fn()} />);

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

    render(<NotificationCenter onOpenRecoveryCenter={vi.fn()} />);

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

    const { container } = render(<NotificationCenter onOpenRecoveryCenter={vi.fn()} />);

    expect(container.querySelector('.notification-center-trigger-badge')).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: 'Notifications' }));

    expect(screen.queryByText('settings.update_available:1.2.3')).toBeNull();
    expect(screen.getByText('No notifications right now.')).toBeDefined();
  });

  it('closes the open panel on outside click and Escape', () => {
    render(<NotificationCenter onOpenRecoveryCenter={vi.fn()} />);

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
