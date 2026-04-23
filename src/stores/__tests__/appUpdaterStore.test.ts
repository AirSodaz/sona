import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useAppUpdaterStore } from '../appUpdaterStore';

const checkMock = vi.fn();
const relaunchMock = vi.fn();
const openUrlMock = vi.fn();
const showErrorMock = vi.fn();

vi.mock('@tauri-apps/plugin-updater', () => ({
  check: (...args: unknown[]) => checkMock(...args),
}));

vi.mock('@tauri-apps/plugin-process', () => ({
  relaunch: (...args: unknown[]) => relaunchMock(...args),
}));

vi.mock('@tauri-apps/plugin-opener', () => ({
  openUrl: (...args: unknown[]) => openUrlMock(...args),
}));

vi.mock('../../i18n', () => ({
  default: {
    t: (key: string) => key,
  },
}));

vi.mock('../errorDialogStore', () => ({
  useErrorDialogStore: {
    getState: () => ({
      showError: showErrorMock,
    }),
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

describe('appUpdaterStore', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetUpdaterStore();
  });

  it('shows the notification after an automatic update check finds a new version', async () => {
    checkMock.mockResolvedValueOnce(makeUpdate('1.2.3'));

    await useAppUpdaterStore.getState().checkUpdate(false);

    expect(useAppUpdaterStore.getState().status).toBe('available');
    expect(useAppUpdaterStore.getState().notificationVisible).toBe(true);
    expect(useAppUpdaterStore.getState().hasAutoCheckedThisSession).toBe(true);
  });

  it('does not reopen the toast for the same version after it was dismissed in this session', async () => {
    checkMock.mockResolvedValueOnce(makeUpdate('1.2.3'));

    await useAppUpdaterStore.getState().checkUpdate(false);
    useAppUpdaterStore.getState().dismissNotification();

    checkMock.mockResolvedValueOnce(makeUpdate('1.2.3'));
    useAppUpdaterStore.setState({ hasAutoCheckedThisSession: false });

    await useAppUpdaterStore.getState().checkUpdate(false);

    expect(useAppUpdaterStore.getState().dismissedVersion).toBe('1.2.3');
    expect(useAppUpdaterStore.getState().notificationVisible).toBe(false);
    expect(useAppUpdaterStore.getState().status).toBe('available');
  });

  it('keeps manual checks usable after the toast was dismissed', async () => {
    checkMock.mockResolvedValueOnce(makeUpdate('1.2.3'));

    await useAppUpdaterStore.getState().checkUpdate(false);
    useAppUpdaterStore.getState().dismissNotification();

    checkMock.mockResolvedValueOnce(makeUpdate('1.2.3'));

    await useAppUpdaterStore.getState().checkUpdate(true);

    expect(useAppUpdaterStore.getState().status).toBe('available');
    expect(useAppUpdaterStore.getState().notificationVisible).toBe(false);
    expect(useAppUpdaterStore.getState().updateInfo?.version).toBe('1.2.3');
  });

  it('shows the toast again when a newer version appears later in the same session', async () => {
    checkMock.mockResolvedValueOnce(makeUpdate('1.2.3'));

    await useAppUpdaterStore.getState().checkUpdate(false);
    useAppUpdaterStore.getState().dismissNotification();

    checkMock.mockResolvedValueOnce(makeUpdate('1.2.4'));
    useAppUpdaterStore.setState({ hasAutoCheckedThisSession: false });

    await useAppUpdaterStore.getState().checkUpdate(false);

    expect(useAppUpdaterStore.getState().notificationVisible).toBe(true);
    expect(useAppUpdaterStore.getState().updateInfo?.version).toBe('1.2.4');
  });

  it('keeps automatic check failures silent instead of surfacing an error state', async () => {
    checkMock.mockRejectedValueOnce(new Error('network timeout'));

    await useAppUpdaterStore.getState().checkUpdate(false);

    expect(useAppUpdaterStore.getState().status).toBe('idle');
    expect(useAppUpdaterStore.getState().error).toBe('network timeout');
    expect(showErrorMock).not.toHaveBeenCalled();
  });

  it('returns to available after an installation failure so the user can retry', async () => {
    const downloadAndInstall = vi.fn().mockRejectedValue(new Error('network timeout'));
    useAppUpdaterStore.setState({
      status: 'available',
      updateInfo: makeUpdate('1.2.3', { downloadAndInstall }) as any,
      notificationVisible: true,
    });
    showErrorMock.mockResolvedValueOnce('dismiss');

    await useAppUpdaterStore.getState().installUpdate();

    expect(downloadAndInstall).toHaveBeenCalled();
    expect(useAppUpdaterStore.getState().status).toBe('available');
    expect(useAppUpdaterStore.getState().notificationVisible).toBe(true);
    expect(showErrorMock).toHaveBeenCalled();
  });

  it('enters the downloaded state and can relaunch the app', async () => {
    const downloadAndInstall = vi.fn().mockResolvedValue(undefined);
    useAppUpdaterStore.setState({
      status: 'available',
      updateInfo: makeUpdate('1.2.3', { downloadAndInstall }) as any,
      notificationVisible: true,
    });

    await useAppUpdaterStore.getState().installUpdate();

    expect(useAppUpdaterStore.getState().status).toBe('downloaded');
    expect(useAppUpdaterStore.getState().progress).toBe(100);

    await useAppUpdaterStore.getState().relaunchToUpdate();

    expect(relaunchMock).toHaveBeenCalled();
  });
});
