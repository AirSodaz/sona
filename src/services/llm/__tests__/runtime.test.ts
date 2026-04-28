import { describe, expect, it } from 'vitest';
import {
  addLlmModel,
  buildLlmConfigPatch,
  createLlmSettings,
  setFeatureModelSelection,
  setFeatureTemperature,
  updateProviderSetting,
} from '../state';
import {
  getFeatureLlmConfig,
  isFeatureLlmConfigComplete,
  isSummaryLlmConfigComplete,
} from '../runtime';
import { DEFAULT_LLM_TEMPERATURE } from '../providers';

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
    llmSettings = addLlmModel(llmSettings, { provider: 'anthropic', model: 'claude-sonnet-4-20250514' });
    llmSettings = setFeatureModelSelection(llmSettings, 'polish', llmSettings.modelOrder[0]);
    llmSettings = setFeatureModelSelection(llmSettings, 'translation', llmSettings.modelOrder[1]);
    llmSettings = setFeatureModelSelection(llmSettings, 'summary', llmSettings.modelOrder[0]);

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
    expect(getFeatureLlmConfig(config, 'summary')).toEqual(expect.objectContaining({
      provider: 'open_ai',
      apiKey: 'openai-key',
      model: 'gpt-4o-mini',
    }));
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

    expect(getFeatureLlmConfig(config, 'polish')).toEqual(expect.objectContaining({
      temperature: 0.2,
    }));
    expect(getFeatureLlmConfig(config, 'translation')).toEqual(expect.objectContaining({
      temperature: 1.1,
    }));
    expect(getFeatureLlmConfig(config, 'summary')).toEqual(expect.objectContaining({
      temperature: 0.4,
    }));
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

    expect(getFeatureLlmConfig(config, 'polish')).toEqual(expect.objectContaining({
      temperature: DEFAULT_LLM_TEMPERATURE,
    }));
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

    expect(getFeatureLlmConfig(config, 'polish')).toEqual(expect.objectContaining({
      temperature: DEFAULT_LLM_TEMPERATURE,
    }));
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
    expect(isFeatureLlmConfigComplete({ llmSettings: missingSummarySelection }, 'summary')).toBe(false);
  });
});
