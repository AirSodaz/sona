import { describe, expect, it } from 'vitest';
import {
  addLlmModel,
  buildLlmConfigPatch,
  createLlmSettings,
  addCustomProvider,
  ensureProviderSetting,
  findLlmModelId,
  getModelDiscoveryStatus,
  getFeatureModelEntry,
  getOrderedLlmModels,
  getProviderLlmModels,
  isProviderModelDiscoveryExpired,
  removeLlmModel,
  sanitizeProviderSetting,
  setFeatureModelSelection,
  setFeatureTemperature,
  syncProviderDiscoveredModels,
  updateLlmModelMetadata,
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
      expect.objectContaining({ provider: 'open_ai', model: 'gpt-4o-mini', source: 'manual' }),
      expect.objectContaining({ provider: 'anthropic', model: 'claude-sonnet-4-20250514', source: 'manual' }),
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
      source: 'manual',
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

  it('adds a custom provider with provider settings and stable metadata', () => {
    const createdAt = '2026-05-18T00:00:00.000Z';
    const nextSettings = addCustomProvider(createLlmSettings(), {
      name: 'Private Gateway',
      strategy: 'openai_responses',
      createdAt,
    });

    expect(nextSettings.activeProvider).toBe('custom-private-gateway');
    expect(nextSettings.customProviders).toEqual({
      'custom-private-gateway': {
        id: 'custom-private-gateway',
        name: 'Private Gateway',
        strategy: 'openai_responses',
        createdAt,
      },
    });
    expect(nextSettings.providers['custom-private-gateway']).toEqual(expect.objectContaining({
      apiHost: '',
      apiKey: '',
      apiPath: '/v1/responses',
    }));
  });

  it('finds provider models by provider and preserves metadata for manual additions', () => {
    let llmSettings = createLlmSettings('open_ai');
    llmSettings = addLlmModel(llmSettings, {
      provider: 'open_ai',
      model: 'gpt-4.1',
      metadata: {
        contextWindow: 128000,
        supportsTools: true,
      },
    });
    llmSettings = addLlmModel(llmSettings, { provider: 'gemini', model: 'gemini-2.5-flash' });

    expect(findLlmModelId(llmSettings, 'open_ai', 'gpt-4.1')).toBe(llmSettings.modelOrder[0]);
    expect(getProviderLlmModels(llmSettings, 'open_ai')).toEqual([
      expect.objectContaining({
        provider: 'open_ai',
        model: 'gpt-4.1',
        source: 'manual',
        metadata: expect.objectContaining({
          contextWindow: 128000,
          supportsTools: true,
        }),
      }),
    ]);
  });

  it('syncs discovered provider models without deleting manual entries for the same provider', () => {
    let llmSettings = createLlmSettings('open_ai');
    llmSettings = addLlmModel(llmSettings, {
      provider: 'open_ai',
      model: 'manual-model',
    });
    llmSettings = syncProviderDiscoveredModels(llmSettings, 'open_ai', [
      {
        model: 'gpt-4.1',
        contextWindow: 100000,
        supportsMultimodal: true,
      },
      {
        model: 'gpt-4.1-mini',
        outputPrice: 0.8,
      },
    ]);

    expect(getProviderLlmModels(llmSettings, 'open_ai')).toEqual([
      expect.objectContaining({ model: 'manual-model', source: 'manual' }),
      expect.objectContaining({
        model: 'gpt-4.1',
        source: 'discovered',
        metadata: expect.objectContaining({
          contextWindow: 100000,
          supportsMultimodal: true,
        }),
      }),
      expect.objectContaining({
        model: 'gpt-4.1-mini',
        source: 'discovered',
        metadata: expect.objectContaining({
          outputPrice: 0.8,
        }),
      }),
    ]);
  });

  it('stores provider model discovery expiration when syncing discovered models', () => {
    const fetchedAt = '2026-05-24T10:00:00.000Z';
    const llmSettings = syncProviderDiscoveredModels(createLlmSettings('open_ai'), 'open_ai', [
      { model: 'gpt-4.1' },
    ], fetchedAt);

    expect(getModelDiscoveryStatus(llmSettings, 'open_ai')).toEqual({
      fetchedAt,
      expiresAt: '2026-05-25T10:00:00.000Z',
    });
    expect(isProviderModelDiscoveryExpired(llmSettings, 'open_ai', '2026-05-24T12:00:00.000Z')).toBe(false);
    expect(isProviderModelDiscoveryExpired(llmSettings, 'open_ai', '2026-05-25T10:00:00.000Z')).toBe(true);
    expect(isProviderModelDiscoveryExpired(createLlmSettings('open_ai'), 'open_ai', fetchedAt)).toBe(true);
  });

  it('marks a provider model discovery cache expired when provider settings change', () => {
    const synced = syncProviderDiscoveredModels(createLlmSettings('open_ai'), 'open_ai', [
      { model: 'gpt-4.1' },
    ], '2026-05-24T10:00:00.000Z');

    const updated = updateProviderSetting(synced, 'open_ai', {
      apiHost: 'https://gateway.example.com',
    });

    expect(getProviderLlmModels(updated, 'open_ai')).toEqual([
      expect.objectContaining({ model: 'gpt-4.1', source: 'discovered' }),
    ]);
    expect(isProviderModelDiscoveryExpired(updated, 'open_ai', '2026-05-24T10:05:00.000Z')).toBe(true);
  });

  it('removes stale discovered models while preserving manual models and clearing stale selections', () => {
    let llmSettings = createLlmSettings('open_ai');
    llmSettings = syncProviderDiscoveredModels(llmSettings, 'open_ai', [
      { model: 'gpt-4.1' },
      { model: 'gpt-4.1-mini' },
    ]);
    llmSettings = addLlmModel(llmSettings, { provider: 'open_ai', model: 'manual-model' });

    const staleDiscoveredId = findLlmModelId(llmSettings, 'open_ai', 'gpt-4.1');
    expect(staleDiscoveredId).toBeDefined();
    llmSettings = setFeatureModelSelection(llmSettings, 'polish', staleDiscoveredId);

    llmSettings = syncProviderDiscoveredModels(llmSettings, 'open_ai', [
      { model: 'gpt-4.1-mini', supportsReasoning: true },
    ]);

    expect(findLlmModelId(llmSettings, 'open_ai', 'gpt-4.1')).toBeUndefined();
    expect(findLlmModelId(llmSettings, 'open_ai', 'manual-model')).toBeDefined();
    expect(llmSettings.selections.polishModelId).toBeUndefined();
    expect(getProviderLlmModels(llmSettings, 'open_ai')).toEqual([
      expect.objectContaining({ model: 'gpt-4.1-mini', source: 'discovered' }),
      expect.objectContaining({ model: 'manual-model', source: 'manual' }),
    ]);
  });

  it('updates model metadata while preserving model order and feature selections', () => {
    let llmSettings = createLlmSettings('open_ai');
    llmSettings = addLlmModel(llmSettings, {
      provider: 'open_ai',
      model: 'gpt-4.1',
      metadata: {
        contextWindow: 128000,
        supportsTools: true,
      },
    });
    const modelId = llmSettings.modelOrder[0];
    llmSettings = setFeatureModelSelection(llmSettings, 'summary', modelId);

    const nextSettings = updateLlmModelMetadata(llmSettings, modelId, {
      contextWindow: 200000,
      inputPrice: 2.5,
      supportsTools: false,
      supportsReasoning: true,
    });

    expect(nextSettings.modelOrder).toEqual([modelId]);
    expect(nextSettings.selections.summaryModelId).toBe(modelId);
    expect(nextSettings.models[modelId]).toEqual(expect.objectContaining({
      model: 'gpt-4.1',
      metadata: expect.objectContaining({
        contextWindow: 200000,
        inputPrice: 2.5,
        supportsTools: false,
        supportsReasoning: true,
      }),
      metadataOverrides: {
        contextWindow: true,
        inputPrice: true,
        supportsTools: true,
        supportsReasoning: true,
      },
    }));
  });

  it('keeps edited discovered metadata fields while refreshing provider values for untouched fields', () => {
    let llmSettings = syncProviderDiscoveredModels(createLlmSettings('open_ai'), 'open_ai', [
      {
        model: 'gpt-4.1',
        contextWindow: 128000,
        inputPrice: 2,
        outputPrice: 8,
        supportsTools: true,
      },
    ]);
    const modelId = findLlmModelId(llmSettings, 'open_ai', 'gpt-4.1');
    expect(modelId).toBeDefined();
    llmSettings = updateLlmModelMetadata(llmSettings, modelId!, {
      contextWindow: 200000,
      supportsTools: false,
    });

    const nextSettings = syncProviderDiscoveredModels(llmSettings, 'open_ai', [
      {
        model: 'gpt-4.1',
        contextWindow: 64000,
        inputPrice: 1.5,
        outputPrice: 6,
        supportsTools: true,
        supportsReasoning: true,
      },
    ]);

    expect(nextSettings.models[modelId!].metadata).toEqual(expect.objectContaining({
      contextWindow: 200000,
      inputPrice: 1.5,
      outputPrice: 6,
      supportsTools: false,
      supportsReasoning: true,
    }));
    expect(nextSettings.models[modelId!].metadataOverrides).toEqual({
      contextWindow: true,
      supportsTools: true,
    });
  });

  it('keeps edited manual model metadata outside discovered refreshes', () => {
    let llmSettings = addLlmModel(createLlmSettings('open_ai'), {
      provider: 'open_ai',
      model: 'manual-model',
      metadata: {
        contextWindow: 32000,
      },
    });
    const manualModelId = llmSettings.modelOrder[0];
    llmSettings = updateLlmModelMetadata(llmSettings, manualModelId, {
      contextWindow: 64000,
      outputPrice: 3,
    });

    const nextSettings = syncProviderDiscoveredModels(llmSettings, 'open_ai', [
      {
        model: 'gpt-4.1',
        contextWindow: 128000,
      },
    ]);

    expect(nextSettings.models[manualModelId]).toEqual(expect.objectContaining({
      model: 'manual-model',
      source: 'manual',
      metadata: expect.objectContaining({
        contextWindow: 64000,
        outputPrice: 3,
      }),
      metadataOverrides: {
        contextWindow: true,
        outputPrice: true,
      },
    }));
    expect(findLlmModelId(nextSettings, 'open_ai', 'gpt-4.1')).toBeDefined();
  });

  it('sanitizes google_translate_free to the official endpoint ignoring persisted gateway', () => {
    const setting = sanitizeProviderSetting('google_translate_free', {
      apiHost: 'https://api2.apiaqi.com',
      apiKey: 'leaked-key',
      apiPath: '/custom/path',
      apiVersion: '2',
    });

    expect(setting).toEqual({
      apiHost: 'https://translate.googleapis.com/translate_a/single',
      apiKey: '',
      apiPath: undefined,
      apiVersion: undefined,
    });
  });

  it('preserves user-configured apiHost and apiKey for non-free providers', () => {
    const setting = sanitizeProviderSetting('open_ai', {
      apiHost: 'https://custom-gateway.example.com',
      apiKey: 'sk-test',
    });

    expect(setting.apiHost).toBe('https://custom-gateway.example.com');
    expect(setting.apiKey).toBe('sk-test');
  });

  it('ensures google_translate_free provider setting returns official defaults from stored settings', () => {
    let llmSettings = createLlmSettings('google_translate_free');
    llmSettings = updateProviderSetting(llmSettings, 'google_translate_free', {
      apiHost: 'https://api2.apiaqi.com',
      apiKey: 'bad-key',
    });

    const setting = ensureProviderSetting(llmSettings, 'google_translate_free');

    expect(setting.apiHost).toBe('https://translate.googleapis.com/translate_a/single');
    expect(setting.apiKey).toBe('');
  });
});
