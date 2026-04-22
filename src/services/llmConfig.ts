import {
  AppConfig,
  LlmConfig,
  LlmFeature,
  LlmModelEntry,
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
export const DEFAULT_LLM_PROVIDER: LlmProvider = 'google_translate_free';

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

const FEATURE_MODEL_SELECTION_KEYS = {
  polish: 'polishModelId',
  translation: 'translationModelId',
  summary: 'summaryModelId',
} as const;

const FEATURE_TEMPERATURE_SELECTION_KEYS = {
  polish: 'polishTemperature',
  translation: 'translationTemperature',
  summary: 'summaryTemperature',
} as const;

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
    apiPath: definition.defaultApiPath,
    apiVersion: definition.defaultApiVersion,
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
    apiPath: setting?.apiPath ?? defaults.apiPath,
    apiVersion: setting?.apiVersion ?? defaults.apiVersion,
  };
}

function createEmptyModelState() {
  return {
    models: {} as Record<string, LlmModelEntry>,
    modelOrder: [] as string[],
    selections: {} as LlmSettings['selections'],
  };
}

function createModelId(provider: LlmProvider, model: string): string {
  const normalizedModel = model.trim().toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '');
  return normalizedModel ? `${provider}-${normalizedModel}` : `${provider}-model`;
}

function ensureUniqueModelId(models: Record<string, LlmModelEntry>, baseId: string): string {
  if (!models[baseId]) {
    return baseId;
  }

  let suffix = 2;
  while (models[`${baseId}-${suffix}`]) {
    suffix += 1;
  }
  return `${baseId}-${suffix}`;
}

function sanitizeModelEntry(entry: Partial<LlmModelEntry> | null | undefined): LlmModelEntry | null {
  if (!entry) {
    return null;
  }

  const provider = normalizeProvider(entry.provider);
  const model = typeof entry.model === 'string' ? entry.model.trim() : '';
  const id = typeof entry.id === 'string' ? entry.id.trim() : '';
  if (!id || !model) {
    return null;
  }

  return {
    id,
    provider,
    model,
  };
}

function normalizeStoredModels(rawModels: unknown): Record<string, LlmModelEntry> {
  if (!rawModels || typeof rawModels !== 'object') {
    return {};
  }

  const models: Record<string, LlmModelEntry> = {};
  for (const rawEntry of Object.values(rawModels as Record<string, unknown>)) {
    const entry = sanitizeModelEntry(rawEntry as Partial<LlmModelEntry>);
    if (entry) {
      models[entry.id] = entry;
    }
  }
  return models;
}

function normalizeStoredModelOrder(rawOrder: unknown, models: Record<string, LlmModelEntry>): string[] {
  const seen = new Set<string>();
  const modelIds = Object.keys(models);
  const ordered: string[] = [];

  if (Array.isArray(rawOrder)) {
    for (const value of rawOrder) {
      if (typeof value === 'string' && models[value] && !seen.has(value)) {
        seen.add(value);
        ordered.push(value);
      }
    }
  }

  for (const modelId of modelIds) {
    if (!seen.has(modelId)) {
      seen.add(modelId);
      ordered.push(modelId);
    }
  }

  return ordered;
}

function normalizeTemperature(value: unknown): number | undefined {
  return typeof value === 'number' && value >= 0 && value <= 2 ? value : undefined;
}

function normalizeStoredSelections(rawSelections: unknown, models: Record<string, LlmModelEntry>) {
  if (!rawSelections || typeof rawSelections !== 'object') {
    return {};
  }

  const selections = rawSelections as Record<string, unknown>;
  return {
    polishModelId:
      typeof selections.polishModelId === 'string' && models[selections.polishModelId]
        ? selections.polishModelId
        : undefined,
    translationModelId:
      typeof selections.translationModelId === 'string' && models[selections.translationModelId]
        ? selections.translationModelId
        : undefined,
    summaryModelId:
      typeof selections.summaryModelId === 'string' && models[selections.summaryModelId]
        ? selections.summaryModelId
        : undefined,
    polishTemperature: normalizeTemperature(selections.polishTemperature),
    translationTemperature: normalizeTemperature(selections.translationTemperature),
    summaryTemperature: normalizeTemperature(selections.summaryTemperature),
  };
}

function applyLegacyTemperature(
  llmSettings: LlmSettings,
  legacyTemperature: unknown,
): LlmSettings {
  const normalizedTemperature = normalizeTemperature(legacyTemperature);
  if (normalizedTemperature === undefined) {
    return llmSettings;
  }

  return {
    ...llmSettings,
    selections: {
      ...llmSettings.selections,
      polishTemperature: llmSettings.selections.polishTemperature ?? normalizedTemperature,
      translationTemperature: llmSettings.selections.translationTemperature ?? normalizedTemperature,
    },
  };
}

function supportsSummaryModel(modelEntry: LlmModelEntry | undefined): boolean {
  if (!modelEntry) {
    return false;
  }

  return modelEntry.provider !== 'google_translate' && modelEntry.provider !== 'google_translate_free';
}

export function createLlmSettings(activeProvider: LlmProvider = DEFAULT_LLM_PROVIDER): LlmSettings {
  return {
    activeProvider,
    providers: {
      [activeProvider]: createProviderSetting(activeProvider),
    },
    ...createEmptyModelState(),
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
    ...current,
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
    ...current,
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

export function addLlmModel(
  llmSettings: LlmSettings | undefined,
  entry: Pick<LlmModelEntry, 'provider' | 'model'>,
): LlmSettings {
  const current = llmSettings ?? createLlmSettings(entry.provider);
  const model = entry.model.trim();
  if (!model) {
    return current;
  }

  const existingId = current.modelOrder.find((modelId) => {
    const existing = current.models[modelId];
    return existing?.provider === entry.provider && existing.model === model;
  });
  if (existingId) {
    return current;
  }

  const nextId = ensureUniqueModelId(current.models, createModelId(entry.provider, model));
  return {
    ...current,
    models: {
      ...current.models,
      [nextId]: {
        id: nextId,
        provider: entry.provider,
        model,
      },
    },
    modelOrder: [...current.modelOrder, nextId],
  };
}

export function removeLlmModel(
  llmSettings: LlmSettings | undefined,
  modelId: string,
): LlmSettings {
  const current = llmSettings ?? createLlmSettings();
  if (!current.models[modelId]) {
    return current;
  }

  const nextModels = { ...current.models };
  delete nextModels[modelId];

  return {
    ...current,
    models: nextModels,
    modelOrder: current.modelOrder.filter((id) => id !== modelId),
    selections: {
      polishModelId: current.selections.polishModelId === modelId ? undefined : current.selections.polishModelId,
      translationModelId:
        current.selections.translationModelId === modelId ? undefined : current.selections.translationModelId,
      summaryModelId: current.selections.summaryModelId === modelId ? undefined : current.selections.summaryModelId,
      polishTemperature: current.selections.polishTemperature,
      translationTemperature: current.selections.translationTemperature,
      summaryTemperature: current.selections.summaryTemperature,
    },
  };
}

export function setFeatureModelSelection(
  llmSettings: LlmSettings | undefined,
  feature: LlmFeature,
  modelId: string | undefined,
): LlmSettings {
  const current = llmSettings ?? createLlmSettings();
  const key = FEATURE_MODEL_SELECTION_KEYS[feature];
  return {
    ...current,
    selections: {
      ...current.selections,
      [key]: modelId && current.models[modelId] ? modelId : undefined,
    },
  };
}

export function setFeatureTemperature(
  llmSettings: LlmSettings | undefined,
  feature: LlmFeature,
  temperature: number | undefined,
): LlmSettings {
  const current = llmSettings ?? createLlmSettings();
  const key = FEATURE_TEMPERATURE_SELECTION_KEYS[feature];
  return {
    ...current,
    selections: {
      ...current.selections,
      [key]: normalizeTemperature(temperature),
    },
  };
}

export function getOrderedLlmModels(llmSettings: LlmSettings | undefined): LlmModelEntry[] {
  const current = llmSettings ?? createLlmSettings();
  return current.modelOrder
    .map((modelId) => current.models[modelId])
    .filter((entry): entry is LlmModelEntry => Boolean(entry));
}

export function getFeatureModelId(
  config: Pick<AppConfig, 'llmSettings'>,
  feature: LlmFeature,
): string | undefined {
  if (!config.llmSettings) {
    return undefined;
  }

  return config.llmSettings.selections[FEATURE_MODEL_SELECTION_KEYS[feature]];
}

export function getFeatureModelEntry(
  config: Pick<AppConfig, 'llmSettings'>,
  feature: LlmFeature,
): LlmModelEntry | null {
  if (!config.llmSettings) {
    return null;
  }

  const modelId = getFeatureModelId(config, feature);
  return modelId ? config.llmSettings.models[modelId] ?? null : null;
}

export function buildLlmConfig(provider: LlmProvider, setting: LlmProviderSetting): LlmConfig {
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

function extractLegacyProviderSetting(source: Record<string, any>): Partial<LlmProviderSetting> {
  return {
    apiHost: source.llmBaseUrl || source.aiBaseUrl || source.baseUrl || undefined,
    apiKey: source.llmApiKey || source.aiApiKey || source.apiKey || undefined,
    apiPath: source.llmApiPath || source.aiApiPath || source.apiPath || undefined,
    apiVersion: source.llmApiVersion || source.aiApiVersion || source.apiVersion || undefined,
  };
}

function extractLegacyModel(source: Record<string, any>): { provider: LlmProvider; model: string } | null {
  const provider = normalizeProvider(
    source.llmSettings?.activeProvider ?? source.llm?.provider ?? source.llmServiceType,
  );
  let model = '';
  if (typeof source.llm?.model === 'string' && source.llm.model.trim()) {
    model = source.llm.model.trim();
  } else if (typeof source.llmModel === 'string' && source.llmModel.trim()) {
    model = source.llmModel.trim();
  } else if (typeof source.aiModel === 'string' && source.aiModel.trim()) {
    model = source.aiModel.trim();
  } else if (typeof source.model === 'string' && source.model.trim()) {
    model = source.model.trim();
  }

  return model ? { provider, model } : null;
}

function normalizeStoredProviders(rawProviders: unknown): Partial<Record<LlmProvider, LlmProviderSetting>> {
  if (!rawProviders || typeof rawProviders !== 'object') {
    return {};
  }

  const providers: Partial<Record<LlmProvider, LlmProviderSetting>> = {};

  for (const [rawProvider, rawSetting] of Object.entries(rawProviders as Record<string, unknown>)) {
    const provider = normalizeProvider(rawProvider);
    const setting = rawSetting as Partial<LlmProviderSetting> & { model?: string };
    providers[provider] = sanitizeProviderSetting(provider, {
      apiHost: setting.apiHost,
      apiKey: setting.apiKey,
      apiPath: setting.apiPath,
      apiVersion: setting.apiVersion,
    });
  }

  return providers;
}

export function ensureLlmState(
  source?: Partial<AppConfig> & Record<string, any>,
): { llmSettings: LlmSettings } {
  const candidate = source ?? {};
  const currentProvider = normalizeProvider(
    candidate.llmSettings?.activeProvider ??
      candidate.llmServiceType ??
      candidate.llm?.provider,
  );

  const providers = normalizeStoredProviders(candidate.llmSettings?.providers);

  if (candidate.llm) {
    const provider = normalizeProvider(candidate.llm.provider);
    providers[provider] = sanitizeProviderSetting(provider, {
      apiHost: candidate.llm.baseUrl || undefined,
      apiKey: candidate.llm.apiKey || undefined,
      apiPath: candidate.llm.apiPath || undefined,
      apiVersion: candidate.llm.apiVersion || undefined,
    });
  } else {
    const legacySetting = extractLegacyProviderSetting(candidate);
    if (
      legacySetting.apiHost ||
      legacySetting.apiKey ||
      legacySetting.apiPath ||
      legacySetting.apiVersion
    ) {
      providers[currentProvider] = sanitizeProviderSetting(currentProvider, {
        ...legacySetting,
        ...(providers[currentProvider] ?? {}),
      });
    }
  }

  const storedModels = normalizeStoredModels(candidate.llmSettings?.models);
  const storedModelOrder = normalizeStoredModelOrder(candidate.llmSettings?.modelOrder, storedModels);
  const storedSelections = normalizeStoredSelections(candidate.llmSettings?.selections, storedModels);

  let llmSettings: LlmSettings = {
    activeProvider: currentProvider,
    providers: {
      ...providers,
      [currentProvider]: sanitizeProviderSetting(currentProvider, providers[currentProvider]),
    },
    models: storedModels,
    modelOrder: storedModelOrder,
    selections: storedSelections,
  };

  const legacyStoredProviderModel = Object.entries(candidate.llmSettings?.providers ?? {}).find(([, rawSetting]) => {
    const model = (rawSetting as { model?: string } | undefined)?.model;
    return typeof model === 'string' && model.trim();
  });

  const legacyModel = legacyStoredProviderModel
    ? {
        provider: normalizeProvider(legacyStoredProviderModel[0]),
        model: ((legacyStoredProviderModel[1] as { model?: string }).model || '').trim(),
      }
    : extractLegacyModel(candidate);

  if (llmSettings.modelOrder.length === 0) {
    if (legacyModel) {
      llmSettings = addLlmModel(llmSettings, legacyModel);
      const migratedModelId = llmSettings.modelOrder[0];
      llmSettings = setFeatureModelSelection(llmSettings, 'polish', migratedModelId);
      llmSettings = setFeatureModelSelection(llmSettings, 'translation', migratedModelId);
    } else {
      // Default fallback for new users or fresh state
      llmSettings = addLlmModel(llmSettings, { provider: 'google_translate_free', model: 'default' });
      const defaultModelId = llmSettings.modelOrder[0];
      llmSettings = setFeatureModelSelection(llmSettings, 'translation', defaultModelId);
    }
  }

  llmSettings = applyLegacyTemperature(llmSettings, candidate.llm?.temperature);

  const summaryModelId = llmSettings.selections.summaryModelId;
  if (!summaryModelId) {
    const polishModelId = llmSettings.selections.polishModelId;
    const polishModelEntry = polishModelId ? llmSettings.models[polishModelId] : undefined;
    if (polishModelId && supportsSummaryModel(polishModelEntry)) {
      llmSettings = setFeatureModelSelection(llmSettings, 'summary', polishModelId);
    }
  }

  return {
    llmSettings,
  };
}

export function getActiveProvider(config: Pick<AppConfig, 'llmSettings'>): LlmProvider {
  return normalizeProvider(config.llmSettings?.activeProvider);
}

export function getActiveProviderSetting(config: Pick<AppConfig, 'llmSettings'>): LlmProviderSetting {
  const provider = getActiveProvider(config);
  return ensureProviderSetting(config.llmSettings, provider);
}

export function getActiveLlmConfig(config: Pick<AppConfig, 'llmSettings'>): LlmConfig {
  const provider = getActiveProvider(config);
  return buildLlmConfig(provider, getActiveProviderSetting(config));
}

function getFeatureTemperature(
  config: Pick<AppConfig, 'llmSettings'>,
  feature: LlmFeature,
): number | undefined {
  const selectionTemperature = config.llmSettings?.selections[FEATURE_TEMPERATURE_SELECTION_KEYS[feature]];

  return selectionTemperature ?? DEFAULT_LLM_TEMPERATURE;
}

export function getFeatureLlmConfig(
  config: Pick<AppConfig, 'llmSettings'>,
  feature: LlmFeature,
): LlmConfig | null {
  const modelEntry = getFeatureModelEntry(config, feature);
  if (!modelEntry) {
    return null;
  }

  const setting = ensureProviderSetting(config.llmSettings, modelEntry.provider);
  return {
    ...buildLlmConfig(modelEntry.provider, setting),
    model: modelEntry.model,
    temperature: getFeatureTemperature(config, feature),
  };
}

export function isLlmConfigComplete(llmConfig: LlmConfig | null): boolean {
  if (!llmConfig) {
    return false;
  }

  const definition = getProviderDefinition(llmConfig.provider);
  const hasApiHost = Boolean(llmConfig.baseUrl?.trim() || definition.defaultApiHost);
  const hasApiKey = !definition.requiresApiKey || Boolean(llmConfig.apiKey?.trim());
  const hasModel = Boolean(llmConfig.model?.trim());

  return hasApiHost && hasApiKey && hasModel;
}

export function isFeatureLlmConfigComplete(
  config: Pick<AppConfig, 'llmSettings'>,
  feature: LlmFeature,
): boolean {
  return isLlmConfigComplete(getFeatureLlmConfig(config, feature));
}

export function isSummaryLlmConfigComplete(
  config: Pick<AppConfig, 'llmSettings'>,
): boolean {
  return isFeatureLlmConfigComplete(config, 'summary');
}

export function buildLlmConfigPatch(
  nextLlmSettings: LlmSettings,
): Pick<AppConfig, 'llmSettings'> {
  return {
    llmSettings: nextLlmSettings,
  };
}
