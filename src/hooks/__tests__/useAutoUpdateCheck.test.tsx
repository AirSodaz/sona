import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useAutoUpdateCheck } from '../useAutoUpdateCheck';
import { useConfigStore } from '../../stores/configStore';
import { useAppUpdaterStore } from '../../stores/appUpdaterStore';

const checkMock = vi.fn();

vi.mock('@tauri-apps/plugin-updater', () => ({
  check: (...args: unknown[]) => checkMock(...args),
}));

vi.mock('@tauri-apps/plugin-process', () => ({
  relaunch: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-opener', () => ({
  openUrl: vi.fn(),
}));

vi.mock('../../i18n', () => ({
  default: {
    t: (key: string) => key,
  },
}));

vi.mock('../../stores/errorDialogStore', () => ({
  useErrorDialogStore: {
    getState: () => ({
      showError: vi.fn(),
    }),
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

describe('useAutoUpdateCheck', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.clearAllMocks();
    resetUpdaterStore();
    useConfigStore.setState({
      config: {
        ...useConfigStore.getState().config,
        autoCheckUpdates: true,
      },
    });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('runs the automatic check once after startup and shows the toast for a new version', async () => {
    checkMock.mockResolvedValueOnce(makeUpdate('1.2.3'));

    renderHook(() => useAutoUpdateCheck(true));

    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
    });

    expect(checkMock).toHaveBeenCalledTimes(1);
    expect(useAppUpdaterStore.getState().notificationVisible).toBe(true);
  });

  it('skips the automatic check when the preference is disabled', async () => {
    useConfigStore.setState({
      config: {
        ...useConfigStore.getState().config,
        autoCheckUpdates: false,
      },
    });

    renderHook(() => useAutoUpdateCheck(true));

    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
    });

    expect(checkMock).not.toHaveBeenCalled();
  });
});
