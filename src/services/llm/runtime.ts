import type { AppConfig } from '../../types/config';
import type {
  CustomLlmProvider,
  CustomLlmProviderStrategy,
  LlmConfig,
  LlmFeature,
  LlmProvider,
  LlmProviderSetting,
} from '../../types/transcript';
import {
  buildLlmConfig,
  DEFAULT_LLM_TEMPERATURE,
  getProviderDefinition,
  normalizeProvider,
} from './providers';
import {
  ensureProviderSetting,
  getFeatureModelEntry,
} from './state';

function customProviderFromConfig(llmConfig: LlmConfig): Partial<Record<LlmProvider, CustomLlmProvider>> | undefined {
  if (!llmConfig.provider.startsWith('custom-')) {
    return undefined;
  }

  const strategy: CustomLlmProviderStrategy = llmConfig.strategy === 'anthropic' || llmConfig.strategy === 'gemini' || llmConfig.strategy === 'openai_responses'
    ? llmConfig.strategy
    : 'openai_compatible';

  return {
    [llmConfig.provider]: {
      id: llmConfig.provider as `custom-${string}`,
      name: llmConfig.provider,
      strategy,
      createdAt: '',
    },
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
  return buildLlmConfig(provider, getActiveProviderSetting(config), config.llmSettings?.customProviders);
}

function getFeatureTemperature(
  config: Pick<AppConfig, 'llmSettings'>,
  feature: LlmFeature,
): number | undefined {
  let selectionTemperature: number | undefined;

  if (feature === 'polish') {
    selectionTemperature = config.llmSettings?.selections.polishTemperature;
  } else if (feature === 'translation') {
    selectionTemperature = config.llmSettings?.selections.translationTemperature;
  } else {
    selectionTemperature = config.llmSettings?.selections.summaryTemperature;
  }

  return selectionTemperature ?? DEFAULT_LLM_TEMPERATURE;
}

const FEATURE_REASONING_ENABLED_KEYS = {
  polish: 'polishReasoningEnabled',
  translation: 'translationReasoningEnabled',
  summary: 'summaryReasoningEnabled',
} as const;

const FEATURE_REASONING_LEVEL_KEYS = {
  polish: 'polishReasoningLevel',
  translation: 'translationReasoningLevel',
  summary: 'summaryReasoningLevel',
} as const;

export function getFeatureLlmConfig(
  config: Pick<AppConfig, 'llmSettings'>,
  feature: LlmFeature,
): LlmConfig | null {
  const modelEntry = getFeatureModelEntry(config, feature);
  if (!modelEntry) {
    return null;
  }

  const setting = ensureProviderSetting(config.llmSettings, modelEntry.provider);
  // Runtime calls always derive from the selected feature model instead of the active
  // provider UI. That keeps one persisted provider registry while allowing each feature
  // to talk to a different backend with its own model and temperature.
  const selections = config.llmSettings?.selections;
  const reasoningEnabled = selections?.[FEATURE_REASONING_ENABLED_KEYS[feature]] ?? false;
  const reasoningLevel = selections?.[FEATURE_REASONING_LEVEL_KEYS[feature]] ?? 'medium';

  return {
    ...buildLlmConfig(modelEntry.provider, setting, config.llmSettings?.customProviders),
    model: modelEntry.model,
    temperature: getFeatureTemperature(config, feature),
    reasoningEnabled,
    reasoningLevel: reasoningEnabled ? reasoningLevel : undefined,
  };
}

export function isLlmConfigComplete(llmConfig: LlmConfig | null): boolean {
  if (!llmConfig) {
    return false;
  }

  const definition = getProviderDefinition(llmConfig.provider, customProviderFromConfig(llmConfig));
  const hasApiHost = Boolean(llmConfig.baseUrl?.trim() || definition.defaultApiHost);
  const hasApiKey = !definition.requiresApiKey || Boolean(llmConfig.apiKey?.trim());
  const hasModel = Boolean(llmConfig.model?.trim());

  return hasApiHost && hasApiKey && hasModel;
}

export function isProviderConfigComplete(
  provider: LlmProvider,
  setting: LlmProviderSetting | undefined,
  customProviders?: Pick<NonNullable<AppConfig['llmSettings']>, 'customProviders'>['customProviders'],
): boolean {
  const definition = getProviderDefinition(provider, customProviders);
  const hasApiHost = Boolean(setting?.apiHost?.trim() || definition.defaultApiHost);
  const hasApiKey = !definition.requiresApiKey || Boolean(setting?.apiKey?.trim());

  return hasApiHost && hasApiKey;
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
