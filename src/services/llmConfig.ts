import { LlmConfig, LlmProvider } from '../types/transcript';

export const DEFAULT_LLM_URLS: Record<LlmProvider, string> = {
  open_ai: 'https://api.openai.com/v1',
  anthropic: 'https://api.anthropic.com',
  ollama: 'http://localhost:11434/v1',
  gemini: 'https://generativelanguage.googleapis.com',
  deep_seek: 'https://api.deepseek.com',
  kimi: 'https://api.moonshot.cn/v1',
  silicon_flow: 'https://api.siliconflow.cn/v1',
  open_ai_compatible: '',
};

export const DEFAULT_LLM_CONFIG: LlmConfig = {
  provider: 'open_ai',
  baseUrl: DEFAULT_LLM_URLS.open_ai,
  apiKey: '',
  model: '',
  temperature: 0.7,
};

export function getDefaultLlmConfig(provider: LlmProvider): LlmConfig {
  return {
    provider,
    baseUrl: DEFAULT_LLM_URLS[provider] || '',
    apiKey: '',
    model: '',
    temperature: 0.7,
  };
}
