import { beforeEach, describe, expect, it } from 'vitest';
import type { AppConfig } from '../../../types/config';
import type { EmbyToolOverrides } from '../../../types/emby';
import {
  buildEmbyEffectiveConfig,
  DEFAULT_EMBY_TOOL_OVERRIDES,
  EMBY_TOOL_OVERRIDES_STORAGE_KEY,
  loadEmbyToolOverrides,
  saveEmbyToolOverrides,
} from '../embySettingsService';

const mockBaseConfig: AppConfig = {
  language: 'zh-CN',
  asr: {
    selectedModel: 'base',
    selections: {
      live: { engine: 'local', mode: 'streaming', modelId: 'base', modelPath: '/path/base' },
      batch: { engine: 'local', mode: 'batch', modelId: 'base', modelPath: '/path/base' },
      caption: { engine: 'local', mode: 'streaming', modelId: 'base', modelPath: '/path/base' },
      voiceTyping: { engine: 'local', mode: 'streaming', modelId: 'base', modelPath: '/path/base' },
    },
    localModels: [],
  },
  llmSettings: {
    selections: {
      defaultModelId: 'gpt-4o',
      translationModelId: 'gpt-4o',
    },
    models: [
      { id: 'gpt-4o', name: 'GPT-4o', provider: 'openai', model: 'gpt-4o', enabled: true },
      {
        id: 'deepseek-v3',
        name: 'DeepSeek-V3',
        provider: 'deepseek',
        model: 'deepseek-chat',
        enabled: true,
      },
    ],
  },
  translationLanguage: 'zh-CN',
} as unknown as AppConfig;

describe('embySettingsService', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  describe('loadEmbyToolOverrides and saveEmbyToolOverrides', () => {
    it('returns default overrides when nothing is stored', () => {
      const overrides = loadEmbyToolOverrides();
      expect(overrides).toEqual(DEFAULT_EMBY_TOOL_OVERRIDES);
      expect(overrides.asrLanguage).toBe('auto');
      expect(overrides.defaultSubtitleMode).toBe('bilingual');
      expect(overrides.overwriteExistingExternal).toBe(true);
    });

    it('saves and loads overrides correctly', () => {
      const customOverrides: EmbyToolOverrides = {
        asrSelection: {
          engine: 'online',
          mode: 'batch',
          providerId: 'volcengine-doubao',
          modelId: 'doubao-pro',
          modelPath: '',
        },
        asrLanguage: 'ja',
        translationModelId: 'deepseek-v3',
        defaultSubtitleMode: 'pure_translation',
        overwriteExistingExternal: false,
      };

      saveEmbyToolOverrides(customOverrides);
      const loaded = loadEmbyToolOverrides();

      expect(loaded).toEqual(customOverrides);
      expect(localStorage.getItem(EMBY_TOOL_OVERRIDES_STORAGE_KEY)).toContain('deepseek-v3');
    });

    it('recovers gracefully from invalid JSON in localStorage', () => {
      localStorage.setItem(EMBY_TOOL_OVERRIDES_STORAGE_KEY, '{invalid_json}');
      const loaded = loadEmbyToolOverrides();
      expect(loaded).toEqual(DEFAULT_EMBY_TOOL_OVERRIDES);
    });
  });

  describe('buildEmbyEffectiveConfig', () => {
    it('overrides batch ASR selection in snapshot', () => {
      const overrides: EmbyToolOverrides = {
        asrSelection: {
          engine: 'online',
          mode: 'batch',
          providerId: 'volcengine-doubao',
          modelId: 'doubao-pro',
          modelPath: '',
        },
      };

      const effectiveConfig = buildEmbyEffectiveConfig(mockBaseConfig, overrides);

      expect(effectiveConfig.asr.selections.batch).toEqual(overrides.asrSelection);
      // Realtime selection should remain untouched
      expect(effectiveConfig.asr.selections.live).toEqual(mockBaseConfig.asr.selections.live);
      // Original config not mutated
      expect(mockBaseConfig.asr.selections.batch.engine).toBe('local');
    });

    it('strictly honors asrLanguage override (including auto)', () => {
      const overridesJa: EmbyToolOverrides = { asrLanguage: 'ja' };
      const configJa = buildEmbyEffectiveConfig(mockBaseConfig, overridesJa);
      expect(configJa.language).toBe('ja');

      const overridesAuto: EmbyToolOverrides = { asrLanguage: 'auto' };
      const configAuto = buildEmbyEffectiveConfig(mockBaseConfig, overridesAuto);
      expect(configAuto.language).toBe('auto');
    });

    it('overrides translationModelId in snapshot', () => {
      const overrides: EmbyToolOverrides = {
        translationModelId: 'deepseek-v3',
      };

      const effectiveConfig = buildEmbyEffectiveConfig(mockBaseConfig, overrides);
      expect(effectiveConfig.llmSettings?.selections.translationModelId).toBe('deepseek-v3');
      expect(mockBaseConfig.llmSettings?.selections.translationModelId).toBe('gpt-4o');
    });

    it('preserves existing settings when overrides are empty or undefined', () => {
      const effectiveConfig = buildEmbyEffectiveConfig(mockBaseConfig, {});
      expect(effectiveConfig.language).toBe(mockBaseConfig.language);
      expect(effectiveConfig.asr.selections.batch).toEqual(mockBaseConfig.asr.selections.batch);
      expect(effectiveConfig.llmSettings?.selections.translationModelId).toBe(
        mockBaseConfig.llmSettings?.selections.translationModelId
      );
    });
  });
});
