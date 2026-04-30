import { LlmProvider, LlmProviderSetting } from '../../../types/transcript';
import { LlmAssistantConfig } from '../../../types/config';
import { ensureLlmState } from '../../../services/llm/migration';
import { getProviderDefinition } from '../../../services/llm/providers';

export function getCurrentLlmSettings(config: LlmAssistantConfig) {
  return config.llmSettings ?? ensureLlmState(config).llmSettings;
}

export function getCurrentLlmState(config: LlmAssistantConfig) {
  return config.llmSettings ? { llmSettings: config.llmSettings } : ensureLlmState(config);
}

export function getModelPlaceholder(provider: LlmProvider): string {
  switch (provider) {
    case 'azure_openai': return 'gpt-4o-deployment';
    case 'anthropic': return 'claude-sonnet-4-20250514';
    case 'gemini': return 'gemini-2.5-flash';
    case 'ollama': return 'qwen3:8b';
    case 'deep_seek': return 'deepseek-chat';
    case 'kimi': return 'moonshot-v1-8k';
    case 'qwen':
    case 'qwen_portal': return 'qwen-max';
    case 'groq': return 'llama-3.3-70b-versatile';
    case 'x_ai': return 'grok-3-mini';
    case 'mistral_ai': return 'mistral-large-latest';
    case 'perplexity': return 'sonar';
    case 'google_translate':
    case 'google_translate_free': return 'default';
    default: return 'gpt-4o-mini';
  }
}

export function isProviderConfigured(provider: LlmProvider, setting: LlmProviderSetting | undefined): boolean {
  const def = getProviderDefinition(provider);

  if (def.requiresApiKey) {
    if (!setting || !(setting.apiKey || '').trim()) return false;
  }

  const effectiveHost = setting?.apiHost || def.defaultApiHost;
  if (!effectiveHost || !effectiveHost.trim()) return false;

  return true;
}
