import { describe, expect, it } from 'vitest';
import {
  ensureLlmState,
  getActiveLlmConfig,
  createLlmSettings,
  updateProviderSetting,
  buildLlmConfigPatch,
} from '../llmConfig';

describe('llmConfig', () => {
  it('migrates legacy llm config into llmSettings without losing values', () => {
    const { llmSettings, llm } = ensureLlmState({
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
      model: 'gpt-4o',
      temperature: 0.2,
    }));
    expect(llm).toEqual(expect.objectContaining({
      provider: 'open_ai',
      baseUrl: 'https://api.openai.com',
      apiKey: 'legacy-key',
      model: 'gpt-4o',
    }));
  });

  it('restores the Chatbox default host when migrating an empty Gemini host', () => {
    const { llmSettings, llm } = ensureLlmState({
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
    expect(llm.baseUrl).toBe('https://generativelanguage.googleapis.com');
  });

  it('derives active llm config from the active provider entry', () => {
    const baseConfig: any = {
      llmSettings: createLlmSettings(),
    };
    let llmSettings = updateProviderSetting(baseConfig.llmSettings, 'open_ai', {
      apiHost: 'https://api.openai.com',
      apiKey: 'openai-key',
      model: 'gpt-4o-mini',
    });
    llmSettings = updateProviderSetting(llmSettings, 'azure_openai', {
      apiHost: 'https://example.openai.azure.com',
      apiKey: 'azure-key',
      model: 'deployment-1',
      apiVersion: '2024-10-21',
    });
    llmSettings.activeProvider = 'azure_openai';

    const config = {
      ...baseConfig,
      ...buildLlmConfigPatch(llmSettings),
    };

    expect(getActiveLlmConfig(config)).toEqual(expect.objectContaining({
      provider: 'azure_openai',
      baseUrl: 'https://example.openai.azure.com',
      apiKey: 'azure-key',
      model: 'deployment-1',
      apiVersion: '2024-10-21',
    }));
  });
});
