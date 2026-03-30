import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { UpdateNotification } from '../UpdateNotification';

const checkMock = vi.fn();
const relaunchMock = vi.fn();
const openUrlMock = vi.fn();
const showErrorMock = vi.fn();

vi.mock('@tauri-apps/plugin-updater', () => ({
  check: () => checkMock(),
}));

vi.mock('@tauri-apps/plugin-process', () => ({
  relaunch: () => relaunchMock(),
}));

vi.mock('@tauri-apps/plugin-opener', () => ({
  openUrl: (...args: unknown[]) => openUrlMock(...args),
}));

vi.mock('../../stores/errorDialogStore', () => ({
  useErrorDialogStore: (selector: (state: { showError: typeof showErrorMock }) => unknown) => selector({
    showError: showErrorMock,
  }),
}));

vi.mock('../../stores/transcriptStore', () => ({
  useTranscriptStore: (selector: (state: { config: { autoCheckUpdates: boolean } }) => unknown) => selector({
    config: { autoCheckUpdates: true },
  }),
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

describe('UpdateNotification error handling', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('opens the releases page when install failure resolves to primary action', async () => {
    const downloadAndInstall = vi.fn().mockRejectedValue(new Error('network timeout'));

    checkMock.mockResolvedValueOnce({
      available: true,
      version: '1.2.3',
      body: 'Release notes',
      downloadAndInstall,
    });
    showErrorMock.mockResolvedValueOnce('primary');

    render(<UpdateNotification />);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
      await Promise.resolve();
    });

    expect(screen.getByText('settings.update_available:1.2.3')).toBeDefined();

    await act(async () => {
      fireEvent.click(screen.getByText('settings.update_btn_install'));
      await Promise.resolve();
    });

    expect(downloadAndInstall).toHaveBeenCalled();
    expect(showErrorMock).toHaveBeenCalled();
    expect(openUrlMock).toHaveBeenCalledWith('https://github.com/AirSodaz/sona/releases/latest');
  });
});
