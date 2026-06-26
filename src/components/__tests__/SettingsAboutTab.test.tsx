import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SettingsAboutTab } from '../settings/SettingsAboutTab';
import { useAppUpdaterStore } from '../../stores/appUpdaterStore';
import { useConfigStore } from '../../stores/configStore';

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
    crossChannelDownloadUrl: null,
  });
}

function resetConfigStore() {
  useConfigStore.setState({ config: { ...useConfigStore.getState().config, channel: 'stable' } });
}

describe('SettingsAboutTab', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    runGuardedQuitMock.mockReset();
    resetUpdaterStore();
    resetConfigStore();
  });

  it('renders channel selector with both stable and nightly options', () => {
    render(<SettingsAboutTab />);
    const select = screen.getByRole('combobox', { name: 'settings.channel' });
    expect(select).toBeDefined();
    expect(screen.getByText('settings.channel_stable')).toBeDefined();
    expect(screen.getByText('settings.channel_nightly')).toBeDefined();
  });

  it('shows confirmation dialog when switching from stable to nightly', () => {
    render(<SettingsAboutTab />);
    const select = screen.getByRole('combobox', { name: 'settings.channel' });
    fireEvent.change(select, { target: { value: 'nightly' } });
    expect(screen.getByText('settings.channel_switch_confirm_title')).toBeDefined();
    expect(screen.getByText('settings.channel_switch_confirm_body')).toBeDefined();
  });

  it('canceling confirmation dialog hides it and does not change channel', async () => {
    render(<SettingsAboutTab />);
    const select = screen.getByRole('combobox', { name: 'settings.channel' });
    fireEvent.change(select, { target: { value: 'nightly' } });
    await act(async () => {
      screen.getByText('settings.channel_switch_confirm_cancel').click();
    });
    expect(screen.queryByText('settings.channel_switch_confirm_title')).toBeNull();
    expect(useConfigStore.getState().config.channel).toBe('stable');
  });

  it('confirming switch changes channel to nightly', async () => {
    render(<SettingsAboutTab />);
    const select = screen.getByRole('combobox', { name: 'settings.channel' });
    fireEvent.change(select, { target: { value: 'nightly' } });
    await act(async () => {
      screen.getByText('settings.channel_switch_confirm_confirm').click();
    });
    expect(screen.queryByText('settings.channel_switch_confirm_title')).toBeNull();
    expect(useConfigStore.getState().config.channel).toBe('nightly');
  });

  it('switching from nightly to stable does not show confirmation dialog', () => {
    useConfigStore.setState({ config: { ...useConfigStore.getState().config, channel: 'nightly' } });
    render(<SettingsAboutTab />);
    const select = screen.getByRole('combobox', { name: 'settings.channel' });
    fireEvent.change(select, { target: { value: 'stable' } });
    expect(screen.queryByText('settings.channel_switch_confirm_title')).toBeNull();
    expect(useConfigStore.getState().config.channel).toBe('stable');
  });

  it('renders cross-channel download button when crossChannelDownloadUrl is set', () => {
    useAppUpdaterStore.setState({
      status: 'available',
      crossChannelDownloadUrl: 'https://example.com/download',
      updateInfo: null,
    });
    render(<SettingsAboutTab />);
    expect(screen.getByText('settings.channel_download_stable')).toBeDefined();
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
      expect(checkMock).toHaveBeenCalledWith({ endpoints: undefined });
      expect(screen.getByText('settings.update_available:1.2.3')).toBeDefined();
    });

    expect(useAppUpdaterStore.getState().notificationVisible).toBe(false);
    expect(useAppUpdaterStore.getState().status).toBe('available');
  });
});
