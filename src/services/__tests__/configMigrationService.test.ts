import { beforeEach, describe, expect, it, vi } from 'vitest';
import { migrateConfig } from '../configMigrationService';
import { DEFAULT_CONFIG } from '../../stores/configStore';
import type { AppConfig } from '../../types/config';
import { migrateAppConfig } from '../tauri/app';

vi.mock('../../i18n', () => ({
  default: {
    t: vi.fn((key: string, options?: { defaultValue?: string }) => {
      if (key === 'settings.default_rule_set_name') {
        return 'Default Rules';
      }
      return options?.defaultValue ?? key;
    }),
  },
}));

vi.mock('../tauri/app', () => ({
  migrateAppConfig: vi.fn(),
}));

describe('configMigrationService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('delegates saved and legacy config normalization to Rust with the localized default rule-set name', async () => {
    const savedConfig: AppConfig = {
      ...DEFAULT_CONFIG,
      logLevel: 'debug',
    };
    const legacyConfig = {
      recognitionModelPath: '/legacy/model',
    };
    vi.mocked(migrateAppConfig).mockResolvedValueOnce({
      config: {
        ...savedConfig,
        streamingModelPath: '/legacy/model',
      },
      migrated: true,
    });

    const result = await migrateConfig(savedConfig, legacyConfig);

    expect(result.migrated).toBe(true);
    expect(result.config.streamingModelPath).toBe('/legacy/model');
    expect(migrateAppConfig).toHaveBeenCalledWith(
      savedConfig,
      legacyConfig,
      'Default Rules',
    );
  });

  it('passes nulls for missing startup config inputs', async () => {
    vi.mocked(migrateAppConfig).mockResolvedValueOnce({
      config: { ...DEFAULT_CONFIG },
      migrated: false,
    });

    await migrateConfig(undefined);

    expect(migrateAppConfig).toHaveBeenCalledWith(null, null, 'Default Rules');
  });
});
