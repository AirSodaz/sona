import type { AppConfig } from '../../types/config';
import {
  CustomLlmProvider,
  CustomLlmProviderStrategy,
  LlmDiscoveredModelSummary,
  LlmFeature,
  LlmModelDiscoveryStatus,
  LlmModelEntry,
  LlmModelMetadata,
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

const EDITABLE_MODEL_METADATA_KEYS = [
  'inputPrice',
  'outputPrice',
  'contextWindow',
  'maxOutputTokens',
  'supportsMultimodal',
  'supportsTools',
  'supportsReasoning',
] as const satisfies (keyof LlmModelMetadata)[];

const MODEL_DISCOVERY_TTL_MS = 24 * 60 * 60 * 1000;

function parseTimestampMs(value: string | undefined): number | null {
  if (!value) {
    return null;
  }

  const parsed = Date.parse(value);
  return Number.isFinite(parsed) ? parsed : null;
}

function createDiscoveryStatus(fetchedAt: string = new Date().toISOString()): LlmModelDiscoveryStatus {
  const fetchedAtMs = parseTimestampMs(fetchedAt) ?? Date.now();
  return {
    fetchedAt,
    expiresAt: new Date(fetchedAtMs + MODEL_DISCOVERY_TTL_MS).toISOString(),
  };
}

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
    modelDiscovery: {} as LlmSettings['modelDiscovery'],
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
  const existingSetting = ensureProviderSetting(current, provider);
  const nextSetting = sanitizeProviderSetting(provider, {
    ...existingSetting,
    ...updates,
  }, current.customProviders);
  const settingChanged =
    existingSetting.apiHost !== nextSetting.apiHost ||
    existingSetting.apiKey !== nextSetting.apiKey ||
    existingSetting.apiPath !== nextSetting.apiPath ||
    existingSetting.apiVersion !== nextSetting.apiVersion;
  const currentDiscovery = current.modelDiscovery ?? {};
  return {
    ...current,
    customProviders: current.customProviders ?? {},
    activeProvider: current.activeProvider,
    modelDiscovery: settingChanged
      ? {
        ...currentDiscovery,
        [provider]: undefined,
      }
      : current.modelDiscovery,
    providers: {
      ...current.providers,
      [provider]: nextSetting,
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
  entry: Pick<LlmModelEntry, 'provider' | 'model'> & Partial<Pick<LlmModelEntry, 'source' | 'metadata' | 'metadataOverrides'>>,
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
    const metadataOverrides = entry.metadataOverrides ?? existing.metadataOverrides;
    const metadata = mergeModelMetadata(existing.metadata, entry.metadata, metadataOverrides);
    const nextEntry: LlmModelEntry = {
      ...existing,
      source: entry.source ?? existing.source ?? 'manual',
      metadata,
      metadataOverrides,
    };
    if (
      nextEntry.source === existing.source
      && nextEntry.metadata === existing.metadata
      && nextEntry.metadataOverrides === existing.metadataOverrides
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
        metadataOverrides: entry.metadataOverrides,
      },
    },
    modelOrder: [...current.modelOrder, nextId],
  };
}

function mergeModelMetadata(
  existing: LlmModelMetadata | undefined,
  incoming: LlmModelMetadata | undefined,
  overrides: LlmModelEntry['metadataOverrides'] | undefined,
): LlmModelMetadata | undefined {
  if (!incoming) {
    return existing;
  }

  const next: LlmModelMetadata = {
    ...existing,
  };

  for (const key of EDITABLE_MODEL_METADATA_KEYS) {
    if (overrides?.[key]) {
      continue;
    }
    next[key] = incoming[key] as never;
  }

  return next;
}

export function updateLlmModelMetadata(
  llmSettings: LlmSettings | undefined,
  modelId: string,
  metadata: Partial<LlmModelMetadata>,
): LlmSettings {
  const current = llmSettings ?? createLlmSettings();
  const existing = current.models[modelId];
  if (!existing) {
    return current;
  }

  const nextMetadata: LlmModelMetadata = {
    ...(existing.metadata ?? {}),
  };
  const nextOverrides: LlmModelEntry['metadataOverrides'] = {
    ...(existing.metadataOverrides ?? {}),
  };

  for (const key of EDITABLE_MODEL_METADATA_KEYS) {
    if (!Object.prototype.hasOwnProperty.call(metadata, key)) {
      continue;
    }
    nextMetadata[key] = metadata[key] as never;
    nextOverrides[key] = true;
  }

  return {
    ...current,
    customProviders: current.customProviders ?? {},
    models: {
      ...current.models,
      [modelId]: {
        ...existing,
        metadata: nextMetadata,
        metadataOverrides: nextOverrides,
      },
    },
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

export function getModelDiscoveryStatus(
  llmSettings: LlmSettings | undefined,
  provider: LlmProvider,
): LlmModelDiscoveryStatus | undefined {
  return llmSettings?.modelDiscovery?.[provider];
}

export function isProviderModelDiscoveryExpired(
  llmSettings: LlmSettings | undefined,
  provider: LlmProvider,
  now: string = new Date().toISOString(),
): boolean {
  const status = getModelDiscoveryStatus(llmSettings, provider);
  const expiresAtMs = parseTimestampMs(status?.expiresAt);
  const nowMs = parseTimestampMs(now);
  if (expiresAtMs === null || nowMs === null) {
    return true;
  }

  return nowMs >= expiresAtMs;
}

export function syncProviderDiscoveredModels(
  llmSettings: LlmSettings | undefined,
  provider: LlmProvider,
  discoveredModels: LlmDiscoveredModelSummary[],
  fetchedAt?: string,
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

  return {
    ...nextSettings,
    modelDiscovery: {
      ...(nextSettings.modelDiscovery ?? {}),
      [provider]: createDiscoveryStatus(fetchedAt),
    },
  };
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
