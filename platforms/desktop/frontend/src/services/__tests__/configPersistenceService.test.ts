import { beforeEach, describe, expect, it, vi } from 'vitest';
import { DEFAULT_CONFIG, useConfigStore } from '../../stores/configStore';
import {
  flushConfigPersistence,
  persistConfigImmediately,
  scheduleConfigPersistence,
} from '../configPersistenceService';
import { settingsStore } from '../storageService';
import { emit } from '../tauri/platform/events';

vi.mock('../storageService', () => ({
  STORE_KEY_CONFIG: 'config',
  settingsStore: {
    set: vi.fn().mockResolvedValue(undefined),
  },
}));

vi.mock('../tauri/platform/events', () => ({
  emit: vi.fn().mockResolvedValue(undefined),
}));

describe('configPersistenceService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('flushes pending config changes immediately on flushConfigPersistence', async () => {
    const updatedConfig = { ...DEFAULT_CONFIG, language: 'zh' };
    scheduleConfigPersistence(updatedConfig);

    await flushConfigPersistence();

    expect(settingsStore.set).toHaveBeenCalledWith('config', updatedConfig);
    expect(emit).toHaveBeenCalledWith('asr-config-updated');
  });

  it('no-ops when there is no pending config to flush', async () => {
    await flushConfigPersistence();
    expect(settingsStore.set).not.toHaveBeenCalled();
  });

  it('updates config store and persists immediately on persistConfigImmediately', async () => {
    await persistConfigImmediately({ language: 'ja' });

    expect(useConfigStore.getState().config.language).toBe('ja');
    expect(settingsStore.set).toHaveBeenCalledWith(
      'config',
      expect.objectContaining({ language: 'ja' })
    );
    expect(emit).toHaveBeenCalledWith('asr-config-updated');
  });
});
