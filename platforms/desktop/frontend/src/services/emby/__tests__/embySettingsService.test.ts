import { beforeEach, describe, expect, it, vi } from 'vitest';
import { DEFAULT_CONFIG, useConfigStore } from '../../../stores/configStore';
import type { AppConfig } from '../../../types/config';
import type { EmbyToolOverrides } from '../../../types/emby';
import * as embyService from '../embyService';
import { cleanSubtitleText } from '../embyService';
import {
  buildEmbyEffectiveConfig,
  DEFAULT_EMBY_TOOL_OVERRIDES,
  EMBY_TOOL_OVERRIDES_STORAGE_KEY,
  loadEmbyToolOverrides,
  loadEmbyToolOverridesAsync,
  normalizeEmbyToolOverrides,
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
    useConfigStore.setState({ config: { ...DEFAULT_CONFIG } });
    vi.restoreAllMocks();
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

    it('guards against null properties in stored JSON', () => {
      localStorage.setItem(
        EMBY_TOOL_OVERRIDES_STORAGE_KEY,
        JSON.stringify({ defaultSubtitleMode: null, overwriteExistingExternal: null })
      );
      const loaded = loadEmbyToolOverrides();
      expect(loaded.defaultSubtitleMode).toBe('bilingual');
      expect(loaded.overwriteExistingExternal).toBe(true);
    });

    it('loads from configStore when localStorage has no entry', () => {
      useConfigStore.setState({
        config: {
          ...DEFAULT_CONFIG,
          embyProcessingSettings: {
            asrLanguage: 'ko',
            defaultSubtitleMode: 'separate_tracks',
            overwriteExistingExternal: false,
          },
        },
      });
      const loaded = loadEmbyToolOverrides();
      expect(loaded.asrLanguage).toBe('ko');
      expect(loaded.defaultSubtitleMode).toBe('separate_tracks');
      expect(loaded.overwriteExistingExternal).toBe(false);
    });

    it('updates configStore and invokes saveEmbyProcessingSettings on save', async () => {
      const saveSpy = vi
        .spyOn(embyService, 'saveEmbyProcessingSettings')
        .mockResolvedValue(undefined);
      const custom: EmbyToolOverrides = {
        asrLanguage: 'de',
        defaultSubtitleMode: 'bilingual',
        overwriteExistingExternal: true,
      };

      saveEmbyToolOverrides(custom);

      expect(useConfigStore.getState().config.embyProcessingSettings?.asrLanguage).toBe('de');
      expect(saveSpy).toHaveBeenCalledWith(normalizeEmbyToolOverrides(custom));
    });

    it('loads overrides asynchronously from backend and syncs configStore and localStorage', async () => {
      const backendOverrides: EmbyToolOverrides = {
        asrLanguage: 'fr',
        defaultSubtitleMode: 'pure_translation',
        overwriteExistingExternal: false,
      };
      vi.spyOn(embyService, 'getEmbyProcessingSettings').mockResolvedValue(backendOverrides);

      const loaded = await loadEmbyToolOverridesAsync();

      expect(loaded.asrLanguage).toBe('fr');
      expect(useConfigStore.getState().config.embyProcessingSettings?.asrLanguage).toBe('fr');
      expect(localStorage.getItem(EMBY_TOOL_OVERRIDES_STORAGE_KEY)).toContain('pure_translation');
    });
  });

  describe('cleanSubtitleText', () => {
    it('converts p and div closings to newlines, strips html, and eliminates blank lines', () => {
      const input = '<div>Line 1</div><p>Line 2</p><br/>\n\nLine 3<font color="red"> Red</font>';
      expect(cleanSubtitleText(input)).toBe('Line 1\nLine 2\nLine 3 Red');
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
