import { beforeEach, describe, expect, it, vi } from 'vitest';
import { settingsStore } from '../../services/storageService';
import { emit } from '../../services/tauri/platform/events';
import { DEFAULT_CONFIG } from '../../stores/configStore';
import { flushConfigPersistence, scheduleConfigPersistence } from '../useConfigPersistence';

vi.mock('../../services/storageService', () => ({
  STORE_KEY_CONFIG: 'config',
  settingsStore: {
    set: vi.fn().mockResolvedValue(undefined),
  },
}));

vi.mock('../../services/tauri/platform/events', () => ({
  emit: vi.fn().mockResolvedValue(undefined),
}));

describe('useConfigPersistence', () => {
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
});
