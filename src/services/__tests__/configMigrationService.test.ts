import { beforeEach, describe, expect, it } from 'vitest';
import { migrateConfig } from '../configMigrationService';
import { DEFAULT_CONFIG } from '../../stores/configStore';

describe('configMigrationService', () => {
  beforeEach(() => {
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
  });

  it('preserves an explicitly disabled summary setting without forcing migration', async () => {
    const savedConfig = {
      ...DEFAULT_CONFIG,
      summaryEnabled: false,
    };

    const result = await migrateConfig(savedConfig);

    expect(result.config.summaryEnabled).toBe(false);
  });

  it('fills missing summary template registry fields with defaults', async () => {
    const savedConfig: any = {
      ...DEFAULT_CONFIG,
      configVersion: 4,
      summaryTemplateId: undefined,
      summaryCustomTemplates: undefined,
    };

    const result = await migrateConfig(savedConfig);

    expect(result.config.summaryTemplateId).toBe('general');
    expect(result.config.summaryCustomTemplates).toEqual([]);
    expect(result.migrated).toBe(true);
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

  it('fills missing speaker model and profile fields with defaults', async () => {
    const savedConfig: any = {
      ...DEFAULT_CONFIG,
      configVersion: 5,
      speakerProfiles: undefined,
      speakerSegmentationModelPath: undefined,
      speakerEmbeddingModelPath: undefined,
    };

    const result = await migrateConfig(savedConfig);

    expect(result.config.speakerProfiles).toEqual([]);
    expect(result.config.speakerSegmentationModelPath).toBe('');
    expect(result.config.speakerEmbeddingModelPath).toBe('');
    expect(result.migrated).toBe(true);
  });

  it('accepts a legacy config object as pure input without relying on storage side effects', async () => {
    const legacyConfig = {
      modelPath: '/legacy/model.onnx',
      recognitionModelPath: '/legacy/model.onnx',
    } as any;

    const result = await migrateConfig(null, legacyConfig);

    expect(result.config.streamingModelPath).toBe('/legacy/model.onnx');
    expect(result.config.offlineModelPath).toBe('/legacy/model.onnx');
    expect(result.migrated).toBe(true);
  });
});
