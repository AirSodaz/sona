import type { AppConfig } from '../../types/config';
import type {
  CustomLlmProvider,
  CustomLlmProviderId,
  CustomLlmProviderStrategy,
  LlmModelDiscoveryStatus,
  LlmModelEntry,
  LlmModelMetadata,
  LlmProvider,
  LlmProviderSetting,
  LlmSettings,
} from '../../types/transcript';
import { isCustomProviderId, normalizeProvider } from './providers';
import {
  addLlmModel,
  normalizeTemperature,
  sanitizeProviderSetting,
  setFeatureModelSelection,
} from './state';

const EDITABLE_MODEL_METADATA_KEYS = [
  'inputPrice',
  'outputPrice',
  'contextWindow',
  'maxOutputTokens',
  'supportsMultimodal',
  'supportsTools',
  'supportsReasoning',
] as const satisfies (keyof LlmModelMetadata)[];

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
    source: entry.source === 'discovered' ? 'discovered' : 'manual',
    metadata: entry.metadata && typeof entry.metadata === 'object' ? {
      inputPrice: typeof entry.metadata.inputPrice === 'number' ? entry.metadata.inputPrice : undefined,
      outputPrice: typeof entry.metadata.outputPrice === 'number' ? entry.metadata.outputPrice : undefined,
      contextWindow: typeof entry.metadata.contextWindow === 'number' ? entry.metadata.contextWindow : undefined,
      maxOutputTokens: typeof entry.metadata.maxOutputTokens === 'number' ? entry.metadata.maxOutputTokens : undefined,
      supportsMultimodal:
        typeof entry.metadata.supportsMultimodal === 'boolean' ? entry.metadata.supportsMultimodal : undefined,
      supportsTools: typeof entry.metadata.supportsTools === 'boolean' ? entry.metadata.supportsTools : undefined,
      supportsReasoning:
        typeof entry.metadata.supportsReasoning === 'boolean' ? entry.metadata.supportsReasoning : undefined,
    } : undefined,
    metadataOverrides: sanitizeMetadataOverrides(entry.metadataOverrides),
  };
}

function sanitizeMetadataOverrides(
  overrides: LlmModelEntry['metadataOverrides'] | null | undefined,
): LlmModelEntry['metadataOverrides'] | undefined {
  if (!overrides || typeof overrides !== 'object') {
    return undefined;
  }

  const nextOverrides: LlmModelEntry['metadataOverrides'] = {};
  for (const key of EDITABLE_MODEL_METADATA_KEYS) {
    if (overrides[key] === true) {
      nextOverrides[key] = true;
    }
  }

  return Object.keys(nextOverrides).length > 0 ? nextOverrides : undefined;
}

function isValidIsoTimestamp(value: unknown): value is string {
  return typeof value === 'string' && Number.isFinite(Date.parse(value));
}

function normalizeStoredModelDiscovery(
  rawDiscovery: unknown,
): LlmSettings['modelDiscovery'] {
  if (!rawDiscovery || typeof rawDiscovery !== 'object') {
    return {};
  }

  const modelDiscovery: LlmSettings['modelDiscovery'] = {};
  for (const [rawProvider, rawStatus] of Object.entries(rawDiscovery as Record<string, unknown>)) {
    if (!rawStatus || typeof rawStatus !== 'object') {
      continue;
    }

    const status = rawStatus as Partial<LlmModelDiscoveryStatus>;
    if (!isValidIsoTimestamp(status.fetchedAt) || !isValidIsoTimestamp(status.expiresAt)) {
      continue;
    }

    const provider = normalizeProvider(rawProvider);
    modelDiscovery[provider] = {
      fetchedAt: status.fetchedAt,
      expiresAt: status.expiresAt,
    };
  }

  return modelDiscovery;
}

// Stored model records are untrusted compatibility input. Drop incomplete rows here so
// later selection logic can assume every remaining model is valid and provider-normalized.
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

// Keep persisted order when possible, then append any surviving models that were missing
// from the order array so migrations never orphan a valid model entry.
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

// Feature selections are only valid when they still point at a normalized model. This
// lets migration clean dangling ids once, instead of forcing every consumer to re-check.
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

// Google Translate providers remain translation-only in product semantics, so they can
// seed a default translation model without implicitly enabling summary generation.
function supportsSummaryModel(modelEntry: LlmModelEntry | undefined): boolean {
  if (!modelEntry) {
    return false;
  }

  return modelEntry.provider !== 'google_translate' && modelEntry.provider !== 'google_translate_free';
}

type LegacyBootstrapModel = {
  provider: LlmProvider;
  model: string;
};

type LegacyStoredProviderSetting = Partial<LlmProviderSetting> & {
  model?: unknown;
};

const LEGACY_OPENAI_COMPATIBLE_CUSTOM_PROVIDER: CustomLlmProvider = {
  id: 'custom-openai-compatible',
  name: 'OpenAI Compatible',
  strategy: 'openai_compatible',
  createdAt: '2026-05-18T00:00:00.000Z',
};

type LegacyNestedLlmSource = {
  provider?: unknown;
  model?: unknown;
  baseUrl?: unknown;
  apiKey?: unknown;
  apiPath?: unknown;
  apiVersion?: unknown;
  temperature?: unknown;
};

export type LlmMigrationSource = Partial<AppConfig> & {
  llm?: LegacyNestedLlmSource;
  llmServiceType?: unknown;
  llmModel?: unknown;
  aiModel?: unknown;
  model?: unknown;
  llmBaseUrl?: unknown;
  aiBaseUrl?: unknown;
  baseUrl?: unknown;
  llmApiKey?: unknown;
  aiApiKey?: unknown;
  apiKey?: unknown;
  llmApiPath?: unknown;
  aiApiPath?: unknown;
  apiPath?: unknown;
  llmApiVersion?: unknown;
  aiApiVersion?: unknown;
  apiVersion?: unknown;
};

function getTrimmedString(value: unknown): string | undefined {
  return typeof value === 'string' && value.trim() ? value.trim() : undefined;
}

function isCustomProviderStrategy(value: unknown): value is CustomLlmProviderStrategy {
  return value === 'openai_compatible'
    || value === 'openai_responses'
    || value === 'anthropic'
    || value === 'gemini';
}

function normalizeStoredCustomProviders(rawProviders: unknown): Record<CustomLlmProviderId, CustomLlmProvider> {
  if (!rawProviders || typeof rawProviders !== 'object') {
    return {};
  }

  const customProviders: Record<CustomLlmProviderId, CustomLlmProvider> = {};
  for (const [rawId, rawProvider] of Object.entries(rawProviders as Record<string, unknown>)) {
    const provider = rawProvider as Partial<CustomLlmProvider>;
    const normalizedId = normalizeProvider(provider.id ?? rawId);
    if (!isCustomProviderId(normalizedId) || !isCustomProviderStrategy(provider.strategy)) {
      continue;
    }

    const name = getTrimmedString(provider.name) ?? normalizedId;
    const createdAt = getTrimmedString(provider.createdAt) ?? LEGACY_OPENAI_COMPATIBLE_CUSTOM_PROVIDER.createdAt;
    customProviders[normalizedId] = {
      id: normalizedId,
      name,
      strategy: provider.strategy,
      createdAt,
    };
  }

  return customProviders;
}

function needsLegacyOpenAiCompatibleProvider(source: LlmMigrationSource): boolean {
  if (normalizeProvider(source.llmSettings?.activeProvider) === LEGACY_OPENAI_COMPATIBLE_CUSTOM_PROVIDER.id) {
    return true;
  }
  if (normalizeProvider(source.llm?.provider ?? source.llmServiceType) === LEGACY_OPENAI_COMPATIBLE_CUSTOM_PROVIDER.id) {
    return true;
  }

  const providerKeys = Object.keys(source.llmSettings?.providers ?? {});
  if (providerKeys.some((provider) => normalizeProvider(provider) === LEGACY_OPENAI_COMPATIBLE_CUSTOM_PROVIDER.id)) {
    return true;
  }

  return Object.values(source.llmSettings?.models ?? {}).some((rawEntry) => {
    const entry = rawEntry as Partial<LlmModelEntry> | undefined;
    return normalizeProvider(entry?.provider) === LEGACY_OPENAI_COMPATIBLE_CUSTOM_PROVIDER.id;
  });
}

function extractLegacyProviderSetting(source: LlmMigrationSource): Partial<LlmProviderSetting> {
  return {
    apiHost: getTrimmedString(source.llmBaseUrl) ?? getTrimmedString(source.aiBaseUrl) ?? getTrimmedString(source.baseUrl),
    apiKey: getTrimmedString(source.llmApiKey) ?? getTrimmedString(source.aiApiKey) ?? getTrimmedString(source.apiKey),
    apiPath: getTrimmedString(source.llmApiPath) ?? getTrimmedString(source.aiApiPath) ?? getTrimmedString(source.apiPath),
    apiVersion: getTrimmedString(source.llmApiVersion) ?? getTrimmedString(source.aiApiVersion) ?? getTrimmedString(source.apiVersion),
  };
}

function extractLegacyModel(source: LlmMigrationSource): { provider: LlmProvider; model: string } | null {
  const provider = normalizeProvider(
    source.llmSettings?.activeProvider ?? source.llm?.provider ?? source.llmServiceType,
  );
  const model = getTrimmedString(source.llm?.model)
    ?? getTrimmedString(source.llmModel)
    ?? getTrimmedString(source.aiModel)
    ?? getTrimmedString(source.model)
    ?? '';

  return model ? { provider, model } : null;
}

function resolveLegacyBootstrapModel(source: LlmMigrationSource): LegacyBootstrapModel | null {
  const legacyStoredProviderModel = Object.entries(source.llmSettings?.providers ?? {}).find(([, rawSetting]) => {
    const model = (rawSetting as LegacyStoredProviderSetting | undefined)?.model;
    return typeof model === 'string' && model.trim();
  });

  if (legacyStoredProviderModel) {
    return {
      provider: normalizeProvider(legacyStoredProviderModel[0]),
      model: getTrimmedString((legacyStoredProviderModel[1] as LegacyStoredProviderSetting).model) || '',
    };
  }

  return extractLegacyModel(source);
}

function normalizeStoredProviders(
  rawProviders: unknown,
  customProviders: LlmSettings['customProviders'],
): Partial<Record<LlmProvider, LlmProviderSetting>> {
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
    }, customProviders);
  }

  return providers;
}

function bootstrapMissingModelSelections(
  llmSettings: LlmSettings,
  legacyModel: LegacyBootstrapModel | null,
): LlmSettings {
  if (llmSettings.modelOrder.length > 0) {
    return llmSettings;
  }

  if (legacyModel) {
    const nextSettings = addLlmModel(llmSettings, legacyModel);
    const migratedModelId = nextSettings.modelOrder[0];

    // Legacy single-model setups powered both polish and translation. Summary is filled
    // separately below so translation-only providers do not accidentally unlock it.
    return setFeatureModelSelection(
      setFeatureModelSelection(nextSettings, 'polish', migratedModelId),
      'translation',
      migratedModelId,
    );
  }

  // Fresh installs still need one usable translation path even before the user picks an
  // LLM provider, so we bootstrap the free Google model as a translation-only fallback.
  const nextSettings = addLlmModel(llmSettings, { provider: 'google_translate_free', model: 'default' });
  const defaultModelId = nextSettings.modelOrder[0];
  return setFeatureModelSelection(nextSettings, 'translation', defaultModelId);
}

function ensureSummaryModelSelection(llmSettings: LlmSettings): LlmSettings {
  if (llmSettings.selections.summaryModelId) {
    return llmSettings;
  }

  const polishModelId = llmSettings.selections.polishModelId;
  const polishModelEntry = polishModelId ? llmSettings.models[polishModelId] : undefined;
  if (!polishModelId || !supportsSummaryModel(polishModelEntry)) {
    return llmSettings;
  }

  return setFeatureModelSelection(llmSettings, 'summary', polishModelId);
}

export function ensureLlmState(
  source?: LlmMigrationSource,
): { llmSettings: LlmSettings } {
  const candidate = source ?? {};
  const currentProvider = normalizeProvider(
    candidate.llmSettings?.activeProvider ??
      candidate.llmServiceType ??
      candidate.llm?.provider,
  );

  const customProviders = normalizeStoredCustomProviders(candidate.llmSettings?.customProviders);
  if (needsLegacyOpenAiCompatibleProvider(candidate)) {
    customProviders[LEGACY_OPENAI_COMPATIBLE_CUSTOM_PROVIDER.id] = LEGACY_OPENAI_COMPATIBLE_CUSTOM_PROVIDER;
  }
  const providers = normalizeStoredProviders(candidate.llmSettings?.providers, customProviders);

  if (candidate.llm) {
    const provider = normalizeProvider(candidate.llm.provider);
    providers[provider] = sanitizeProviderSetting(provider, {
      apiHost: getTrimmedString(candidate.llm.baseUrl),
      apiKey: getTrimmedString(candidate.llm.apiKey),
      apiPath: getTrimmedString(candidate.llm.apiPath),
      apiVersion: getTrimmedString(candidate.llm.apiVersion),
    }, customProviders);
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
      }, customProviders);
    }
  }

  const storedModels = normalizeStoredModels(candidate.llmSettings?.models);
  const storedModelOrder = normalizeStoredModelOrder(candidate.llmSettings?.modelOrder, storedModels);
  const storedModelDiscovery = normalizeStoredModelDiscovery(candidate.llmSettings?.modelDiscovery);
  const storedSelections = normalizeStoredSelections(candidate.llmSettings?.selections, storedModels);

  // Migration order matters here:
  // 1. normalize providers and legacy runtime credentials,
  // 2. hydrate only valid models / selections,
  // 3. bootstrap a translation-capable fallback when no model survives,
  // 4. backfill summary only when the chosen polish model also supports summary.
  let llmSettings: LlmSettings = {
    activeProvider: currentProvider,
    customProviders,
    providers: {
      ...providers,
      [currentProvider]: sanitizeProviderSetting(currentProvider, providers[currentProvider], customProviders),
    },
    models: storedModels,
    modelOrder: storedModelOrder,
    modelDiscovery: storedModelDiscovery,
    selections: storedSelections,
  };

  llmSettings = bootstrapMissingModelSelections(llmSettings, resolveLegacyBootstrapModel(candidate));
  llmSettings = applyLegacyTemperature(llmSettings, candidate.llm?.temperature);
  llmSettings = ensureSummaryModelSelection(llmSettings);

  return {
    llmSettings,
  };
}
