import { describe, expect, it } from 'vitest';
import {
  buildLlmConfig,
  createProviderSetting,
  DEFAULT_LLM_CONFIG,
  DEFAULT_LLM_PROVIDER,
  DEFAULT_LLM_TEMPERATURE,
  getDefaultLlmConfig,
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
      apiPath: '/v1/chat/completions',
      apiVersion: undefined,
      temperature: DEFAULT_LLM_TEMPERATURE,
    });
  });

  it('keeps the exported default runtime config in sync with the default provider settings', () => {
    expect(DEFAULT_LLM_CONFIG).toEqual(getDefaultLlmConfig(DEFAULT_LLM_PROVIDER));
  });
});
