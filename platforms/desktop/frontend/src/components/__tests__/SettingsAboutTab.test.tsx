import { act, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  clearUptodateResetTimer,
  UPTODATE_RESET_DELAY_MS,
  useAppUpdaterStore,
} from '../../stores/appUpdaterStore';
import { SettingsAboutTab } from '../settings/SettingsAboutTab';

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
    t: (key: string, options?: Record<string, unknown>) =>
      options?.version ? `${key}:${options.version}` : key,
  }),
  initReactI18next: {
    type: '3rdParty',
    init: () => undefined,
  },
}));

vi.mock('../../i18n', () => ({
  default: {
    t: (key: string, options?: Record<string, unknown>) =>
      options?.version ? `${key}:${options.version}` : key,
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
  clearUptodateResetTimer();
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
  afterEach(() => {
    clearUptodateResetTimer();
    vi.useRealTimers();
  });

  it('shows up-to-date message on manual check and automatically reverts to check button', async () => {
    vi.useFakeTimers();
    checkMock.mockResolvedValueOnce(null);

    render(<SettingsAboutTab />);

    expect(screen.getByRole('button', { name: 'settings.about_check_updates' })).toBeDefined();

    await act(async () => {
      screen.getByRole('button', { name: 'settings.about_check_updates' }).click();
      await Promise.resolve();
    });

    expect(screen.getByText('settings.update_not_available')).toBeDefined();
    expect(screen.queryByRole('button', { name: 'settings.about_check_updates' })).toBeNull();

    act(() => {
      vi.advanceTimersByTime(UPTODATE_RESET_DELAY_MS);
    });

    expect(screen.getByRole('button', { name: 'settings.about_check_updates' })).toBeDefined();
    expect(screen.queryByText('settings.update_not_available')).toBeNull();
  });

  it('resets uptodate status back to idle when component unmounts', () => {
    useAppUpdaterStore.setState({ status: 'uptodate' });

    const { unmount } = render(<SettingsAboutTab />);

    expect(useAppUpdaterStore.getState().status).toBe('uptodate');

    unmount();

    expect(useAppUpdaterStore.getState().status).toBe('idle');
  });

  it('renders app title and version badge containing version, divider, and channel', () => {
    const { container } = render(<SettingsAboutTab />);

    expect(screen.getByRole('heading', { level: 2, name: 'Sona' })).toBeDefined();
    const versionBadge = container.querySelector('.about-version-badge');
    expect(versionBadge).not.toBeNull();
    expect(versionBadge?.querySelector('.about-version-text')?.textContent).toMatch(
      /^v\d+\.\d+\.\d+/
    );
    expect(versionBadge?.querySelector('.about-version-divider')?.textContent).toBe('/');
    expect(versionBadge?.querySelector('.about-channel-text')?.textContent).toBe('Stable');
  });

  it('shows the available update after a manual trigger even if the toast was dismissed', async () => {
    useAppUpdaterStore.setState({
      dismissedVersion: '1.2.3',
      notificationVisible: false,
    });
    checkMock.mockResolvedValueOnce(makeUpdate('1.2.3'));

    render(<SettingsAboutTab />);

    await act(async () => {
      screen.getByRole('button', { name: 'settings.about_check_updates' }).click();
      await Promise.resolve();
    });

    await waitFor(() => {
      expect(checkMock).toHaveBeenCalledWith();
      screen.getByText('settings.update_available:1.2.3');
    });

    expect(useAppUpdaterStore.getState().notificationVisible).toBe(false);
    expect(useAppUpdaterStore.getState().status).toBe('available');
  });
});
