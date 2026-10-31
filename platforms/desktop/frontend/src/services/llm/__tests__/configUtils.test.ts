import { describe, expect, it } from 'vitest';
import {
  getFeatureLlmConfig,
  isFeatureLlmConfigComplete,
  isSummaryLlmConfigComplete,
} from '../configUtils';
import { DEFAULT_LLM_TEMPERATURE } from '../providers';
import {
  addLlmModel,
  buildLlmConfigPatch,
  createLlmSettings,
  setFeatureModelSelection,
  setFeatureReasoningEnabled,
  setFeatureReasoningLevel,
  setFeatureTemperature,
  updateProviderSetting,
} from '../state';

describe('llm runtime', () => {
  it('resolves feature configs independently', () => {
    let llmSettings = createLlmSettings();
    llmSettings = updateProviderSetting(llmSettings, 'open_ai', {
      apiHost: 'https://api.openai.com',
      apiKey: 'openai-key',
    });
    llmSettings = updateProviderSetting(llmSettings, 'anthropic', {
      apiHost: 'https://api.anthropic.com',
      apiKey: 'anthropic-key',
    });
    llmSettings = addLlmModel(llmSettings, { provider: 'open_ai', model: 'gpt-4o-mini' });
    llmSettings = addLlmModel(llmSettings, {
      provider: 'anthropic',
      model: 'claude-sonnet-4-20250514',
    });
    llmSettings = setFeatureModelSelection(llmSettings, 'polish', llmSettings.modelOrder[0]);
    llmSettings = setFeatureModelSelection(llmSettings, 'translation', llmSettings.modelOrder[1]);
    llmSettings = setFeatureModelSelection(llmSettings, 'summary', llmSettings.modelOrder[0]);

    const config = buildLlmConfigPatch(llmSettings);

    expect(getFeatureLlmConfig(config, 'polish')).toEqual(
      expect.objectContaining({
        provider: 'open_ai',
        apiKey: 'openai-key',
        model: 'gpt-4o-mini',
      })
    );
    expect(getFeatureLlmConfig(config, 'translation')).toEqual(
      expect.objectContaining({
        provider: 'anthropic',
        apiKey: 'anthropic-key',
        model: 'claude-sonnet-4-20250514',
      })
    );
    expect(getFeatureLlmConfig(config, 'summary')).toEqual(
      expect.objectContaining({
        provider: 'open_ai',
        apiKey: 'openai-key',
        model: 'gpt-4o-mini',
      })
    );
  });

  it('resolves feature-specific temperatures independently', () => {
    let llmSettings = createLlmSettings();
    llmSettings = updateProviderSetting(llmSettings, 'open_ai', {
      apiHost: 'https://api.openai.com',
      apiKey: 'openai-key',
    });
    llmSettings = addLlmModel(llmSettings, { provider: 'open_ai', model: 'gpt-4o-mini' });
    llmSettings = setFeatureModelSelection(llmSettings, 'polish', llmSettings.modelOrder[0]);
    llmSettings = setFeatureModelSelection(llmSettings, 'translation', llmSettings.modelOrder[0]);
    llmSettings = setFeatureModelSelection(llmSettings, 'summary', llmSettings.modelOrder[0]);
    llmSettings = setFeatureTemperature(llmSettings, 'polish', 0.2);
    llmSettings = setFeatureTemperature(llmSettings, 'translation', 1.1);
    llmSettings = setFeatureTemperature(llmSettings, 'summary', 0.4);

    const config = buildLlmConfigPatch(llmSettings);

    expect(getFeatureLlmConfig(config, 'polish')).toEqual(
      expect.objectContaining({
        temperature: 0.2,
      })
    );
    expect(getFeatureLlmConfig(config, 'translation')).toEqual(
      expect.objectContaining({
        temperature: 1.1,
      })
    );
    expect(getFeatureLlmConfig(config, 'summary')).toEqual(
      expect.objectContaining({
        temperature: 0.4,
      })
    );
  });

  it('falls back to the global default temperature when feature temperature is unset', () => {
    let llmSettings = createLlmSettings();
    llmSettings = updateProviderSetting(llmSettings, 'open_ai', {
      apiHost: 'https://api.openai.com',
      apiKey: 'openai-key',
    });
    llmSettings = addLlmModel(llmSettings, { provider: 'open_ai', model: 'gpt-4o-mini' });
    llmSettings = setFeatureModelSelection(llmSettings, 'polish', llmSettings.modelOrder[0]);

    const config = buildLlmConfigPatch(llmSettings);

    expect(getFeatureLlmConfig(config, 'polish')).toEqual(
      expect.objectContaining({
        temperature: DEFAULT_LLM_TEMPERATURE,
      })
    );
  });

  it('ignores provider-level temperature when feature temperature is unset', () => {
    const config = {
      llmSettings: {
        activeProvider: 'open_ai',
        providers: {
          open_ai: {
            apiHost: 'https://api.openai.com',
            apiKey: 'openai-key',
            temperature: 0.55,
          },
        },
        models: {
          'open-ai-test': {
            id: 'open-ai-test',
            provider: 'open_ai',
            model: 'gpt-4o-mini',
          },
        },
        modelOrder: ['open-ai-test'],
        selections: {
          polishModelId: 'open-ai-test',
        },
      },
    } as any;

    expect(getFeatureLlmConfig(config, 'polish')).toEqual(
      expect.objectContaining({
        temperature: DEFAULT_LLM_TEMPERATURE,
      })
    );
  });

  it('reports completeness from the feature-specific runtime selection', () => {
    let llmSettings = createLlmSettings();
    llmSettings = updateProviderSetting(llmSettings, 'open_ai', {
      apiHost: 'https://api.openai.com',
      apiKey: 'openai-key',
    });
    llmSettings = addLlmModel(llmSettings, { provider: 'open_ai', model: 'gpt-4o-mini' });
    llmSettings = setFeatureModelSelection(llmSettings, 'summary', llmSettings.modelOrder[0]);

    expect(isSummaryLlmConfigComplete({ llmSettings })).toBe(true);
    expect(isFeatureLlmConfigComplete({ llmSettings }, 'summary')).toBe(true);

    const missingSummarySelection = setFeatureModelSelection(llmSettings, 'summary', undefined);
    expect(isSummaryLlmConfigComplete({ llmSettings: missingSummarySelection })).toBe(false);
    expect(isFeatureLlmConfigComplete({ llmSettings: missingSummarySelection }, 'summary')).toBe(
      false
    );
  });

  it('resolves runtime strategy for custom providers', () => {
    let llmSettings = createLlmSettings();
    llmSettings.customProviders = {
      'custom-claude-gateway': {
        id: 'custom-claude-gateway',
        name: 'Claude Gateway',
        strategy: 'anthropic',
        createdAt: '2026-05-18T00:00:00.000Z',
      },
    };
    llmSettings = updateProviderSetting(llmSettings, 'custom-claude-gateway', {
      apiHost: 'https://claude.example.com',
      apiKey: 'claude-key',
    });
    llmSettings = addLlmModel(llmSettings, {
      provider: 'custom-claude-gateway',
      model: 'claude-sonnet-4-20250514',
    });
    llmSettings = setFeatureModelSelection(llmSettings, 'summary', llmSettings.modelOrder[0]);

    expect(getFeatureLlmConfig({ llmSettings }, 'summary')).toEqual(
      expect.objectContaining({
        provider: 'custom-claude-gateway',
        strategy: 'anthropic',
        baseUrl: 'https://claude.example.com',
        apiKey: 'claude-key',
        model: 'claude-sonnet-4-20250514',
      })
    );
    expect(isFeatureLlmConfigComplete({ llmSettings }, 'summary')).toBe(true);
  });

  it('resolves feature-specific reasoning options independently', () => {
    let llmSettings = createLlmSettings();
    llmSettings = updateProviderSetting(llmSettings, 'open_ai', {
      apiHost: 'https://api.openai.com',
      apiKey: 'openai-key',
    });
    llmSettings = addLlmModel(llmSettings, { provider: 'open_ai', model: 'gpt-4o-mini' });
    llmSettings = setFeatureModelSelection(llmSettings, 'polish', llmSettings.modelOrder[0]);
    llmSettings = setFeatureModelSelection(llmSettings, 'translation', llmSettings.modelOrder[0]);
    llmSettings = setFeatureReasoningEnabled(llmSettings, 'polish', true);
    llmSettings = setFeatureReasoningLevel(llmSettings, 'polish', 'high');
    llmSettings = setFeatureReasoningEnabled(llmSettings, 'translation', false);

    const config = buildLlmConfigPatch(llmSettings);

    expect(getFeatureLlmConfig(config, 'polish')).toEqual(
      expect.objectContaining({
        reasoningEnabled: true,
        reasoningLevel: 'high',
      })
    );
    expect(getFeatureLlmConfig(config, 'translation')).toEqual(
      expect.objectContaining({
        reasoningEnabled: false,
        reasoningLevel: undefined,
      })
    );
  });

  it('applies reasoningBudget only when model metadata specifies budget mode', () => {
    let llmSettings = createLlmSettings();
    llmSettings = updateProviderSetting(llmSettings, 'anthropic', {
      apiHost: 'https://api.anthropic.com',
      apiKey: 'anthropic-key',
    });
    llmSettings = addLlmModel(llmSettings, {
      provider: 'anthropic',
      model: 'claude-3-7-sonnet',
      metadata: {
        reasoningMode: {
          type: 'budget',
          min_budget: 1024,
          max_budget: 64000,
          default_budget: 4096,
        },
      },
    });
    llmSettings = addLlmModel(llmSettings, {
      provider: 'anthropic',
      model: 'o3-mini-proxy',
      metadata: {
        reasoningMode: {
          type: 'effort',
          supported_levels: [{ mode: 'low' }, { mode: 'medium' }, { mode: 'high' }],
        },
      },
    });

    // Set model to budget model with a budget
    llmSettings = setFeatureModelSelection(llmSettings, 'summary', llmSettings.modelOrder[0]);
    llmSettings = setFeatureReasoningEnabled(llmSettings, 'summary', true);
    llmSettings = setFeatureReasoningLevel(llmSettings, 'summary', 'high');
    llmSettings = {
      ...llmSettings,
      selections: {
        ...llmSettings.selections,
        summaryReasoningBudget: 8192,
      },
    };

    let config = buildLlmConfigPatch(llmSettings);
    expect(getFeatureLlmConfig(config, 'summary')).toEqual(
      expect.objectContaining({
        reasoningEnabled: true,
        reasoningLevel: '8192',
        reasoningBudget: 8192,
      })
    );

    // Switch model to effort model, keeping old summaryReasoningBudget in selections
    llmSettings = setFeatureModelSelection(llmSettings, 'summary', llmSettings.modelOrder[1]);
    config = buildLlmConfigPatch(llmSettings);
    expect(getFeatureLlmConfig(config, 'summary')).toEqual(
      expect.objectContaining({
        reasoningEnabled: true,
        reasoningLevel: 'high',
        reasoningBudget: undefined,
      })
    );
  });

  it('resolves local NMT translation config with local_nmt strategy and auto model', () => {
    let llmSettings = createLlmSettings();
    llmSettings = addLlmModel(llmSettings, { provider: 'local', model: 'auto' });
    llmSettings = setFeatureModelSelection(llmSettings, 'translation', llmSettings.modelOrder[0]);

    const config = buildLlmConfigPatch(llmSettings);
    expect(getFeatureLlmConfig(config, 'translation')).toEqual(
      expect.objectContaining({
        provider: 'local',
        strategy: 'local_nmt',
        model: 'auto',
      })
    );
  });

  it('strictly routes opus-mt-* models to local_nmt and custom GGUFs containing opus to local', () => {
    let llmSettings = createLlmSettings();
    llmSettings = addLlmModel(llmSettings, { provider: 'local', model: 'test-opus-model.gguf' });
    llmSettings = addLlmModel(llmSettings, { provider: 'local', model: 'opus-mt-en-zh' });

    // Custom GGUF model with 'opus' in the name -> should NOT route to local_nmt
    llmSettings = setFeatureModelSelection(llmSettings, 'translation', llmSettings.modelOrder[0]);
    let config = buildLlmConfigPatch(llmSettings);
    expect(getFeatureLlmConfig(config, 'translation')).toEqual(
      expect.objectContaining({
        provider: 'local',
        strategy: 'local',
        model: 'test-opus-model.gguf',
      })
    );

    // Exact opus-mt- prefix model -> routes to local_nmt
    llmSettings = setFeatureModelSelection(llmSettings, 'translation', llmSettings.modelOrder[1]);
    config = buildLlmConfigPatch(llmSettings);
    expect(getFeatureLlmConfig(config, 'translation')).toEqual(
      expect.objectContaining({
        provider: 'local',
        strategy: 'local_nmt',
        model: 'opus-mt-en-zh',
      })
    );
  });

  it('syncs realtimeTranslationEngine and modelId for local translation models', () => {
    let llmSettings = createLlmSettings();
    llmSettings = addLlmModel(llmSettings, { provider: 'local', model: 'opus-mt-en-zh' });
    llmSettings = addLlmModel(llmSettings, { provider: 'local', model: 'index-translate-2b' });
    llmSettings = addLlmModel(llmSettings, { provider: 'local', model: 'hy-mt1.5-1.8b' });
    llmSettings = addLlmModel(llmSettings, { provider: 'local', model: 'Qwen/Qwen3.5-4B' });
    llmSettings = addLlmModel(llmSettings, { provider: 'local', model: 'auto' });

    // Opus-MT -> onnx with opus model id
    llmSettings = setFeatureModelSelection(llmSettings, 'translation', llmSettings.modelOrder[0]);
    let patch = buildLlmConfigPatch(llmSettings);
    expect(patch.realtimeTranslationEngine).toBe('onnx');
    expect(patch.realtimeTranslationModelId).toBe('opus-mt-en-zh');

    // Index-Translate -> llama-cpp
    llmSettings = setFeatureModelSelection(llmSettings, 'translation', llmSettings.modelOrder[1]);
    patch = buildLlmConfigPatch(llmSettings);
    expect(patch.realtimeTranslationEngine).toBe('llama-cpp');
    expect(patch.realtimeTranslationModelId).toBe('index-translate-2b');

    // Hunyuan-MT -> llama-cpp
    llmSettings = setFeatureModelSelection(llmSettings, 'translation', llmSettings.modelOrder[2]);
    patch = buildLlmConfigPatch(llmSettings);
    expect(patch.realtimeTranslationEngine).toBe('llama-cpp');
    expect(patch.realtimeTranslationModelId).toBe('hy-mt1.5-1.8b');

    // General local LLM -> llama-cpp
    llmSettings = setFeatureModelSelection(llmSettings, 'translation', llmSettings.modelOrder[3]);
    patch = buildLlmConfigPatch(llmSettings);
    expect(patch.realtimeTranslationEngine).toBe('llama-cpp');
    expect(patch.realtimeTranslationModelId).toBe('Qwen/Qwen3.5-4B');

    // Auto NMT -> onnx with null model id
    llmSettings = setFeatureModelSelection(llmSettings, 'translation', llmSettings.modelOrder[4]);
    patch = buildLlmConfigPatch(llmSettings);
    expect(patch.realtimeTranslationEngine).toBe('onnx');
    expect(patch.realtimeTranslationModelId).toBeNull();
  });
});
