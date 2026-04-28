import { act, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SettingsAboutTab } from '../settings/SettingsAboutTab';
import { useAppUpdaterStore } from '../../stores/appUpdaterStore';

const checkMock = vi.fn();
const runGuardedQuitMock = vi.fn();

vi.mock('@tauri-apps/plugin-updater', () => ({
  check: (...args: unknown[]) => checkMock(...args),
}));

vi.mock('@tauri-apps/plugin-process', () => ({
  relaunch: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-opener', () => ({
  openUrl: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('../../services/quitGuard', () => ({
  runGuardedQuit: (...args: unknown[]) => runGuardedQuitMock(...args),
}));

vi.mock('../../stores/errorDialogStore', () => ({
  useErrorDialogStore: {
    getState: () => ({
      showError: vi.fn(),
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

function makeUpdate(version: string) {
  return {
    currentVersion: '0.6.0',
    version,
    body: 'Release notes',
    downloadAndInstall: vi.fn().mockResolvedValue(undefined),
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

describe('SettingsAboutTab', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    runGuardedQuitMock.mockReset();
    resetUpdaterStore();
  });

  it('shows the available update after a manual trigger even if the toast was dismissed', async () => {
    useAppUpdaterStore.setState({
      dismissedVersion: '1.2.3',
      notificationVisible: false,
    });
    checkMock.mockResolvedValueOnce(makeUpdate('1.2.3'));

    render(<SettingsAboutTab />);

    await act(async () => {
      window.dispatchEvent(new CustomEvent('trigger-update-check'));
      await Promise.resolve();
    });

    await waitFor(() => {
      expect(checkMock).toHaveBeenCalledWith();
      expect(screen.getByText('settings.update_available:1.2.3')).toBeDefined();
    });

    expect(useAppUpdaterStore.getState().notificationVisible).toBe(false);
    expect(useAppUpdaterStore.getState().status).toBe('available');
  });
});
