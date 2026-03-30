import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useAppUpdater } from '../useAppUpdater';

const checkMock = vi.fn();
const openUrlMock = vi.fn();
const showErrorMock = vi.fn();

vi.mock('@tauri-apps/plugin-updater', () => ({
  check: () => checkMock(),
}));

vi.mock('@tauri-apps/plugin-opener', () => ({
  openUrl: (...args: unknown[]) => openUrlMock(...args),
}));

vi.mock('../../stores/errorDialogStore', () => ({
  useErrorDialogStore: (selector: (state: { showError: typeof showErrorMock }) => unknown) => selector({
    showError: showErrorMock,
  }),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
}));

describe('useAppUpdater error handling', () => {
  beforeEach(() => {
    vi.clearAllMocks();
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
