import {
  AppConfig,
  LlmConfig,
  LlmProvider,
  LlmProviderSetting,
  LlmProviderStrategy,
  LlmSettings,
} from '../types/transcript';

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
export const DEFAULT_LLM_PROVIDER: LlmProvider = 'open_ai';

export const LLM_PROVIDER_DEFINITIONS: LlmProviderDefinition[] = [
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
  if (isProvider(value)) return value;
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
    model: '',
    apiPath: definition.defaultApiPath,
    apiVersion: definition.defaultApiVersion,
    temperature: DEFAULT_LLM_TEMPERATURE,
  };
}

function sanitizeProviderSetting(
  provider: LlmProvider,
  setting?: Partial<LlmProviderSetting> | null,
): LlmProviderSetting {
  const defaults = createProviderSetting(provider);
  return {
    ...defaults,
    ...(setting ?? {}),
    apiHost: setting?.apiHost ?? defaults.apiHost,
    apiKey: setting?.apiKey ?? defaults.apiKey,
    model: setting?.model ?? defaults.model,
    apiPath: setting?.apiPath ?? defaults.apiPath,
    apiVersion: setting?.apiVersion ?? defaults.apiVersion,
    temperature: setting?.temperature ?? defaults.temperature,
  };
}

export function createLlmSettings(activeProvider: LlmProvider = DEFAULT_LLM_PROVIDER): LlmSettings {
  return {
    activeProvider,
    providers: {
      [activeProvider]: createProviderSetting(activeProvider),
    },
  };
}

export function ensureProviderSetting(
  llmSettings: LlmSettings | undefined,
  provider: LlmProvider,
): LlmProviderSetting {
  return sanitizeProviderSetting(provider, llmSettings?.providers?.[provider]);
}

export function setActiveProvider(
  llmSettings: LlmSettings | undefined,
  provider: LlmProvider,
): LlmSettings {
  const current = llmSettings ?? createLlmSettings();
  return {
    activeProvider: provider,
    providers: {
      ...current.providers,
      [provider]: ensureProviderSetting(current, provider),
    },
  };
}

export function updateProviderSetting(
  llmSettings: LlmSettings | undefined,
  provider: LlmProvider,
  updates: Partial<LlmProviderSetting>,
): LlmSettings {
  const current = llmSettings ?? createLlmSettings(provider);
  return {
    activeProvider: current.activeProvider,
    providers: {
      ...current.providers,
      [provider]: sanitizeProviderSetting(provider, {
        ...ensureProviderSetting(current, provider),
        ...updates,
      }),
    },
  };
}

export function buildLlmConfig(provider: LlmProvider, setting: LlmProviderSetting): LlmConfig {
  return {
    provider,
    baseUrl: setting.apiHost,
    apiKey: setting.apiKey,
    model: setting.model,
    apiPath: setting.apiPath,
    apiVersion: setting.apiVersion,
    temperature: setting.temperature ?? DEFAULT_LLM_TEMPERATURE,
  };
}

export function getDefaultLlmConfig(provider: LlmProvider): LlmConfig {
  return buildLlmConfig(provider, createProviderSetting(provider));
}

export const DEFAULT_LLM_CONFIG: LlmConfig = getDefaultLlmConfig(DEFAULT_LLM_PROVIDER);

function extractLegacyProviderSetting(source: Record<string, any>): Partial<LlmProviderSetting> {
  return {
    apiHost: source.llmBaseUrl || source.aiBaseUrl || source.baseUrl || undefined,
    apiKey: source.llmApiKey || source.aiApiKey || source.apiKey || undefined,
    model: source.llmModel || source.aiModel || source.model || undefined,
    temperature:
      typeof source.llmTemperature === 'number'
        ? source.llmTemperature
        : typeof source.aiTemperature === 'number'
          ? source.aiTemperature
          : typeof source.temperature === 'number'
            ? source.temperature
            : undefined,
  };
}

function normalizeStoredProviders(rawProviders: unknown): Partial<Record<LlmProvider, LlmProviderSetting>> {
  if (!rawProviders || typeof rawProviders !== 'object') {
    return {};
  }

  const providers: Partial<Record<LlmProvider, LlmProviderSetting>> = {};

  for (const [rawProvider, rawSetting] of Object.entries(rawProviders as Record<string, unknown>)) {
    const provider = normalizeProvider(rawProvider);
    providers[provider] = sanitizeProviderSetting(provider, rawSetting as Partial<LlmProviderSetting>);
  }

  return providers;
}

export function ensureLlmState(
  source?: Partial<AppConfig> & Record<string, any>,
): { llmSettings: LlmSettings; llm: LlmConfig } {
  const candidate = source ?? {};
  const currentProvider = normalizeProvider(
    candidate.llmSettings?.activeProvider ??
      candidate.llm?.provider ??
      candidate.llmServiceType,
  );

  const providers = normalizeStoredProviders(candidate.llmSettings?.providers);

  if (candidate.llm) {
    const provider = normalizeProvider(candidate.llm.provider);
    providers[provider] = sanitizeProviderSetting(provider, {
      apiHost: candidate.llm.baseUrl || undefined,
      apiKey: candidate.llm.apiKey || undefined,
      model: candidate.llm.model || undefined,
      apiPath: candidate.llm.apiPath || undefined,
      apiVersion: candidate.llm.apiVersion || undefined,
      temperature: candidate.llm.temperature,
    });
  } else {
    const legacySetting = extractLegacyProviderSetting(candidate);
    if (legacySetting.apiHost || legacySetting.apiKey || legacySetting.model || legacySetting.temperature !== undefined) {
      providers[currentProvider] = sanitizeProviderSetting(currentProvider, {
        ...legacySetting,
        ...(providers[currentProvider] ?? {}),
      });
    }
  }

  const llmSettings: LlmSettings = {
    activeProvider: currentProvider,
    providers: {
      ...providers,
      [currentProvider]: sanitizeProviderSetting(currentProvider, providers[currentProvider]),
    },
  };

  return {
    llmSettings,
    llm: buildLlmConfig(currentProvider, ensureProviderSetting(llmSettings, currentProvider)),
  };
}

export function getActiveProvider(config: Pick<AppConfig, 'llmSettings' | 'llm'>): LlmProvider {
  return normalizeProvider(config.llmSettings?.activeProvider ?? config.llm?.provider);
}

export function getActiveProviderSetting(config: Pick<AppConfig, 'llmSettings' | 'llm'>): LlmProviderSetting {
  const provider = getActiveProvider(config);
  if (config.llmSettings) {
    return ensureProviderSetting(config.llmSettings, provider);
  }

  return sanitizeProviderSetting(provider, config.llm
    ? {
        apiHost: config.llm.baseUrl || undefined,
        apiKey: config.llm.apiKey || undefined,
        model: config.llm.model || undefined,
        apiPath: config.llm.apiPath || undefined,
        apiVersion: config.llm.apiVersion || undefined,
        temperature: config.llm.temperature,
      }
    : undefined);
}

export function getActiveLlmConfig(config: Pick<AppConfig, 'llmSettings' | 'llm'>): LlmConfig {
  const provider = getActiveProvider(config);
  return buildLlmConfig(provider, getActiveProviderSetting(config));
}

export function isLlmConfigComplete(llmConfig: LlmConfig): boolean {
  const definition = getProviderDefinition(llmConfig.provider);
  const hasApiHost = Boolean(llmConfig.baseUrl?.trim() || definition.defaultApiHost);
  const hasApiKey = !definition.requiresApiKey || Boolean(llmConfig.apiKey?.trim());
  const hasModel = Boolean(llmConfig.model?.trim());

  return hasApiHost && hasApiKey && hasModel;
}

export function buildLlmConfigPatch(
  nextLlmSettings: LlmSettings,
): Pick<AppConfig, 'llmSettings' | 'llm'> {
  return {
    llmSettings: nextLlmSettings,
    llm: buildLlmConfig(
      nextLlmSettings.activeProvider,
      ensureProviderSetting(nextLlmSettings, nextLlmSettings.activeProvider),
    ),
  };
}
