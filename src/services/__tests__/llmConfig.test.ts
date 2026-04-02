import { describe, expect, it } from 'vitest';
import {
  addLlmModel,
  ensureLlmState,
  getFeatureLlmConfig,
  createLlmSettings,
  updateProviderSetting,
  buildLlmConfigPatch,
  removeLlmModel,
  setFeatureModelSelection,
  setFeatureTemperature,
} from '../llmConfig';

describe('llmConfig', () => {
  it('migrates legacy llm config into llmSettings without losing values', () => {
    const { llmSettings } = ensureLlmState({
      llm: {
        provider: 'open_ai',
        baseUrl: 'https://api.openai.com',
        apiKey: 'legacy-key',
        model: 'gpt-4o',
        temperature: 0.2,
      },
    } as any);

    expect(llmSettings.activeProvider).toBe('open_ai');
    expect(llmSettings.providers.open_ai).toEqual(expect.objectContaining({
      apiHost: 'https://api.openai.com',
      apiKey: 'legacy-key',
      temperature: 0.2,
    }));
    expect(llmSettings.modelOrder).toHaveLength(1);
    expect(llmSettings.models[llmSettings.modelOrder[0]]).toEqual(expect.objectContaining({
      provider: 'open_ai',
      model: 'gpt-4o',
    }));
    expect(llmSettings.selections.polishModelId).toBe(llmSettings.modelOrder[0]);
    expect(llmSettings.selections.translationModelId).toBe(llmSettings.modelOrder[0]);
    expect(getFeatureLlmConfig({ llmSettings }, 'polish')).toEqual(expect.objectContaining({
      provider: 'open_ai',
      baseUrl: 'https://api.openai.com',
      apiKey: 'legacy-key',
      model: 'gpt-4o',
      temperature: 0.2,
    }));
  });

  it('restores the Chatbox default host when migrating an empty Gemini host', () => {
    const { llmSettings } = ensureLlmState({
      llm: {
        provider: 'gemini',
        baseUrl: '',
        apiKey: 'gemini-key',
        model: 'gemini-2.5-flash',
        temperature: 0.7,
      },
    } as any);

    expect(llmSettings.providers.gemini).toEqual(expect.objectContaining({
      apiHost: 'https://generativelanguage.googleapis.com',
    }));
    expect(getFeatureLlmConfig({ llmSettings }, 'polish')?.baseUrl).toBe('https://generativelanguage.googleapis.com');
  });

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
    llmSettings = addLlmModel(llmSettings, { provider: 'anthropic', model: 'claude-sonnet-4-20250514' });
    llmSettings = setFeatureModelSelection(llmSettings, 'polish', llmSettings.modelOrder[0]);
    llmSettings = setFeatureModelSelection(llmSettings, 'translation', llmSettings.modelOrder[1]);

    const config = buildLlmConfigPatch(llmSettings);

    expect(getFeatureLlmConfig(config, 'polish')).toEqual(expect.objectContaining({
      provider: 'open_ai',
      apiKey: 'openai-key',
      model: 'gpt-4o-mini',
    }));
    expect(getFeatureLlmConfig(config, 'translation')).toEqual(expect.objectContaining({
      provider: 'anthropic',
      apiKey: 'anthropic-key',
      model: 'claude-sonnet-4-20250514',
    }));
  });

  it('resolves feature-specific temperatures before provider defaults', () => {
    let llmSettings = createLlmSettings();
    llmSettings = updateProviderSetting(llmSettings, 'open_ai', {
      apiHost: 'https://api.openai.com',
      apiKey: 'openai-key',
      temperature: 0.7,
    });
    llmSettings = addLlmModel(llmSettings, { provider: 'open_ai', model: 'gpt-4o-mini' });
    llmSettings = setFeatureModelSelection(llmSettings, 'polish', llmSettings.modelOrder[0]);
    llmSettings = setFeatureModelSelection(llmSettings, 'translation', llmSettings.modelOrder[0]);
    llmSettings = setFeatureTemperature(llmSettings, 'polish', 0.2);
    llmSettings = setFeatureTemperature(llmSettings, 'translation', 1.1);

    const config = buildLlmConfigPatch(llmSettings);

    expect(getFeatureLlmConfig(config, 'polish')).toEqual(expect.objectContaining({
      temperature: 0.2,
    }));
    expect(getFeatureLlmConfig(config, 'translation')).toEqual(expect.objectContaining({
      temperature: 1.1,
    }));
  });

  it('falls back to provider temperature when feature temperature is unset', () => {
    let llmSettings = createLlmSettings();
    llmSettings = updateProviderSetting(llmSettings, 'open_ai', {
      apiHost: 'https://api.openai.com',
      apiKey: 'openai-key',
      temperature: 0.65,
    });
    llmSettings = addLlmModel(llmSettings, { provider: 'open_ai', model: 'gpt-4o-mini' });
    llmSettings = setFeatureModelSelection(llmSettings, 'polish', llmSettings.modelOrder[0]);

    const config = buildLlmConfigPatch(llmSettings);

    expect(getFeatureLlmConfig(config, 'polish')).toEqual(expect.objectContaining({
      temperature: 0.65,
    }));
  });

  it('ignores invalid stored feature temperatures', () => {
    const { llmSettings } = ensureLlmState({
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
          polishTemperature: 9,
        },
      },
    } as any);

    expect(llmSettings.selections.polishTemperature).toBeUndefined();
    expect(getFeatureLlmConfig({ llmSettings }, 'polish')).toEqual(expect.objectContaining({
      temperature: 0.55,
    }));
  });

  it('clears feature selections when removing the selected model', () => {
    let llmSettings = addLlmModel(createLlmSettings(), { provider: 'open_ai', model: 'gpt-4o-mini' });
    const modelId = llmSettings.modelOrder[0];
    llmSettings = setFeatureModelSelection(llmSettings, 'polish', modelId);
    llmSettings = setFeatureModelSelection(llmSettings, 'translation', modelId);

    const nextSettings = removeLlmModel(llmSettings, modelId);

    expect(nextSettings.modelOrder).toEqual([]);
    expect(nextSettings.selections.polishModelId).toBeUndefined();
    expect(nextSettings.selections.translationModelId).toBeUndefined();
  });
});
