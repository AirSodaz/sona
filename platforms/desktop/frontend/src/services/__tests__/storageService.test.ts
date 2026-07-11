import { beforeEach, describe, expect, it, vi } from 'vitest';

const loadAppConfigMock = vi.fn();
const saveAppConfigMock = vi.fn();
const getAppSettingMock = vi.fn();
const setAppSettingMock = vi.fn();
const legacyGetMock = vi.fn();
const legacySetMock = vi.fn();
const legacySaveMock = vi.fn();
const legacyOnKeyChangeMock = vi.fn();
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

vi.mock('@tauri-apps/plugin-store', () => ({
  LazyStore: vi.fn().mockImplementation(function LazyStore() {
    return {
      get: (...args: unknown[]) => legacyGetMock(...args),
      set: (...args: unknown[]) => legacySetMock(...args),
      save: (...args: unknown[]) => legacySaveMock(...args),
      onKeyChange: (...args: unknown[]) => legacyOnKeyChangeMock(...args),
    };
  }),
}));

describe('settingsStore SQLite adapter', () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
    loadAppConfigMock.mockResolvedValue(null);
    saveAppConfigMock.mockResolvedValue(undefined);
    getAppSettingMock.mockResolvedValue(null);
    setAppSettingMock.mockResolvedValue(undefined);
    legacyGetMock.mockResolvedValue(null);
    legacySetMock.mockResolvedValue(undefined);
    legacySaveMock.mockResolvedValue(undefined);
    legacyOnKeyChangeMock.mockResolvedValue(() => undefined);
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

  it('falls back to legacy settings.json once and backfills SQLite settings', async () => {
    const { settingsStore, STORE_KEY_ONBOARDING } = await import('../storageService');
    const onboarding = { version: 1, status: 'completed' };
    getAppSettingMock.mockResolvedValueOnce(null);
    legacyGetMock.mockResolvedValueOnce(onboarding);

    await expect(settingsStore.get(STORE_KEY_ONBOARDING)).resolves.toEqual(onboarding);

    expect(legacyGetMock).toHaveBeenCalledWith(STORE_KEY_ONBOARDING);
    expect(setAppSettingMock).toHaveBeenCalledWith(STORE_KEY_ONBOARDING, onboarding);
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
});
