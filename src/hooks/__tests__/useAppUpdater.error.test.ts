import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useAppUpdater } from '../useAppUpdater';

const checkMock = vi.fn();
const openUrlMock = vi.fn();
const showErrorMock = vi.fn();
const runGuardedQuitMock = vi.fn();

vi.mock('@tauri-apps/plugin-updater', () => ({
  check: () => checkMock(),
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

vi.mock('../../i18n', () => ({
  default: {
    t: (key: string) => key,
  },
}));

describe('useAppUpdater error handling', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    runGuardedQuitMock.mockReset();
  });

  it('opens the releases page when update check failure resolves to primary action', async () => {
    checkMock.mockRejectedValueOnce(new Error('network timeout'));
    showErrorMock.mockResolvedValueOnce('primary');

    const { result } = renderHook(() => useAppUpdater());

    await act(async () => {
      await result.current.checkUpdate(true);
    });

    expect(showErrorMock).toHaveBeenCalled();
    expect(openUrlMock).toHaveBeenCalledWith('https://github.com/AirSodaz/sona/releases/latest');
  });
});
