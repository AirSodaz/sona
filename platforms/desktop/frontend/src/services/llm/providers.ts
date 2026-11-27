import llmProvidersManifest from '../../../../../../core/src/llm/llm-providers.json';
import type {
  BuiltInLlmProvider,
  CustomLlmProvider,
  CustomLlmProviderId,
  CustomLlmProviderStrategy,
  LlmConfig,
  LlmProvider,
  LlmProviderSetting,
  LlmProviderStrategy,
} from '../../types/transcript';

export interface LlmProviderDefinition {
  id: LlmProvider;
  category?: 'llm' | 'translation';
  labelKey: string;
  labelDefault: string;
  strategy: LlmProviderStrategy;
  defaultApiHost: string;
  apiHostPlaceholder?: string;
  defaultApiPath?: string;
  defaultApiVersion?: string;
  supportsModelListing: boolean;
  requiresApiKey: boolean;
  requiresAppId?: boolean;
  supportsRegion?: boolean;
  apiHostLabelKey?: string;
  apiHostLabelDefault?: string;
  appIdLabelKey?: string;
  appIdLabelDefault?: string;
  apiKeyLabelKey?: string;
  apiKeyLabelDefault?: string;
  apiRegionLabelKey?: string;
  apiRegionLabelDefault?: string;
  modelLabelKey?: string;
  modelLabelDefault?: string;
  editableApiHost?: boolean;
}

export const DEFAULT_LLM_TEMPERATURE = 0.7;
export const DEFAULT_LLM_PROVIDER: LlmProvider = 'google_translate_free';

export type CustomLlmProviderInput = Omit<CustomLlmProvider, 'id'> & {
  id?: CustomLlmProviderId;
};

// This registry is the durable source for provider-specific defaults. Feature configs
// derive from it later, so adding or changing provider behavior should start here.
const rawManifestDefinitions: LlmProviderDefinition[] = llmProvidersManifest.providers.map((p) => ({
  id: p.id as LlmProvider,
  labelKey: p.ui.labelKey,
  labelDefault: p.ui.labelDefault,
  strategy: p.strategy as LlmProviderStrategy,
  defaultApiHost: p.defaults.apiHost,
  defaultApiPath: p.defaults.apiPath,
  defaultApiVersion: p.defaults.apiVersion,
  supportsModelListing: p.capabilities.supportsModelListing,
  requiresApiKey: p.capabilities.requiresApiKey,
  editableApiHost: p.capabilities.editableApiHost,
  apiHostLabelKey: p.ui.apiHostLabelKey,
  apiHostLabelDefault: p.ui.apiHostLabelDefault,
  modelLabelKey: p.ui.modelLabelKey,
  modelLabelDefault: p.ui.modelLabelDefault,
}));

const localDefinition = rawManifestDefinitions.find((p) => p.id === 'local');
const nonLocalManifestDefinitions = rawManifestDefinitions.filter((p) => p.id !== 'local');

export const BUILT_IN_LLM_PROVIDER_DEFINITIONS: LlmProviderDefinition[] = [
  ...(localDefinition ? [localDefinition] : []),
  ...nonLocalManifestDefinitions,
  {
    id: 'google_translate_free',
    category: 'translation',
    labelKey: 'settings.llm_providers.google_translate_free',
    labelDefault: 'Google Translate (Free)',
    strategy: 'google_translate_free',
    defaultApiHost: 'https://translate.googleapis.com/translate_a/single',
    supportsModelListing: false,
    requiresApiKey: false,
    editableApiHost: false,
  },
  {
    id: 'google_translate',
    category: 'translation',
    labelKey: 'settings.llm_providers.google_translate',
    labelDefault: 'Google Translate (API)',
    strategy: 'google_translate',
    defaultApiHost: 'https://translation.googleapis.com/language/translate/v2',
    supportsModelListing: false,
    requiresApiKey: true,
    editableApiHost: false,
  },
  {
    id: 'deepl',
    category: 'translation',
    labelKey: 'settings.llm_providers.deepl',
    labelDefault: 'DeepL',
    strategy: 'deepl',
    defaultApiHost: 'https://api-free.deepl.com',
    apiHostPlaceholder: 'https://api-free.deepl.com',
    supportsModelListing: false,
    requiresApiKey: true,
    editableApiHost: true,
  },
  {
    id: 'microsoft_translator',
    category: 'translation',
    labelKey: 'settings.llm_providers.microsoft_translator',
    labelDefault: 'Microsoft Translator',
    strategy: 'microsoft_translator',
    defaultApiHost: 'https://api.cognitive.microsofttranslator.com',
    apiHostPlaceholder: 'https://api.cognitive.microsofttranslator.com',
    supportsModelListing: false,
    requiresApiKey: true,
    supportsRegion: true,
    apiRegionLabelKey: 'settings.llm.api_region',
    apiRegionLabelDefault: 'Region (Optional)',
    editableApiHost: true,
  },
  {
    id: 'libre_translate',
    category: 'translation',
    labelKey: 'settings.llm_providers.libre_translate',
    labelDefault: 'LibreTranslate',
    strategy: 'libre_translate',
    defaultApiHost: '',
    apiHostPlaceholder: 'http://127.0.0.1:5000',
    supportsModelListing: false,
    requiresApiKey: false,
    editableApiHost: true,
  },
  {
    id: 'deeplx',
    category: 'translation',
    labelKey: 'settings.llm_providers.deeplx',
    labelDefault: 'DeepLX',
    strategy: 'deeplx',
    defaultApiHost: '',
    apiHostPlaceholder: 'http://127.0.0.1:1188/translate',
    supportsModelListing: false,
    requiresApiKey: false,
    editableApiHost: true,
  },
  {
    id: 'baidu_translate',
    category: 'translation',
    labelKey: 'settings.llm_providers.baidu_translate',
    labelDefault: 'Baidu Translate',
    strategy: 'baidu_translate',
    defaultApiHost: 'https://fanyi-api.baidu.com/api/trans/vip/translate',
    apiHostPlaceholder: 'https://fanyi-api.baidu.com/api/trans/vip/translate',
    supportsModelListing: false,
    requiresApiKey: true,
    requiresAppId: true,
    appIdLabelKey: 'settings.llm.app_id',
    appIdLabelDefault: 'App ID',
    apiKeyLabelKey: 'settings.llm.secret_key',
    apiKeyLabelDefault: 'Secret Key',
    editableApiHost: true,
  },
  {
    id: 'youdao_translate',
    category: 'translation',
    labelKey: 'settings.llm_providers.youdao_translate',
    labelDefault: 'Youdao Translate',
    strategy: 'youdao_translate',
    defaultApiHost: 'https://openapi.youdao.com/api',
    apiHostPlaceholder: 'https://openapi.youdao.com/api',
    supportsModelListing: false,
    requiresApiKey: true,
    requiresAppId: true,
    appIdLabelKey: 'settings.llm.app_key',
    appIdLabelDefault: 'App Key',
    apiKeyLabelKey: 'settings.llm.app_secret',
    apiKeyLabelDefault: 'App Secret',
    editableApiHost: true,
  },
];

export const LLM_PROVIDER_DEFINITIONS = BUILT_IN_LLM_PROVIDER_DEFINITIONS;

export const LLM_PROVIDER_MAP: Record<BuiltInLlmProvider, LlmProviderDefinition> =
  BUILT_IN_LLM_PROVIDER_DEFINITIONS.reduce(
    (acc, provider) => {
      acc[provider.id as BuiltInLlmProvider] = provider;
      return acc;
    },
    {} as Record<BuiltInLlmProvider, LlmProviderDefinition>
  );

// Older config snapshots used several provider spellings. Normalize them before any
// migration logic so the rest of the file only reasons about canonical ids.
const LEGACY_PROVIDER_MAP: Record<string, LlmProvider> = {
  anthropic: 'anthropic',
  azure_open_ai: 'azure_openai',
  azure_openai: 'azure_openai',
  deep_seek: 'deep_seek',
  deepseek: 'deep_seek',
  gemini: 'gemini',
  kimi: 'moonshot_cn',
  moonshot: 'moonshot_cn',
  ollama: 'ollama',
  open_ai: 'open_ai',
  openai_compatible: 'custom-openai-compatible',
  open_ai_compatible: 'custom-openai-compatible',
  openai: 'open_ai',
  silicon_flow: 'silicon_flow',
  siliconflow: 'silicon_flow',
  llama_cpp: 'local',
  local_model: 'local',
};

function isBuiltInProvider(value: unknown): value is BuiltInLlmProvider {
  return typeof value === 'string' && Object.hasOwn(LLM_PROVIDER_MAP, value);
}

export function isCustomProviderId(value: unknown): value is CustomLlmProviderId {
  return typeof value === 'string' && /^custom-[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/.test(value);
}

export function normalizeProvider(value: unknown): LlmProvider {
  if (isBuiltInProvider(value) || isCustomProviderId(value)) {
    return value;
  }
  if (typeof value === 'string' && value in LEGACY_PROVIDER_MAP) {
    return LEGACY_PROVIDER_MAP[value];
  }
  return DEFAULT_LLM_PROVIDER;
}

export function createCustomProviderId(
  name: string,
  existingProviders: Partial<Record<LlmProvider, unknown>> | undefined
): CustomLlmProviderId {
  const normalized = name
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
  const baseId = `custom-${normalized || 'provider'}` as CustomLlmProviderId;
  if (!existingProviders?.[baseId]) {
    return baseId;
  }

  let suffix = 2;
  while (existingProviders[`${baseId}-${suffix}` as LlmProvider]) {
    suffix += 1;
  }
  return `${baseId}-${suffix}` as CustomLlmProviderId;
}

function customProviderDefaults(
  strategy: CustomLlmProviderStrategy
): Pick<
  LlmProviderDefinition,
  'defaultApiHost' | 'defaultApiPath' | 'supportsModelListing' | 'requiresApiKey'
> {
  switch (strategy) {
    case 'openai_responses':
      return {
        defaultApiHost: '',
        defaultApiPath: '/v1/responses',
        supportsModelListing: true,
        requiresApiKey: true,
      };
    case 'anthropic':
      return {
        defaultApiHost: '',
        supportsModelListing: false,
        requiresApiKey: true,
      };
    case 'gemini':
      return {
        defaultApiHost: '',
        supportsModelListing: true,
        requiresApiKey: true,
      };
    case 'openai_compatible':
    default:
      return {
        defaultApiHost: '',
        defaultApiPath: '/v1/chat/completions',
        supportsModelListing: true,
        requiresApiKey: true,
      };
  }
}

export function createCustomProviderDefinition(provider: CustomLlmProvider): LlmProviderDefinition {
  return {
    id: provider.id,
    labelKey: provider.name,
    labelDefault: provider.name,
    strategy: provider.strategy,
    editableApiHost: true,
    ...customProviderDefaults(provider.strategy),
  };
}

export function listProviderDefinitions(
  customProviders?: Partial<Record<LlmProvider, CustomLlmProvider>>
): LlmProviderDefinition[] {
  return [
    ...BUILT_IN_LLM_PROVIDER_DEFINITIONS,
    ...Object.values(customProviders ?? {})
      .filter((provider): provider is CustomLlmProvider =>
        Boolean(provider?.id && provider.name && provider.strategy)
      )
      .map(createCustomProviderDefinition),
  ];
}

export function getProviderDefinition(
  provider: LlmProvider,
  customProviders?: Partial<Record<LlmProvider, CustomLlmProvider>>
): LlmProviderDefinition {
  if (isBuiltInProvider(provider)) {
    const definition = LLM_PROVIDER_MAP[provider];
    if (definition) {
      return definition;
    }
  }

  const customProvider = customProviders?.[provider];
  if (customProvider) {
    return createCustomProviderDefinition(customProvider);
  }

  return createCustomProviderDefinition({
    id: provider as CustomLlmProviderId,
    name: provider,
    strategy: 'openai_compatible',
    createdAt: '',
  });
}

export function isTranslationOnlyProvider(
  provider: LlmProvider,
  customProviders?: Partial<Record<LlmProvider, CustomLlmProvider>>
): boolean {
  return getProviderDefinition(provider, customProviders).category === 'translation';
}

export function createProviderSetting(
  provider: LlmProvider,
  customProviders?: Partial<Record<LlmProvider, CustomLlmProvider>>
): LlmProviderSetting {
  const definition = getProviderDefinition(provider, customProviders);
  return {
    apiHost: definition.defaultApiHost,
    apiKey: '',
    apiPath: definition.defaultApiPath,
    apiVersion: definition.defaultApiVersion,
    ...(definition.requiresAppId ? { appId: '' } : {}),
    ...(definition.supportsRegion ? { apiRegion: '' } : {}),
  };
}

export function buildLlmConfig(
  provider: LlmProvider,
  setting: LlmProviderSetting,
  customProviders?: Partial<Record<LlmProvider, CustomLlmProvider>>
): LlmConfig {
  const definition = getProviderDefinition(provider, customProviders);

  // Google Translate Free must always hit the official endpoint. This is a
  // second safety net: even if a corrupted provider setting somehow reaches
  // here, the runtime config will still use the correct URL.
  if (provider === 'google_translate_free') {
    return {
      provider,
      strategy: definition.strategy,
      baseUrl: definition.defaultApiHost,
      apiKey: '',
      model: '',
      apiPath: undefined,
      apiVersion: undefined,
      temperature: DEFAULT_LLM_TEMPERATURE,
    };
  }

  // This returns a provider-level runtime snapshot without picking a concrete model yet.
  // Feature helpers layer the selected model and temperature on top of this base shape.
  const effectiveBaseUrl =
    definition.editableApiHost === false
      ? definition.defaultApiHost
      : setting.apiHost || definition.defaultApiHost;

  return {
    provider,
    strategy: definition.strategy,
    baseUrl: effectiveBaseUrl,
    apiKey: setting.apiKey,
    model: '',
    apiPath: setting.apiPath,
    apiVersion: setting.apiVersion,
    appId: setting.appId,
    apiRegion: setting.apiRegion,
    temperature: DEFAULT_LLM_TEMPERATURE,
  };
}

export function getDefaultLlmConfig(provider: LlmProvider): LlmConfig {
  return buildLlmConfig(provider, createProviderSetting(provider));
}

export const DEFAULT_LLM_CONFIG: LlmConfig = getDefaultLlmConfig(DEFAULT_LLM_PROVIDER);
