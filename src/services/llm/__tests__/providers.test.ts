import { describe, expect, it } from 'vitest';
import type { CustomLlmProvider } from '../../../types/transcript';
import {
  BUILT_IN_LLM_PROVIDER_DEFINITIONS,
  buildLlmConfig,
  createCustomProviderDefinition,
  createCustomProviderId,
  createProviderSetting,
  DEFAULT_LLM_CONFIG,
  DEFAULT_LLM_PROVIDER,
  DEFAULT_LLM_TEMPERATURE,
  getDefaultLlmConfig,
  getProviderDefinition,
  listProviderDefinitions,
  normalizeProvider,
} from '../providers';

describe('llm providers', () => {
  it('normalizes legacy provider spellings to canonical ids', () => {
    expect(normalizeProvider('openai')).toBe('open_ai');
    expect(normalizeProvider('azure_open_ai')).toBe('azure_openai');
    expect(normalizeProvider('deepseek')).toBe('deep_seek');
    expect(normalizeProvider('siliconflow')).toBe('silicon_flow');
    expect(normalizeProvider('unknown-provider')).toBe(DEFAULT_LLM_PROVIDER);
  });

  it('creates provider settings with the registry defaults for host, path, and version', () => {
    expect(createProviderSetting('open_ai_responses')).toEqual(expect.objectContaining({
      apiHost: 'https://api.openai.com',
      apiPath: '/v1/responses',
    }));
    expect(createProviderSetting('azure_openai')).toEqual(expect.objectContaining({
      apiHost: '',
      apiVersion: '2024-10-21',
    }));
    expect(createProviderSetting('perplexity')).toEqual(expect.objectContaining({
      apiHost: 'https://api.perplexity.ai',
      apiPath: '/chat/completions',
    }));
    expect(createProviderSetting('volcengine')).toEqual(expect.objectContaining({
      apiHost: 'https://ark.cn-beijing.volces.com',
      apiPath: '/api/v3/chat/completions',
    }));
  });

  it('builds provider-level runtime configs without choosing a model yet', () => {
    const config = buildLlmConfig('open_ai', {
      apiHost: 'https://example.com',
      apiKey: 'test-key',
      apiPath: '/v1/chat/completions',
      apiVersion: undefined,
    });

    expect(config).toEqual({
      provider: 'open_ai',
      baseUrl: 'https://example.com',
      apiKey: 'test-key',
      model: '',
      strategy: 'openai_compatible',
      apiPath: '/v1/chat/completions',
      apiVersion: undefined,
      temperature: DEFAULT_LLM_TEMPERATURE,
    });
  });

  it('keeps the exported default runtime config in sync with the default provider settings', () => {
    expect(DEFAULT_LLM_CONFIG).toEqual(getDefaultLlmConfig(DEFAULT_LLM_PROVIDER));
  });

  it('does not expose OpenAI Compatible as a built-in provider', () => {
    expect(BUILT_IN_LLM_PROVIDER_DEFINITIONS.map((provider) => provider.id)).not.toContain('open_ai_compatible');
  });

  it('creates stable custom provider definitions from API modes', () => {
    const openAiProvider = createCustomProviderDefinition({
      id: 'custom-openai',
      name: 'Private Gateway',
      strategy: 'openai_compatible',
      createdAt: '2026-05-18T00:00:00.000Z',
    });
    const responsesProvider = createCustomProviderDefinition({
      id: 'custom-responses',
      name: 'Responses Gateway',
      strategy: 'openai_responses',
      createdAt: '2026-05-18T00:00:00.000Z',
    });
    const anthropicProvider = createCustomProviderDefinition({
      id: 'custom-claude',
      name: 'Claude Gateway',
      strategy: 'anthropic',
      createdAt: '2026-05-18T00:00:00.000Z',
    });
    const geminiProvider = createCustomProviderDefinition({
      id: 'custom-gemini',
      name: 'Gemini Gateway',
      strategy: 'gemini',
      createdAt: '2026-05-18T00:00:00.000Z',
    });

    expect(openAiProvider).toEqual(expect.objectContaining({
      id: 'custom-openai',
      label: 'Private Gateway',
      defaultApiHost: '',
      defaultApiPath: '/v1/chat/completions',
      requiresApiKey: true,
      supportsModelListing: true,
      strategy: 'openai_compatible',
    }));
    expect(responsesProvider).toEqual(expect.objectContaining({
      defaultApiPath: '/v1/responses',
      strategy: 'openai_responses',
    }));
    expect(anthropicProvider).toEqual(expect.objectContaining({
      supportsModelListing: false,
      strategy: 'anthropic',
    }));
    expect(geminiProvider).toEqual(expect.objectContaining({
      supportsModelListing: true,
      strategy: 'gemini',
    }));
  });

  it('lists built-in and custom provider definitions together', () => {
    const customProviders: Record<`custom-${string}`, CustomLlmProvider> = {
      'custom-private-gateway': {
        id: 'custom-private-gateway',
        name: 'Private Gateway',
        strategy: 'openai_compatible' as const,
        createdAt: '2026-05-18T00:00:00.000Z',
      },
    };

    const definitions = listProviderDefinitions(customProviders);
    expect(definitions[definitions.length - 1]).toEqual(expect.objectContaining({
      id: 'custom-private-gateway',
      label: 'Private Gateway',
    }));
    expect(getProviderDefinition('custom-private-gateway', customProviders)).toEqual(expect.objectContaining({
      id: 'custom-private-gateway',
      strategy: 'openai_compatible',
    }));
  });

  it('generates readable unique custom provider ids from names', () => {
    expect(createCustomProviderId('OpenAI Compatible', {})).toBe('custom-openai-compatible');
    expect(createCustomProviderId('OpenAI Compatible', {
      'custom-openai-compatible': {
        id: 'custom-openai-compatible',
        name: 'OpenAI Compatible',
        strategy: 'openai_compatible',
        createdAt: '2026-05-18T00:00:00.000Z',
      },
    })).toBe('custom-openai-compatible-2');
  });
});
