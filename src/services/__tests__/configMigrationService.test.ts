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

  it('migrates a legacy built-in polish scenario to polishPresetId', async () => {
    const savedConfig: any = {
      ...DEFAULT_CONFIG,
      configVersion: 2,
      polishPresetId: undefined,
      polishCustomPresets: undefined,
      polishScenario: 'meeting',
      polishContext: '',
    };

    const result = await migrateConfig(savedConfig);

    expect(result.config.polishPresetId).toBe('meeting');
    expect(result.config.polishCustomPresets).toEqual([]);
    expect(result.migrated).toBe(true);
  });

  it('maps a legacy empty custom context to the general preset', async () => {
    const savedConfig: any = {
      ...DEFAULT_CONFIG,
      configVersion: 2,
      polishPresetId: undefined,
      polishCustomPresets: undefined,
      polishScenario: 'custom',
      polishContext: '',
    };

    const result = await migrateConfig(savedConfig);

    expect(result.config.polishPresetId).toBe('general');
    expect(result.config.polishCustomPresets).toEqual([]);
  });

  it('imports a legacy non-empty custom context into custom presets', async () => {
    const savedConfig: any = {
      ...DEFAULT_CONFIG,
      configVersion: 2,
      polishPresetId: undefined,
      polishCustomPresets: undefined,
      polishScenario: 'custom',
      polishContext: 'Focus on investor updates and roadmap terms.',
    };

    const result = await migrateConfig(savedConfig);

    expect(result.config.polishCustomPresets).toHaveLength(1);
    expect(result.config.polishCustomPresets?.[0]).toEqual(expect.objectContaining({
      context: 'Focus on investor updates and roadmap terms.',
    }));
    expect(result.config.polishPresetId).toBe(result.config.polishCustomPresets?.[0].id);
  });

  it('migrates an empty legacy polishKeywords string to an empty keyword set array', async () => {
    const savedConfig: any = {
      ...DEFAULT_CONFIG,
      configVersion: 3,
      polishKeywordSets: undefined,
      polishKeywords: '',
    };

    const result = await migrateConfig(savedConfig);

    expect(result.config.polishKeywordSets).toEqual([]);
    expect(result.config.polishKeywords).toBe('');
  });

  it('imports a legacy non-empty polishKeywords string into an enabled keyword set', async () => {
    const savedConfig: any = {
      ...DEFAULT_CONFIG,
      configVersion: 3,
      polishKeywordSets: undefined,
      polishKeywords: 'Sona\nSherpa-onnx',
    };

    const result = await migrateConfig(savedConfig);

    expect(result.config.polishKeywordSets).toHaveLength(1);
    expect(result.config.polishKeywordSets?.[0]).toEqual(expect.objectContaining({
      enabled: true,
      keywords: 'Sona\nSherpa-onnx',
    }));
    expect(result.config.polishKeywords).toBe('');
  });
});
