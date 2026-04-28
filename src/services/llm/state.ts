import type { AppConfig } from '../../types/config';
import {
  LlmFeature,
  LlmModelEntry,
  LlmProvider,
  LlmProviderSetting,
  LlmSettings,
} from '../../types/transcript';
import {
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

export function normalizeTemperature(value: unknown): number | undefined {
  return typeof value === 'number' && value >= 0 && value <= 2 ? value : undefined;
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

export function buildLlmConfigPatch(
  nextLlmSettings: LlmSettings,
): Pick<AppConfig, 'llmSettings'> {
  return {
    llmSettings: nextLlmSettings,
  };
}
