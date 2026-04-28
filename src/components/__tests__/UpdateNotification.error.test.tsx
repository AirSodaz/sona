import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { UpdateNotification } from '../UpdateNotification';
import { useAppUpdaterStore } from '../../stores/appUpdaterStore';

const relaunchMock = vi.fn();
const openUrlMock = vi.fn();
const showErrorMock = vi.fn();
const runGuardedQuitMock = vi.fn();

vi.mock('@tauri-apps/plugin-updater', () => ({}));

vi.mock('@tauri-apps/plugin-process', () => ({
  relaunch: (...args: unknown[]) => relaunchMock(...args),
}));

vi.mock('@tauri-apps/plugin-opener', () => ({
  openUrl: (...args: unknown[]) => openUrlMock(...args),
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

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) => options?.version ? `${key}:${options.version}` : key,
  }),
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
}));

vi.mock('../../i18n', () => ({
  default: {
    t: (key: string, options?: Record<string, unknown>) => options?.version ? `${key}:${options.version}` : key,
  },
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

describe('UpdateNotification', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetUpdaterStore();
    runGuardedQuitMock.mockImplementation(async (onExit: () => Promise<void>) => {
      await onExit();
      return true;
    });
  });

  it('dismisses the toast and records the version for this session', () => {
    useAppUpdaterStore.setState({
      status: 'available',
      updateInfo: makeUpdate('1.2.3') as any,
      notificationVisible: true,
    });

    render(<UpdateNotification />);

    fireEvent.click(screen.getByText('common.cancel'));

    expect(useAppUpdaterStore.getState().notificationVisible).toBe(false);
    expect(useAppUpdaterStore.getState().dismissedVersion).toBe('1.2.3');
  });

  it('opens the releases page when install failure resolves to primary action', async () => {
    const downloadAndInstall = vi.fn().mockRejectedValue(new Error('network timeout'));
    useAppUpdaterStore.setState({
      status: 'available',
      updateInfo: {
      version: '1.2.3',
      body: 'Release notes',
      downloadAndInstall,
      } as any,
      notificationVisible: true,
    });
    showErrorMock.mockResolvedValueOnce('primary');

    render(<UpdateNotification />);

    expect(screen.getByText('settings.update_available:1.2.3')).toBeDefined();

    await act(async () => {
      fireEvent.click(screen.getByText('settings.update_btn_install'));
      await Promise.resolve();
    });

    expect(downloadAndInstall).toHaveBeenCalled();
    expect(showErrorMock).toHaveBeenCalled();
    expect(openUrlMock).toHaveBeenCalledWith('https://github.com/AirSodaz/sona/releases/latest');
  });

  it('renders the shared downloaded state and relaunches from the toast', async () => {
    useAppUpdaterStore.setState({
      status: 'downloaded',
      progress: 100,
      updateInfo: makeUpdate('1.2.3') as any,
      notificationVisible: true,
    });

    render(<UpdateNotification />);

    expect(screen.getByText('settings.update_relaunch')).toBeDefined();

    await act(async () => {
      fireEvent.click(screen.getByText('settings.update_btn_relaunch'));
      await Promise.resolve();
    });

    await waitFor(() => {
      expect(relaunchMock).toHaveBeenCalled();
    });
  });
});
