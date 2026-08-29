import { beforeEach, describe, expect, it, vi } from 'vitest';

const loadAppConfigMock = vi.fn();
const saveAppConfigMock = vi.fn();
const getAppSettingMock = vi.fn();
const setAppSettingMock = vi.fn();
const emitMock = vi.fn();
const listenMock = vi.fn();

vi.mock('../tauri/app', () => ({
  loadAppConfig: (...args: unknown[]) => loadAppConfigMock(...args),
  saveAppConfig: (...args: unknown[]) => saveAppConfigMock(...args),
  getAppSetting: (...args: unknown[]) => getAppSettingMock(...args),
  setAppSetting: (...args: unknown[]) => setAppSettingMock(...args),
}));

vi.mock('../tauri/platform/events', () => ({
  emit: (...args: unknown[]) => emitMock(...args),
  listen: (...args: unknown[]) => listenMock(...args),
}));

describe('settingsStore SQLite adapter', () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
    loadAppConfigMock.mockResolvedValue(null);
    saveAppConfigMock.mockResolvedValue(undefined);
    getAppSettingMock.mockResolvedValue(null);
    setAppSettingMock.mockResolvedValue(undefined);
    emitMock.mockResolvedValue(undefined);
    listenMock.mockResolvedValue(() => undefined);
  });

  it('loads sona-config from app_config and writes config through SQLite', async () => {
    const { settingsStore, STORE_KEY_CONFIG } = await import('../storageService');
    const config = { configVersion: 7, theme: 'dark' };
    loadAppConfigMock.mockResolvedValueOnce(config);

    await expect(settingsStore.get(STORE_KEY_CONFIG)).resolves.toEqual(config);
    await settingsStore.set(STORE_KEY_CONFIG, config);
    await settingsStore.save();

    expect(loadAppConfigMock).toHaveBeenCalledTimes(1);
    expect(saveAppConfigMock).toHaveBeenCalledWith(config);
    expect(setAppSettingMock).not.toHaveBeenCalled();
  });

  it('notifies onKeyChange subscribers from SQLite setting events', async () => {
    const { settingsStore, STORE_KEY_CONFIG } = await import('../storageService');
    const callback = vi.fn();
    let listener: ((event: { payload: unknown }) => void) | undefined;
    listenMock.mockImplementation(async (_eventName: string, cb: typeof listener) => {
      listener = cb;
      return () => undefined;
    });

    await settingsStore.onKeyChange(STORE_KEY_CONFIG, callback);
    listener?.({ payload: { key: STORE_KEY_CONFIG, value: { theme: 'light' } } });
    listener?.({ payload: { key: 'sona-onboarding', value: { status: 'completed' } } });

    expect(callback).toHaveBeenCalledTimes(1);
    expect(callback).toHaveBeenCalledWith({ theme: 'light' });
  });

  it('broadcasts an externally committed config without writing it again', async () => {
    const { settingsStore, STORE_KEY_CONFIG } = await import('../storageService');
    const config = { configVersion: 7, theme: 'dark' };

    await settingsStore.notifyExternalUpdate(STORE_KEY_CONFIG, config);

    expect(emitMock).toHaveBeenCalledWith('app-setting-updated', {
      key: STORE_KEY_CONFIG,
      value: config,
    });
    expect(saveAppConfigMock).not.toHaveBeenCalled();
    expect(setAppSettingMock).not.toHaveBeenCalled();
  });
});
