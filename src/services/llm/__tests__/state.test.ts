import { describe, expect, it } from 'vitest';
import {
  addLlmModel,
  buildLlmConfigPatch,
  createLlmSettings,
  getFeatureModelEntry,
  getOrderedLlmModels,
  removeLlmModel,
  setFeatureModelSelection,
  setFeatureTemperature,
  updateProviderSetting,
} from '../state';
import { DEFAULT_LLM_PROVIDER } from '../providers';

describe('llm state', () => {
  it('creates initial settings for the default provider', () => {
    const llmSettings = createLlmSettings();

    expect(llmSettings.activeProvider).toBe(DEFAULT_LLM_PROVIDER);
    expect(llmSettings.providers[DEFAULT_LLM_PROVIDER]).toEqual(expect.objectContaining({
      apiHost: 'https://translate.googleapis.com/translate_a/single',
    }));
    expect(llmSettings.modelOrder).toEqual([]);
  });

  it('dedupes identical provider and model pairs while keeping insertion order', () => {
    let llmSettings = createLlmSettings('open_ai');
    llmSettings = addLlmModel(llmSettings, { provider: 'open_ai', model: 'gpt-4o-mini' });
    llmSettings = addLlmModel(llmSettings, { provider: 'open_ai', model: 'gpt-4o-mini' });
    llmSettings = addLlmModel(llmSettings, { provider: 'anthropic', model: 'claude-sonnet-4-20250514' });

    expect(llmSettings.modelOrder).toHaveLength(2);
    expect(getOrderedLlmModels(llmSettings)).toEqual([
      expect.objectContaining({ provider: 'open_ai', model: 'gpt-4o-mini' }),
      expect.objectContaining({ provider: 'anthropic', model: 'claude-sonnet-4-20250514' }),
    ]);
  });

  it('reads feature model entries from the persisted llmSettings patch', () => {
    let llmSettings = createLlmSettings();
    llmSettings = updateProviderSetting(llmSettings, 'open_ai', {
      apiHost: 'https://api.openai.com',
      apiKey: 'openai-key',
    });
    llmSettings = addLlmModel(llmSettings, { provider: 'open_ai', model: 'gpt-4o-mini' });
    llmSettings = setFeatureModelSelection(llmSettings, 'summary', llmSettings.modelOrder[0]);

    const config = buildLlmConfigPatch(llmSettings);

    expect(getFeatureModelEntry(config, 'summary')).toEqual(expect.objectContaining({
      provider: 'open_ai',
      model: 'gpt-4o-mini',
    }));
  });

  it('stores feature temperatures independently on selections', () => {
    let llmSettings = createLlmSettings();
    llmSettings = setFeatureTemperature(llmSettings, 'polish', 0.2);
    llmSettings = setFeatureTemperature(llmSettings, 'translation', 1.1);
    llmSettings = setFeatureTemperature(llmSettings, 'summary', 0.4);

    expect(llmSettings.selections).toEqual(expect.objectContaining({
      polishTemperature: 0.2,
      translationTemperature: 1.1,
      summaryTemperature: 0.4,
    }));
  });

  it('clears feature selections when removing the selected model', () => {
    let llmSettings = addLlmModel(createLlmSettings(), { provider: 'open_ai', model: 'gpt-4o-mini' });
    const modelId = llmSettings.modelOrder[0];
    llmSettings = setFeatureModelSelection(llmSettings, 'polish', modelId);
    llmSettings = setFeatureModelSelection(llmSettings, 'translation', modelId);
    llmSettings = setFeatureModelSelection(llmSettings, 'summary', modelId);

    const nextSettings = removeLlmModel(llmSettings, modelId);

    expect(nextSettings.modelOrder).toEqual([]);
    expect(nextSettings.selections.polishModelId).toBeUndefined();
    expect(nextSettings.selections.translationModelId).toBeUndefined();
    expect(nextSettings.selections.summaryModelId).toBeUndefined();
  });
});
