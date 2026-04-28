import type { AppConfig } from '../../types/config';
import type {
  LlmModelEntry,
  LlmProvider,
  LlmProviderSetting,
  LlmSettings,
} from '../../types/transcript';
import { normalizeProvider } from './providers';
import {
  addLlmModel,
  normalizeTemperature,
  sanitizeProviderSetting,
  setFeatureModelSelection,
} from './state';

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

function resolveLegacyBootstrapModel(source: Partial<AppConfig> & Record<string, any>): LegacyBootstrapModel | null {
  const legacyStoredProviderModel = Object.entries(source.llmSettings?.providers ?? {}).find(([, rawSetting]) => {
    const model = (rawSetting as { model?: string } | undefined)?.model;
    return typeof model === 'string' && model.trim();
  });

  if (legacyStoredProviderModel) {
    return {
      provider: normalizeProvider(legacyStoredProviderModel[0]),
      model: ((legacyStoredProviderModel[1] as { model?: string }).model || '').trim(),
    };
  }

  return extractLegacyModel(source);
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

  // Migration order matters here:
  // 1. normalize providers and legacy runtime credentials,
  // 2. hydrate only valid models / selections,
  // 3. bootstrap a translation-capable fallback when no model survives,
  // 4. backfill summary only when the chosen polish model also supports summary.
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

  llmSettings = bootstrapMissingModelSelections(llmSettings, resolveLegacyBootstrapModel(candidate));
  llmSettings = applyLegacyTemperature(llmSettings, candidate.llm?.temperature);
  llmSettings = ensureSummaryModelSelection(llmSettings);

  return {
    llmSettings,
  };
}
