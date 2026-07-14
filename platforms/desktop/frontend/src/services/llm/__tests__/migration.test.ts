import { describe, expect, it } from 'vitest';
import { ensureLlmState } from '../migration';
import { getFeatureLlmConfig } from '../configUtils';

describe('llm migration', () => {
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
    }));
    expect(llmSettings.modelOrder).toHaveLength(1);
    expect(llmSettings.models[llmSettings.modelOrder[0]]).toEqual(expect.objectContaining({
      provider: 'open_ai',
      model: 'gpt-4o',
      source: 'manual',
    }));
    expect(llmSettings.selections.polishModelId).toBe(llmSettings.modelOrder[0]);
    expect(llmSettings.selections.translationModelId).toBe(llmSettings.modelOrder[0]);
    expect(llmSettings.selections.summaryModelId).toBe(llmSettings.modelOrder[0]);
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
  });

  it('migrates a legacy provider-scoped stored model when no normalized models survive', () => {
    const { llmSettings } = ensureLlmState({
      llmSettings: {
        activeProvider: 'anthropic',
        providers: {
          anthropic: {
            apiHost: 'https://api.anthropic.com',
            apiKey: 'anthropic-key',
            model: 'claude-sonnet-4-20250514',
          },
        },
        models: {},
        modelOrder: [],
        selections: {},
      },
    } as any);

    expect(llmSettings.modelOrder).toHaveLength(1);
    expect(llmSettings.models[llmSettings.modelOrder[0]]).toEqual(expect.objectContaining({
      provider: 'anthropic',
      model: 'claude-sonnet-4-20250514',
      source: 'manual',
    }));
    expect(llmSettings.selections.polishModelId).toBe(llmSettings.modelOrder[0]);
    expect(llmSettings.selections.translationModelId).toBe(llmSettings.modelOrder[0]);
    expect(llmSettings.selections.summaryModelId).toBe(llmSettings.modelOrder[0]);
  });

  it('normalizes persisted feature selections', () => {
    const { llmSettings } = ensureLlmState({
      llmSettings: {
        activeProvider: 'open_ai',
        providers: {
          open_ai: {
            apiHost: 'https://api.openai.com',
            apiKey: 'openai-key',
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
          polishModelId: 'missing-model',
          translationModelId: 'open-ai-test',
          summaryModelId: 'missing-model',
          polishTemperature: 9,
          translationTemperature: 1.2,
          polishReasoningEnabled: true,
          polishReasoningLevel: 'high',
          translationReasoningLevel: 'invalid',
        },
      },
    } as any);

    expect(llmSettings.selections.polishModelId).toBeUndefined();
    expect(llmSettings.selections.translationModelId).toBe('open-ai-test');
    expect(llmSettings.selections.summaryModelId).toBeUndefined();
    expect(llmSettings.selections.polishTemperature).toBeUndefined();
    expect(llmSettings.selections.translationTemperature).toBe(1.2);
    expect(llmSettings.selections).toEqual(expect.objectContaining({
      polishReasoningEnabled: true,
      polishReasoningLevel: 'high',
    }));
    expect(llmSettings.selections.translationReasoningLevel).toBeUndefined();
  });

  it('bootstraps a translation-only fallback when no model survives migration', () => {
    const { llmSettings } = ensureLlmState({
      llmSettings: {
        activeProvider: 'google_translate_free',
        providers: {},
        models: {},
        modelOrder: [],
        selections: {},
      },
    } as any);

    expect(llmSettings.modelOrder).toHaveLength(1);
    const fallbackModelId = llmSettings.modelOrder[0];
    expect(llmSettings.models[fallbackModelId]).toEqual(expect.objectContaining({
      provider: 'google_translate_free',
      model: 'default',
      source: 'manual',
    }));
    expect(llmSettings.selections.translationModelId).toBe(fallbackModelId);
    expect(llmSettings.selections.polishModelId).toBeUndefined();
    expect(llmSettings.selections.summaryModelId).toBeUndefined();
  });

  it('does not default summary to a translation-only google provider', () => {
    const { llmSettings } = ensureLlmState({
      llmSettings: {
        activeProvider: 'google_translate_free',
        providers: {
          google_translate_free: {
            apiHost: 'https://translate.googleapis.com/translate_a/single',
            apiKey: '',
          },
        },
        models: {
          'google-default': {
            id: 'google-default',
            provider: 'google_translate_free',
            model: 'default',
          },
        },
        modelOrder: ['google-default'],
        selections: {
          translationModelId: 'google-default',
        },
      },
    } as any);

    expect(llmSettings.selections.summaryModelId).toBeUndefined();
  });

  it('migrates the removed OpenAI Compatible provider into a custom provider', () => {
    const { llmSettings } = ensureLlmState({
      llmSettings: {
        activeProvider: 'open_ai_compatible',
        providers: {
          open_ai_compatible: {
            apiHost: 'https://gateway.example.com/v1',
            apiKey: 'gateway-key',
          },
        },
        models: {
          'open_ai_compatible-gpt-4o': {
            id: 'open_ai_compatible-gpt-4o',
            provider: 'open_ai_compatible',
            model: 'gpt-4o',
          },
        },
        modelOrder: ['open_ai_compatible-gpt-4o'],
        selections: {
          polishModelId: 'open_ai_compatible-gpt-4o',
        },
      },
    } as any);

    expect(llmSettings.activeProvider).toBe('custom-openai-compatible');
    expect(llmSettings.customProviders).toEqual({
      'custom-openai-compatible': {
        id: 'custom-openai-compatible',
        name: 'OpenAI Compatible',
        strategy: 'openai_compatible',
        createdAt: expect.any(String),
      },
    });
    expect(llmSettings.providers['custom-openai-compatible']).toEqual(expect.objectContaining({
      apiHost: 'https://gateway.example.com/v1',
      apiKey: 'gateway-key',
      apiPath: '/v1/chat/completions',
    }));
    expect(llmSettings.models['open_ai_compatible-gpt-4o']).toEqual(expect.objectContaining({
      provider: 'custom-openai-compatible',
      source: 'manual',
    }));
    expect(llmSettings.selections.polishModelId).toBe('open_ai_compatible-gpt-4o');
  });

  it('keeps persisted metadata while defaulting legacy stored models to manual source', () => {
    const { llmSettings } = ensureLlmState({
      llmSettings: {
        activeProvider: 'open_ai',
        providers: {
          open_ai: {
            apiHost: 'https://api.openai.com',
            apiKey: 'openai-key',
          },
        },
        models: {
          'open-ai-test': {
            id: 'open-ai-test',
            provider: 'open_ai',
            model: 'gpt-4.1',
            metadata: {
              displayName: ' GPT-4.1 ',
              contextWindow: 128000,
              cacheReadPrice: 0.5,
              inputModalities: ['text', 'image', 'invalid', 'text'],
              supportsTools: true,
              supportsStructuredOutput: true,
              metadataSources: ['provider', 'models_dev', 'invalid'],
            },
            metadataOverrides: {
              cacheReadPrice: true,
              metadataSources: true,
            },
          },
        },
        modelOrder: ['open-ai-test'],
        selections: {
          polishModelId: 'open-ai-test',
        },
      },
    } as any);

    expect(llmSettings.models['open-ai-test']).toEqual(expect.objectContaining({
      provider: 'open_ai',
      model: 'gpt-4.1',
      source: 'manual',
      metadata: expect.objectContaining({
        displayName: 'GPT-4.1',
        contextWindow: 128000,
        inputModalities: ['text', 'image'],
        supportsTools: true,
        metadataSources: ['provider', 'models_dev'],
      }),
      metadataOverrides: { cacheReadPrice: true },
    }));
    expect(llmSettings.selections.polishModelId).toBe('open-ai-test');
  });

  it('normalizes stored provider model discovery cache metadata', () => {
    const { llmSettings } = ensureLlmState({
      llmSettings: {
        activeProvider: 'open_ai',
        providers: {
          open_ai: {
            apiHost: 'https://api.openai.com',
            apiKey: 'openai-key',
          },
        },
        models: {
          'open-ai-test': {
            id: 'open-ai-test',
            provider: 'open_ai',
            model: 'gpt-4.1',
            source: 'discovered',
          },
        },
        modelOrder: ['open-ai-test'],
        modelDiscovery: {
          open_ai: {
            fetchedAt: '2026-05-24T10:00:00.000Z',
            expiresAt: '2026-05-25T10:00:00.000Z',
          },
          gemini: {
            fetchedAt: 'not-a-date',
            expiresAt: '2026-05-25T10:00:00.000Z',
          },
          anthropic: {
            fetchedAt: '2026-05-24T10:00:00.000Z',
            expiresAt: 'also-not-a-date',
          },
        },
        selections: {
          polishModelId: 'open-ai-test',
        },
      },
    } as any);

    expect(llmSettings.modelDiscovery).toEqual({
      open_ai: {
        fetchedAt: '2026-05-24T10:00:00.000Z',
        expiresAt: '2026-05-25T10:00:00.000Z',
      },
    });
  });
});
