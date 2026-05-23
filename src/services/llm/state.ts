import type { AppConfig } from '../../types/config';
import {
  CustomLlmProvider,
  CustomLlmProviderStrategy,
  LlmDiscoveredModelSummary,
  LlmFeature,
  LlmModelEntry,
  LlmProvider,
  LlmProviderSetting,
  LlmSettings,
} from '../../types/transcript';
import {
  createCustomProviderId,
  createProviderSetting,
  DEFAULT_LLM_PROVIDER,
} from './providers';

// Persisted settings store feature selections as keyed ids so we can keep one shared
// model library while still letting polish / translation / summary pick independently.
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

export function sanitizeProviderSetting(
  provider: LlmProvider,
  setting?: Partial<LlmProviderSetting> | null,
  customProviders?: LlmSettings['customProviders'],
): LlmProviderSetting {
  const defaults = createProviderSetting(provider, customProviders);
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

export function normalizeTemperature(value: unknown): number | undefined {
  return typeof value === 'number' && value >= 0 && value <= 2 ? value : undefined;
}

export function createLlmSettings(activeProvider: LlmProvider = DEFAULT_LLM_PROVIDER): LlmSettings {
  return {
    activeProvider,
    customProviders: {},
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
  return sanitizeProviderSetting(provider, llmSettings?.providers?.[provider], llmSettings?.customProviders);
}

export function setActiveProvider(
  llmSettings: LlmSettings | undefined,
  provider: LlmProvider,
): LlmSettings {
  const current = llmSettings ?? createLlmSettings();
  return {
    ...current,
    activeProvider: provider,
    customProviders: current.customProviders ?? {},
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
    customProviders: current.customProviders ?? {},
    activeProvider: current.activeProvider,
    providers: {
      ...current.providers,
      [provider]: sanitizeProviderSetting(provider, {
        ...ensureProviderSetting(current, provider),
        ...updates,
      }, current.customProviders),
    },
  };
}

export function addCustomProvider(
  llmSettings: LlmSettings | undefined,
  input: {
    name: string;
    strategy: CustomLlmProviderStrategy;
    createdAt?: string;
  },
): LlmSettings {
  const current = llmSettings ?? createLlmSettings();
  const existingCustomProviders = current.customProviders ?? {};
  const provider: CustomLlmProvider = {
    id: createCustomProviderId(input.name, {
      ...current.providers,
      ...existingCustomProviders,
    }),
    name: input.name.trim(),
    strategy: input.strategy,
    createdAt: input.createdAt ?? new Date().toISOString(),
  };

  return {
    ...current,
    activeProvider: provider.id,
    customProviders: {
      ...existingCustomProviders,
      [provider.id]: provider,
    },
    providers: {
      ...current.providers,
      [provider.id]: sanitizeProviderSetting(provider.id, undefined, {
        ...existingCustomProviders,
        [provider.id]: provider,
      }),
    },
  };
}

export function addLlmModel(
  llmSettings: LlmSettings | undefined,
  entry: Pick<LlmModelEntry, 'provider' | 'model'> & Partial<Pick<LlmModelEntry, 'source' | 'metadata'>>,
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
    const existing = current.models[existingId];
    const nextEntry: LlmModelEntry = {
      ...existing,
      source: entry.source ?? existing.source ?? 'manual',
      metadata: entry.metadata ?? existing.metadata,
    };
    if (
      nextEntry.source === existing.source
      && nextEntry.metadata === existing.metadata
    ) {
      return current;
    }

    return {
      ...current,
      customProviders: current.customProviders ?? {},
      models: {
        ...current.models,
        [existingId]: nextEntry,
      },
    };
  }

  const nextId = ensureUniqueModelId(current.models, createModelId(entry.provider, model));
  return {
    ...current,
    customProviders: current.customProviders ?? {},
    models: {
      ...current.models,
      [nextId]: {
        id: nextId,
        provider: entry.provider,
        model,
        source: entry.source ?? 'manual',
        metadata: entry.metadata,
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
    customProviders: current.customProviders ?? {},
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
    customProviders: current.customProviders ?? {},
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
    customProviders: current.customProviders ?? {},
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

export function getProviderLlmModels(
  llmSettings: LlmSettings | undefined,
  provider: LlmProvider,
): LlmModelEntry[] {
  return getOrderedLlmModels(llmSettings).filter((entry) => entry.provider === provider);
}

export function findLlmModelId(
  llmSettings: LlmSettings | undefined,
  provider: LlmProvider,
  model: string,
): string | undefined {
  const normalizedModel = model.trim();
  if (!normalizedModel) {
    return undefined;
  }

  const current = llmSettings ?? createLlmSettings(provider);
  return current.modelOrder.find((modelId) => {
    const entry = current.models[modelId];
    return entry?.provider === provider && entry.model === normalizedModel;
  });
}

export function syncProviderDiscoveredModels(
  llmSettings: LlmSettings | undefined,
  provider: LlmProvider,
  discoveredModels: LlmDiscoveredModelSummary[],
): LlmSettings {
  const current = llmSettings ?? createLlmSettings(provider);
  const nextDiscoveredModelNames = new Set(
    discoveredModels
      .map((entry) => entry.model.trim())
      .filter(Boolean),
  );

  let nextSettings = current;
  for (const discoveredModel of discoveredModels) {
    const model = discoveredModel.model.trim();
    if (!model) {
      continue;
    }
    nextSettings = addLlmModel(nextSettings, {
      provider,
      model,
      source: 'discovered',
      metadata: {
        inputPrice: discoveredModel.inputPrice,
        outputPrice: discoveredModel.outputPrice,
        contextWindow: discoveredModel.contextWindow,
        maxOutputTokens: discoveredModel.maxOutputTokens,
        supportsMultimodal: discoveredModel.supportsMultimodal,
        supportsTools: discoveredModel.supportsTools,
        supportsReasoning: discoveredModel.supportsReasoning,
      },
    });
  }

  for (const entry of getProviderLlmModels(nextSettings, provider)) {
    if (entry.source !== 'discovered' || nextDiscoveredModelNames.has(entry.model)) {
      continue;
    }
    nextSettings = removeLlmModel(nextSettings, entry.id);
  }

  return nextSettings;
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

export function buildLlmConfigPatch(
  nextLlmSettings: LlmSettings,
): Pick<AppConfig, 'llmSettings'> {
  return {
    llmSettings: nextLlmSettings,
  };
}
