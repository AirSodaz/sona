import { beforeEach, describe, expect, it, vi } from 'vitest';
import { migrateConfig } from '../configMigrationService';
import { DEFAULT_CONFIG } from '../../stores/configStore';

const mockSet = vi.fn();
const mockSave = vi.fn();

vi.mock('../storageService', () => ({
  settingsStore: {
    set: (...args: unknown[]) => mockSet(...args),
    save: (...args: unknown[]) => mockSave(...args),
  },
  STORE_KEY_CONFIG: 'sona-config',
}));

describe('configMigrationService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
  });

  it('normalizes missing summaryEnabled to true for existing configs', async () => {
    const savedConfig = {
      ...DEFAULT_CONFIG,
      summaryEnabled: undefined,
    };

    const result = await migrateConfig(savedConfig);

    expect(result.config.summaryEnabled).toBe(true);
    expect(result.migrated).toBe(true);
    expect(mockSet).toHaveBeenCalledWith('sona-config', expect.objectContaining({
      summaryEnabled: true,
    }));
    expect(mockSave).toHaveBeenCalled();
  });

  it('preserves an explicitly disabled summary setting without forcing migration', async () => {
    const savedConfig = {
      ...DEFAULT_CONFIG,
      summaryEnabled: false,
    };

    const result = await migrateConfig(savedConfig);

    expect(result.config.summaryEnabled).toBe(false);
  });
});
