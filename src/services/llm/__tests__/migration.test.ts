import { describe, expect, it } from 'vitest';
import { ensureLlmState } from '../migration';
import { getFeatureLlmConfig } from '../runtime';

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
    }));
    expect(llmSettings.selections.polishModelId).toBe(llmSettings.modelOrder[0]);
    expect(llmSettings.selections.translationModelId).toBe(llmSettings.modelOrder[0]);
    expect(llmSettings.selections.summaryModelId).toBe(llmSettings.modelOrder[0]);
  });

  it('cleans dangling selections and invalid stored temperatures during normalization', () => {
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
        },
      },
    } as any);

    expect(llmSettings.selections.polishModelId).toBeUndefined();
    expect(llmSettings.selections.translationModelId).toBe('open-ai-test');
    expect(llmSettings.selections.summaryModelId).toBeUndefined();
    expect(llmSettings.selections.polishTemperature).toBeUndefined();
    expect(llmSettings.selections.translationTemperature).toBe(1.2);
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
});
