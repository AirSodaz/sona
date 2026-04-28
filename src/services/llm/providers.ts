import {
  LlmConfig,
  LlmProvider,
  LlmProviderSetting,
  LlmProviderStrategy,
} from '../../types/transcript';

export interface LlmProviderDefinition {
  id: LlmProvider;
  label: string;
  strategy: LlmProviderStrategy;
  defaultApiHost: string;
  defaultApiPath?: string;
  defaultApiVersion?: string;
  supportsModelListing: boolean;
  requiresApiKey: boolean;
  apiHostLabel?: string;
  modelLabel?: string;
  editableApiHost?: boolean;
}

export const DEFAULT_LLM_TEMPERATURE = 0.7;
export const DEFAULT_LLM_PROVIDER: LlmProvider = 'google_translate_free';

// This registry is the durable source for provider-specific defaults. Feature configs
// derive from it later, so adding or changing provider behavior should start here.
export const LLM_PROVIDER_DEFINITIONS: LlmProviderDefinition[] = [
  {
    id: 'google_translate_free',
    label: 'Google Translate (Free)',
    strategy: 'google_translate_free',
    defaultApiHost: 'https://translate.googleapis.com/translate_a/single',
    supportsModelListing: false,
    requiresApiKey: false,
  },
  {
    id: 'google_translate',
    label: 'Google Translate (API)',
    strategy: 'google_translate',
    defaultApiHost: 'https://translation.googleapis.com/language/translate/v2',
    supportsModelListing: false,
    requiresApiKey: true,
  },
  {
    id: 'open_ai',
    label: 'OpenAI',
    strategy: 'openai_compatible',
    defaultApiHost: 'https://api.openai.com',
    supportsModelListing: true,
    requiresApiKey: true,
  },
  {
    id: 'open_ai_responses',
    label: 'OpenAI Responses',
    strategy: 'openai_responses',
    defaultApiHost: 'https://api.openai.com',
    defaultApiPath: '/v1/responses',
    supportsModelListing: true,
    requiresApiKey: true,
  },
  {
    id: 'azure_openai',
    label: 'Azure OpenAI',
    strategy: 'azure_openai',
    defaultApiHost: '',
    defaultApiVersion: '2024-10-21',
    supportsModelListing: false,
    requiresApiKey: true,
    apiHostLabel: 'Endpoint',
    modelLabel: 'Deployment Name',
  },
  {
    id: 'anthropic',
    label: 'Claude',
    strategy: 'anthropic',
    defaultApiHost: 'https://api.anthropic.com',
    supportsModelListing: false,
    requiresApiKey: true,
  },
  {
    id: 'gemini',
    label: 'Gemini',
    strategy: 'gemini',
    defaultApiHost: 'https://generativelanguage.googleapis.com',
    supportsModelListing: true,
    requiresApiKey: true,
  },
  {
    id: 'ollama',
    label: 'Ollama',
    strategy: 'ollama',
    defaultApiHost: 'http://127.0.0.1:11434',
    supportsModelListing: true,
    requiresApiKey: false,
  },
  {
    id: 'deep_seek',
    label: 'DeepSeek',
    strategy: 'openai_compatible',
    defaultApiHost: 'https://api.deepseek.com',
    supportsModelListing: true,
    requiresApiKey: true,
  },
  {
    id: 'kimi',
    label: 'Kimi',
    strategy: 'openai_compatible',
    defaultApiHost: 'https://api.moonshot.cn',
    supportsModelListing: true,
    requiresApiKey: true,
  },
  {
    id: 'silicon_flow',
    label: 'SiliconFlow',
    strategy: 'openai_compatible',
    defaultApiHost: 'https://api.siliconflow.cn',
    supportsModelListing: true,
    requiresApiKey: true,
  },
  {
    id: 'qwen',
    label: 'Qwen',
    strategy: 'openai_compatible',
    defaultApiHost: 'https://dashscope.aliyuncs.com/compatible-mode/v1',
    supportsModelListing: true,
    requiresApiKey: true,
  },
  {
    id: 'qwen_portal',
    label: 'Qwen Portal',
    strategy: 'openai_compatible',
    defaultApiHost: 'https://portal.qwen.ai/v1',
    supportsModelListing: true,
    requiresApiKey: true,
  },
  {
    id: 'minimax_global',
    label: 'MiniMax Global',
    strategy: 'openai_compatible',
    defaultApiHost: 'https://api.minimaxi.chat/v1',
    supportsModelListing: true,
    requiresApiKey: true,
  },
  {
    id: 'minimax_cn',
    label: 'MiniMax CN',
    strategy: 'openai_compatible',
    defaultApiHost: 'https://api.minimax.chat/v1',
    supportsModelListing: true,
    requiresApiKey: true,
  },
  {
    id: 'openrouter',
    label: 'OpenRouter',
    strategy: 'openai_compatible',
    defaultApiHost: 'https://openrouter.ai/api/v1',
    supportsModelListing: true,
    requiresApiKey: true,
  },
  {
    id: 'lm_studio',
    label: 'LM Studio',
    strategy: 'openai_compatible',
    defaultApiHost: 'http://localhost:1234/v1',
    supportsModelListing: true,
    requiresApiKey: false,
  },
  {
    id: 'groq',
    label: 'Groq',
    strategy: 'openai_compatible',
    defaultApiHost: 'https://api.groq.com/openai',
    supportsModelListing: true,
    requiresApiKey: true,
  },
  {
    id: 'x_ai',
    label: 'xAI',
    strategy: 'openai_compatible',
    defaultApiHost: 'https://api.x.ai',
    supportsModelListing: true,
    requiresApiKey: true,
  },
  {
    id: 'mistral_ai',
    label: 'Mistral AI',
    strategy: 'openai_compatible',
    defaultApiHost: 'https://api.mistral.ai/v1',
    supportsModelListing: true,
    requiresApiKey: true,
  },
  {
    id: 'perplexity',
    label: 'Perplexity',
    strategy: 'perplexity',
    defaultApiHost: 'https://api.perplexity.ai',
    defaultApiPath: '/chat/completions',
    supportsModelListing: false,
    requiresApiKey: true,
    editableApiHost: false,
  },
  {
    id: 'volcengine',
    label: 'VolcEngine',
    strategy: 'openai_compatible_custom_path',
    defaultApiHost: 'https://ark.cn-beijing.volces.com',
    defaultApiPath: '/api/v3/chat/completions',
    supportsModelListing: false,
    requiresApiKey: true,
  },
  {
    id: 'chatglm',
    label: 'ChatGLM',
    strategy: 'openai_compatible',
    defaultApiHost: 'https://open.bigmodel.cn/api/paas/v4/',
    supportsModelListing: true,
    requiresApiKey: true,
  },
  {
    id: 'open_ai_compatible',
    label: 'OpenAI Compatible',
    strategy: 'openai_compatible',
    defaultApiHost: '',
    supportsModelListing: true,
    requiresApiKey: false,
  },
];

export const LLM_PROVIDER_MAP: Record<LlmProvider, LlmProviderDefinition> =
  LLM_PROVIDER_DEFINITIONS.reduce((acc, provider) => {
    acc[provider.id] = provider;
    return acc;
  }, {} as Record<LlmProvider, LlmProviderDefinition>);

// Older config snapshots used several provider spellings. Normalize them before any
// migration logic so the rest of the file only reasons about canonical ids.
const LEGACY_PROVIDER_MAP: Record<string, LlmProvider> = {
  anthropic: 'anthropic',
  azure_open_ai: 'azure_openai',
  azure_openai: 'azure_openai',
  deep_seek: 'deep_seek',
  deepseek: 'deep_seek',
  gemini: 'gemini',
  kimi: 'kimi',
  moonshot: 'kimi',
  ollama: 'ollama',
  open_ai: 'open_ai',
  openai_compatible: 'open_ai_compatible',
  open_ai_compatible: 'open_ai_compatible',
  openai: 'open_ai',
  silicon_flow: 'silicon_flow',
  siliconflow: 'silicon_flow',
};

function isProvider(value: unknown): value is LlmProvider {
  return typeof value === 'string' && value in LLM_PROVIDER_MAP;
}

export function normalizeProvider(value: unknown): LlmProvider {
  if (isProvider(value)) {
    return value;
  }
  if (typeof value === 'string' && value in LEGACY_PROVIDER_MAP) {
    return LEGACY_PROVIDER_MAP[value];
  }
  return DEFAULT_LLM_PROVIDER;
}

export function getProviderDefinition(provider: LlmProvider): LlmProviderDefinition {
  return LLM_PROVIDER_MAP[provider];
}

export function createProviderSetting(provider: LlmProvider): LlmProviderSetting {
  const definition = getProviderDefinition(provider);
  return {
    apiHost: definition.defaultApiHost,
    apiKey: '',
    apiPath: definition.defaultApiPath,
    apiVersion: definition.defaultApiVersion,
  };
}

export function buildLlmConfig(provider: LlmProvider, setting: LlmProviderSetting): LlmConfig {
  // This returns a provider-level runtime snapshot without picking a concrete model yet.
  // Feature helpers layer the selected model and temperature on top of this base shape.
  return {
    provider,
    baseUrl: setting.apiHost,
    apiKey: setting.apiKey,
    model: '',
    apiPath: setting.apiPath,
    apiVersion: setting.apiVersion,
    temperature: DEFAULT_LLM_TEMPERATURE,
  };
}

export function getDefaultLlmConfig(provider: LlmProvider): LlmConfig {
  return buildLlmConfig(provider, createProviderSetting(provider));
}

export const DEFAULT_LLM_CONFIG: LlmConfig = getDefaultLlmConfig(DEFAULT_LLM_PROVIDER);
