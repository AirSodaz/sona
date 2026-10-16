import { render } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { PROVIDER_ICON_MAP, ProviderLogo } from '../ProviderLogos';

describe('ProviderLogo', () => {
  it('renders an img element for known providers', () => {
    const { container } = render(<ProviderLogo provider="anthropic" label="Anthropic" size={32} />);
    const img = container.querySelector('img');
    expect(img).toBeTruthy();
    expect(img?.getAttribute('alt')).toBe('Anthropic');
    expect(img?.getAttribute('width')).toBe('32');
    expect(img?.getAttribute('height')).toBe('32');
    expect(img?.className).toContain('provider-brand-logo');
  });

  it('adds monochrome class for monochrome providers', () => {
    const { container } = render(<ProviderLogo provider="open_ai" label="OpenAI" />);
    const img = container.querySelector('img');
    expect(img).toBeTruthy();
    expect(img?.className).toContain('provider-brand-logo-monochrome');

    const { container: copilotContainer } = render(
      <ProviderLogo provider="copilot" label="GitHub Copilot" />
    );
    const copilotImg = copilotContainer.querySelector('img');
    expect(copilotImg).toBeTruthy();
    expect(copilotImg?.className).toContain('provider-brand-logo-monochrome');
  });

  it('does not add monochrome class for colored providers', () => {
    const { container } = render(<ProviderLogo provider="deep_seek" label="DeepSeek" />);
    const img = container.querySelector('img');
    expect(img).toBeTruthy();
    expect(img?.className).not.toContain('provider-brand-logo-monochrome');
  });

  it('renders fallback letter-circle for unknown/custom providers', () => {
    const { container } = render(
      <ProviderLogo provider="custom-gateway" label="Private Gateway" size={28} />
    );
    expect(container.querySelector('img')).toBeNull();
    const fallback = container.querySelector('.provider-picker-fallback-icon');
    expect(fallback).toBeTruthy();
    expect(fallback?.textContent).toBe('P');
  });

  it('maps all major built-in providers to valid SVGs', () => {
    const expectedProviders = [
      'open_ai',
      'azure_openai',
      'anthropic',
      'gemini',
      'ollama',
      'deep_seek',
      'moonshot_ai',
      'xiaomi',
      'silicon_flow',
      'qwen',
      'minimax_global',
      'openrouter',
      'lm_studio',
      'groq',
      'x_ai',
      'mistral_ai',
      'perplexity',
      'volcengine',
      'chatglm',
      'copilot',
      'cohere',
      'together',
      'venice',
      'hyperbolic',
      'llamafile',
      'baidu',
      'tencent',
      'stepfun',
      'lingyiwanwu',
      'cerebras',
      'google_translate',
      'google_translate_free',
    ];

    for (const id of expectedProviders) {
      expect(PROVIDER_ICON_MAP[id], `Missing mapping for ${id}`).toBeDefined();
      expect(PROVIDER_ICON_MAP[id].src).toBeTruthy();
    }
  });
});
